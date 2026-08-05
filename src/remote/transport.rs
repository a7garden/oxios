//! WebSocket transport for the encrypted remote companion surface.
//!
//! Phase 2: each connection owns a [`ConnectionCtx`] carrying a push sender
//! (for server-initiated subscription frames) and per-connection auth state.
//! The handler signature is `Fn(Vec<u8>, Arc<ConnectionCtx>) -> Fut<Vec<u8>>`.
#![allow(dead_code)]

use std::{borrow::Cow, collections::VecDeque, future::Future, net::SocketAddr, sync::Arc};

use anyhow::{Context, Result, anyhow};
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    WebSocketStream, accept_async,
    tungstenite::{
        Message,
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};
use tokio_util::sync::CancellationToken;

use super::noise::{self, FrameType, Responder};

/// Sender for server-pushed plaintext frames. The transport loop encrypts
/// anything sent here and writes it to the socket as an `App` frame.
/// Cloned into subscription tasks spawned by the RPC dispatch layer.
pub type PushSender = mpsc::UnboundedSender<Vec<u8>>;

/// Per-connection context created fresh for each companion session.
///
/// Passed to every frame-handler invocation so the handler can:
/// - Read/update auth state (set by `auth.verify`, checked before sensitive methods)
/// - Spawn subscription tasks that push events back to the client
pub struct ConnectionCtx {
    /// Sender for server-pushed plaintext frames. Subscription tasks clone
    /// this and push serialized JSON-RPC notification bytes; the transport
    /// loop encrypts and frames them.
    pub push_tx: PushSender,
    /// Authenticated device ID. `None` until `auth.verify` succeeds.
    /// Checked by `dispatch` before any sensitive RPC method.
    pub device_id: Arc<std::sync::Mutex<Option<String>>>,
}

impl ConnectionCtx {
    /// Create a new connection context with the given push sender.
    pub fn new(push_tx: PushSender) -> Self {
        Self {
            push_tx,
            device_id: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Returns the authenticated device ID, or `None` if not yet verified.
    pub fn authenticated_device(&self) -> Option<String> {
        self.device_id.lock().ok()?.clone()
    }

    /// Set the authenticated device ID (called by `auth.verify` dispatch).
    pub fn set_authenticated(&self, device_id: String) {
        if let Ok(mut guard) = self.device_id.lock() {
            *guard = Some(device_id);
        }
    }

    /// Whether this connection has completed `auth.verify`.
    pub fn is_authenticated(&self) -> bool {
        self.device_id.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

const MAX_QUEUED_FRAMES: usize = 4096;
const MAX_QUEUED_BYTES: usize = 64 * 1024 * 1024;

/// A hard-bounded FIFO of encoded outbound WebSocket frames.
#[derive(Debug, Default)]
pub struct OutboundQueue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
}

impl OutboundQueue {
    /// Create an empty outbound queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one encoded frame unless doing so would exceed a hard cap.
    ///
    /// A rejected frame is not retained and does not change the running byte
    /// total. Exactly 4096 frames and exactly 64 MiB are permitted.
    pub fn push(&mut self, frame: Vec<u8>) -> Result<(), QueueOverflow> {
        let next_frames = self.frames.len().checked_add(1).ok_or(QueueOverflow)?;
        let next_bytes = self.bytes.checked_add(frame.len()).ok_or(QueueOverflow)?;
        if next_frames > MAX_QUEUED_FRAMES || next_bytes > MAX_QUEUED_BYTES {
            return Err(QueueOverflow);
        }

        self.bytes = next_bytes;
        self.frames.push_back(frame);
        Ok(())
    }

    /// Move all queued frames into `out` in FIFO order.
    pub fn drain_into_vec(&mut self, out: &mut Vec<Vec<u8>>) {
        out.reserve(self.frames.len());
        out.extend(self.frames.drain(..));
        self.bytes = 0;
    }
}

/// The outbound queue reached its frame or byte hard cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueOverflow;

impl std::fmt::Display for QueueOverflow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("outbound queue hard cap exceeded")
    }
}

impl std::error::Error for QueueOverflow {}

/// Listen for encrypted WebSocket sessions until `shutdown` is cancelled.
///
/// The caller pre-binds `listener` so it can learn the bound address before
/// handing the listener off. Each accepted connection gets its own
/// [`ConnectionCtx`] (push sender + auth state) created fresh.
///
/// The handler receives `(decrypted_request_bytes, Arc<ConnectionCtx>)` and
/// returns reply bytes. Server-pushed subscription frames are delivered via
/// `ConnectionCtx::push_tx` — the transport loop encrypts and sends them.
pub async fn run_listener<H, Fut>(
    listener: TcpListener,
    server_static: Vec<u8>,
    shutdown: CancellationToken,
    handler: H,
) -> Result<()>
where
    H: Fn(Vec<u8>, Arc<ConnectionCtx>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<u8>> + Send + 'static,
{
    let addr = listener
        .local_addr()
        .context("remote WebSocket listener must be bound")?;
    let handler = Arc::new(handler);
    let server_static = Arc::new(server_static);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!(%addr, "remote WebSocket listener shutting down");
                return Ok(());
            }
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::warn!(%addr, %error, "remote WebSocket accept failed; continuing");
                        continue;
                    }
                };
                let handler = Arc::clone(&handler);
                let server_static = Arc::clone(&server_static);
                let connection_shutdown = shutdown.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(
                        stream,
                        peer,
                        server_static,
                        connection_shutdown,
                        handler,
                    ).await {
                        tracing::warn!(%peer, %error, "remote WebSocket connection ended");
                    }
                });
            }
        }
    }
}

