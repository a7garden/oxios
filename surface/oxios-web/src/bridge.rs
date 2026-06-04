//! Web bridge implementation.
//!
//! Implements the [`Channel`] trait for the web interface, allowing
//! the gateway to route messages to and from the HTTP API.
//!
//! Uses mpsc channels to bridge:
//! - **Incoming**: HTTP POST /api/chat → mpsc → Gateway → Kernel
//! - **Outgoing**: Kernel → Gateway → mpsc → WebSocket/SSE clients

use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::channel::Channel;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use oxios_gateway::GatewayInbox;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex, RwLock};

/// The web bridge adapter.
///
/// Bridges the axum HTTP server with the gateway's channel interface
/// using mpsc channels for message passing.
pub struct WebBridge {
    /// Receiver for incoming messages from the HTTP layer.
    /// `Option` so `start()` can take ownership via `take()`.
    incoming_rx: Mutex<Option<mpsc::Receiver<IncomingMessage>>>,
    /// Sender to pass to the HTTP layer for injecting messages.
    incoming_tx: mpsc::Sender<IncomingMessage>,
    /// Broadcaster for outgoing messages to WebSocket/SSE clients.
    outgoing_tx: broadcast::Sender<OutgoingMessage>,
    /// Correlation map for HTTP request-response matching.
    responses: Arc<RwLock<HashMap<uuid::Uuid, oneshot::Sender<OutgoingMessage>>>>,
}

impl WebBridge {
    /// Creates a new web bridge with a bounded message buffer.
    pub fn new(buffer: usize) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::channel(buffer);
        let (outgoing_tx, _) = broadcast::channel(buffer);
        Self {
            incoming_rx: Mutex::new(Some(incoming_rx)),
            incoming_tx,
            outgoing_tx,
            responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns a sender that can be used by HTTP handlers to inject messages.
    pub fn sender(&self) -> mpsc::Sender<IncomingMessage> {
        self.incoming_tx.clone()
    }

    /// Returns a receiver for outgoing messages (used by WebSocket/SSE handlers).
    pub fn subscribe_outgoing(&self) -> broadcast::Receiver<OutgoingMessage> {
        self.outgoing_tx.subscribe()
    }

    /// Send a message directly (for use in tests or direct API responses).
    pub fn broadcast_outgoing(&self, msg: OutgoingMessage) -> Result<()> {
        let _ = self.outgoing_tx.send(msg);
        Ok(())
    }

    /// Deliver a response to the registered handler, if any.
    /// Also broadcasts for WebSocket/SSE clients.
    pub async fn deliver_response(&self, msg: OutgoingMessage) -> Result<()> {
        let msg_id = msg.id;

        // Try to deliver to a registered HTTP handler first.
        {
            let mut responses = self.responses.write().await;
            if let Some(sender) = responses.remove(&msg_id) {
                let _ = sender.send(msg.clone());
            }
        }

        // Always broadcast for WebSocket/SSE clients.
        let _ = self.outgoing_tx.send(msg);

        tracing::debug!(msg_id = %msg_id, "Delivering response");
        Ok(())
    }
}

#[async_trait]
impl Channel for WebBridge {
    fn name(&self) -> &str {
        "web"
    }

    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let internal_rx = self.incoming_rx.lock().await.take();
        let Some(mut internal_rx) = internal_rx else {
            anyhow::bail!("Web bridge already started (no receiver)");
        };
        let channel_name = self.name().to_owned();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = internal_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if tx.send((channel_name.clone(), msg)).await.is_err() {
                                    break; // Gateway receiver closed
                                }
                            }
                            None => break,
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
            tracing::info!(channel = %channel_name, "Web bridge stopped");
        });

        Ok(handle)
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // Route the response back to the waiting HTTP handler via correlation map.
        // The OutgoingMessage.id matches the original IncomingMessage.id,
        // which is the key registered by send_and_wait().
        {
            let mut responses = self.responses.write().await;
            if let Some(sender) = responses.remove(&msg.id) {
                let _ = sender.send(msg.clone());
                tracing::debug!(msg_id = %msg.id, "Correlated response to HTTP handler");
            }
        }

        // Always broadcast for WebSocket/SSE clients.
        let _ = self.outgoing_tx.send(msg);
        Ok(())
    }
}

impl std::fmt::Debug for WebBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebBridge").finish()
    }
}

/// Shared handle to the web bridge, used by route handlers.
#[derive(Debug, Clone)]
pub struct WebBridgeHandle {
    /// Sender for injecting incoming messages into the gateway pipeline.
    pub incoming_tx: mpsc::Sender<IncomingMessage>,
    /// Broadcast sender for pushing outgoing messages to WebSocket/SSE.
    pub outgoing_tx: broadcast::Sender<OutgoingMessage>,
    /// Correlation map for HTTP request-response matching.
    responses: Arc<RwLock<HashMap<uuid::Uuid, oneshot::Sender<OutgoingMessage>>>>,
}

impl WebBridgeHandle {
    /// Creates a new handle from a WebBridge.
    pub fn from_bridge(channel: &WebBridge) -> Self {
        Self {
            incoming_tx: channel.sender(),
            outgoing_tx: channel.outgoing_tx.clone(),
            responses: channel.responses.clone(),
        }
    }

    /// Subscribe to outgoing messages.
    pub fn subscribe(&self) -> broadcast::Receiver<OutgoingMessage> {
        self.outgoing_tx.subscribe()
    }

    /// Send an incoming message to the gateway pipeline.
    pub async fn send_incoming(&self, msg: IncomingMessage) -> Result<()> {
        self.incoming_tx
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Send a message and wait for a response.
    ///
    /// This registers a oneshot receiver for the response and waits for it.
    /// Used by the HTTP chat endpoint to get the orchestrator's response.
    pub async fn send_and_wait(&self, msg: IncomingMessage) -> Result<OutgoingMessage> {
        let (tx, rx) = oneshot::channel::<OutgoingMessage>();
        let msg_id = msg.id;

        // Register the response handler before sending.
        {
            let mut responses = self.responses.write().await;
            responses.insert(msg_id, tx);
        }

        // Send the message.
        self.incoming_tx
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // Wait for the response.
        rx.await
            .map_err(|e| anyhow::anyhow!("Response channel dropped: {e}"))
    }
}