async fn handle_connection<H, Fut>(
    stream: TcpStream,
    peer: SocketAddr,
    server_static: Arc<Vec<u8>>,
    shutdown: CancellationToken,
    handler: Arc<H>,
) -> Result<()>
where
    H: Fn(Vec<u8>, Arc<ConnectionCtx>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<u8>> + Send + 'static,
{
    let mut socket = accept_async(stream)
        .await
        .context("accept remote WebSocket handshake")?;

    let first = tokio::select! {
        _ = shutdown.cancelled() => return Ok(()),
        message = socket.next() => message.transpose().context("read first remote WebSocket message")?,
    };

    let Some(Message::Binary(first)) = first else {
        refuse_plaintext(&mut socket, peer).await;
        return Ok(());
    };
    let Some((FrameType::Noise, first_payload)) = noise::decode_frame(&first) else {
        refuse_plaintext(&mut socket, peer).await;
        return Ok(());
    };

    let mut responder = Responder::new(server_static.as_slice())?;
    if let Some(response) = responder.handshake_msg(first_payload)? {
        send_encoded(&mut socket, FrameType::Noise, &response).await?;
    }

    while !responder.is_handshake_finished() {
        let message = tokio::select! {
            _ = shutdown.cancelled() => {
                close_going_away(&mut socket).await;
                return Ok(());
            }
            message = socket.next() => message.transpose().context("read Noise handshake frame")?,
        };
        let Some(Message::Binary(frame)) = message else {
            return Err(anyhow!(
                "connection closed before Noise handshake completed"
            ));
        };
        let (frame_type, payload) = noise::decode_frame(&frame)
            .ok_or_else(|| anyhow!("malformed Noise handshake frame"))?;
        if frame_type != FrameType::Noise {
            return Err(anyhow!("non-Noise frame during Noise handshake"));
        }
        if let Some(response) = responder.handshake_msg(payload)? {
            send_encoded(&mut socket, FrameType::Noise, &response).await?;
        }
    }

    let mut transport = responder.into_transport()?;
    let mut outbound = OutboundQueue::new();
    let mut drained = Vec::new();

    // Per-connection push channel for server-initiated frames (subscriptions).
    let (push_tx, mut push_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let conn_ctx = Arc::new(ConnectionCtx::new(push_tx));

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                close_going_away(&mut socket).await;
                return Ok(());
            }
            // Server-pushed subscription frames: encrypt + send.
            Some(plaintext) = push_rx.recv() => {
                let ciphertext = transport.encrypt(&plaintext)?;
                let encoded = noise::encode_frame(FrameType::App, &ciphertext)
                    .ok_or_else(|| anyhow!("pushed frame exceeds frame limit"))?;
                outbound.push(encoded)?;
                outbound.drain_into_vec(&mut drained);
                for frame in drained.drain(..) {
                    socket.send(Message::Binary(frame)).await?;
                }
            }
            message = socket.next() => {
                let message = match message.transpose() {
                    Ok(m) => m,
                    Err(e) => return Err(anyhow!("read encrypted app frame: {e}")),
                };
                let Some(message) = message else {
                    return Ok(());
                };

                match message {
                    Message::Binary(frame) => {
                        let (frame_type, payload) = noise::decode_frame(&frame)
                            .ok_or_else(|| anyhow!("malformed encrypted frame"))?;
                        match frame_type {
                            FrameType::App => {
                                let request = transport.decrypt(payload)?;
                                let reply = handler(request, Arc::clone(&conn_ctx)).await;
                                let ciphertext = transport.encrypt(&reply)?;
                                let encoded = noise::encode_frame(FrameType::App, &ciphertext)
                                    .ok_or_else(|| anyhow!("encrypted reply exceeds frame limit"))?;
                                outbound.push(encoded)?;
                                outbound.drain_into_vec(&mut drained);
                                for frame in drained.drain(..) {
                                    socket.send(Message::Binary(frame)).await?;
                                }
                            }
                            FrameType::Ping => {
                                let plaintext = transport.decrypt(payload)?;
                                let ciphertext = transport.encrypt(&plaintext)?;
                                let encoded = noise::encode_frame(FrameType::Pong, &ciphertext)
                                    .ok_or_else(|| anyhow!("encrypted pong exceeds frame limit"))?;
                                outbound.push(encoded)?;
                                outbound.drain_into_vec(&mut drained);
                                for frame in drained.drain(..) {
                                    socket.send(Message::Binary(frame)).await?;
                                }
                            }
                            FrameType::Close => {
                                let _ = transport.decrypt(payload)?;
                                close_normal(&mut socket).await;
                                return Ok(());
                            }
                            FrameType::Noise | FrameType::Pong => {}
                        }
                    }
                    Message::Ping(payload) => socket.send(Message::Pong(payload)).await?,
                    Message::Close(_) => return Ok(()),
                    Message::Text(_) | Message::Pong(_) | Message::Frame(_) => {}
                }
            }
        }
    }
}

async fn send_encoded(
    socket: &mut WebSocketStream<TcpStream>,
    frame_type: FrameType,
    payload: &[u8],
) -> Result<()> {
    let encoded = noise::encode_frame(frame_type, payload)
        .ok_or_else(|| anyhow!("outbound frame exceeds frame limit"))?;
    socket.send(Message::Binary(encoded)).await?;
    Ok(())
}

async fn refuse_plaintext(socket: &mut WebSocketStream<TcpStream>, peer: SocketAddr) {
    tracing::warn!(%peer, "plaintext refused");
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: CloseCode::Policy,
            reason: Cow::Borrowed("plaintext refused"),
        })))
        .await;
}

async fn close_going_away(socket: &mut WebSocketStream<TcpStream>) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: CloseCode::Away,
            reason: Cow::Borrowed("server shutting down"),
        })))
        .await;
}

async fn close_normal(socket: &mut WebSocketStream<TcpStream>) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: Cow::Borrowed("encrypted close"),
        })))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_drains_in_order() {
        let mut queue = OutboundQueue::new();
        queue.push(b"a".to_vec()).expect("first frame fits");
        queue.push(b"b".to_vec()).expect("second frame fits");
        queue.push(b"c".to_vec()).expect("third frame fits");

        let mut out = Vec::new();
        queue.drain_into_vec(&mut out);

        assert_eq!(out, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn queue_overflow_errors_past_hard_cap() {
        let mut queue = OutboundQueue::new();
        for _ in 0..4096 {
            queue.push(b"x".to_vec()).expect("frame at cap fits");
        }

        assert!(
            queue.push(b"y".to_vec()).is_err(),
            "4097th frame must overflow"
        );
    }
}
