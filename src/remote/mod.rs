//! Remote companion surface (RFC-044): E2EE WS for paired mobile/web clients.
//!
//! Phase 2 adds: device-token verification, full RPC method set
//! (session/persona/chat/subscriptions), RemoteBridge channel for gateway
//! routing, and server-pushed subscription frames.
#![cfg(feature = "remote")]

pub mod devices;
pub mod endpoints;
pub mod identity;
pub mod noise;
pub mod pairing;
pub mod rpc;
pub mod serve;
pub mod transport;

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use oxios_gateway::surface::{Surface, SurfaceContext, SurfaceHandle};
use oxios_gateway::{Channel, GatewayInbox};
use oxios_kernel::event_bus::KernelEvent;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use crate::remote::devices::DeviceRegistry;
use crate::remote::identity::DeviceIdentity;
use crate::remote::rpc::{RpcCtx, RpcError, SubscriptionKind, dispatch};
use crate::remote::transport::ConnectionCtx;

// ── RemoteBridge: gateway channel for chat routing ─────────────────────────

/// Adapter bridging the remote companion surface with the gateway pipeline.
///
/// Mirrors `WebBridge` — incoming messages are pumped to the gateway;
/// outgoing responses are correlated by message id and delivered to waiting
/// `chat.send` callers via a oneshot channel.
pub struct RemoteBridge {
    incoming_rx: Mutex<Option<mpsc::Receiver<IncomingMessage>>>,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    responses: Arc<RwLock<HashMap<uuid::Uuid, oneshot::Sender<OutgoingMessage>>>>,
}

impl RemoteBridge {
    /// Create a new remote bridge with a bounded incoming buffer.
    pub fn new(buffer: usize) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::channel(buffer);
        Self {
            incoming_rx: Mutex::new(Some(incoming_rx)),
            incoming_tx,
            responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Build a cloneable handle for use by RPC dispatch (`chat.send`).
    pub fn handle(&self) -> RemoteBridgeHandle {
        RemoteBridgeHandle {
            incoming_tx: self.incoming_tx.clone(),
            responses: Arc::clone(&self.responses),
        }
    }
}

#[async_trait::async_trait]
impl Channel for RemoteBridge {
    fn name(&self) -> &str {
        "remote"
    }

    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let internal_rx = self.incoming_rx.lock().await.take();
        let Some(mut internal_rx) = internal_rx else {
            anyhow::bail!("Remote bridge already started (no receiver)");
        };
        let channel_name = self.name().to_owned();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = internal_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if tx.send((channel_name.clone(), msg)).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
            tracing::info!(channel = %channel_name, "Remote bridge stopped");
        });

        Ok(handle)
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let msg_id = msg.id;
        {
            let mut responses = self.responses.write().await;
            if let Some(sender) = responses.remove(&msg_id) {
                let _ = sender.send(msg.clone());
            }
        }
        Ok(())
    }
}

/// Cloneable handle to the remote bridge, used by RPC dispatch for `chat.send`.
#[derive(Clone)]
pub struct RemoteBridgeHandle {
    incoming_tx: mpsc::Sender<IncomingMessage>,
    responses: Arc<RwLock<HashMap<uuid::Uuid, oneshot::Sender<OutgoingMessage>>>>,
}

impl rpc::GatewayBridge for RemoteBridgeHandle {
    fn send_and_wait(
        &self,
        content: String,
        session_id: Option<String>,
        persona_id: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        let incoming_tx = self.incoming_tx.clone();
        let responses = Arc::clone(&self.responses);

        Box::pin(async move {
            let mut msg = IncomingMessage::new("remote", "companion", &content);
            let id = msg.id;
            if let Some(sid) = session_id {
                msg.metadata.insert("session_id".to_string(), sid);
            }
            if let Some(pid) = persona_id {
                msg.metadata.insert("persona_id".to_string(), pid);
            }

            let (tx, rx) = oneshot::channel();
            responses.write().await.insert(id, tx);

            incoming_tx
                .send(msg)
                .await
                .map_err(|_| "gateway send failed".to_string())?;

            let response = tokio::time::timeout(Duration::from_secs(120), rx)
                .await
                .map_err(|_| "gateway timeout (120s)".to_string())?
                .map_err(|_| "gateway response dropped".to_string())?;
            Ok(response.content)
        })
    }

    fn send_fire_and_forget(
        &self,
        content: String,
        persona_id: Option<String>,
        metadata: HashMap<String, String>,
    ) -> Result<(), String> {
        let mut msg = IncomingMessage::new("remote", "companion", &content);
        if let Some(pid) = persona_id {
            msg.metadata.insert("persona_id".to_string(), pid);
        }
        for (k, v) in metadata {
            msg.metadata.insert(k, v);
        }
        self.incoming_tx
            .try_send(msg)
            .map_err(|e| format!("gateway send failed: {e}"))
    }
}

// ── Surface ───────────────────────────────────────────────────────────────

/// Kernel-connected E2EE companion surface.
pub struct RemoteRpcSurface;

impl RemoteRpcSurface {
    pub fn new() -> Self {
        Self
    }
}
impl Default for RemoteRpcSurface {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the per-request RPC context shared by every transport invocation.
fn build_rpc_ctx(
    identity: &DeviceIdentity,
    registry: Arc<Mutex<DeviceRegistry>>,
    kernel: Option<Arc<oxios_kernel::KernelHandle>>,
    bridge: Option<Arc<dyn rpc::GatewayBridge>>,
) -> RpcCtx {
    RpcCtx {
        registry,
        device_id: identity.device_id(),
        kernel,
        bridge,
    }
}

// ── RPC frame handling ────────────────────────────────────────────────────

/// Render the wire JSON-RPC 2.0 envelope for a successful `Resp` payload.
fn render_success(id: &Value, result: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .unwrap_or_else(|_| b"{}".to_vec())
}

/// Render the wire JSON-RPC 2.0 envelope for a structured RPC error.
fn render_error(id: &Value, error: RpcError) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": error.code, "message": error.message },
    }))
    .unwrap_or_else(|_| b"{}".to_vec())
}

/// Deserialize + dispatch + serialize one application-frame request.
///
/// For `RpcOutcome::Stream`, sends the subscription response, then spawns
/// a push task that streams events to the client via `conn_ctx.push_tx`.
async fn handle_app_frame(frame: Vec<u8>, ctx: Arc<RpcCtx>, conn: Arc<ConnectionCtx>) -> Vec<u8> {
    let request: Value = match serde_json::from_slice(&frame) {
        Ok(value) => value,
        Err(error) => {
            return render_error(
                &Value::Null,
                RpcError::new(rpc::RPC_INVALID_REQUEST, format!("malformed JSON: {error}")),
            );
        }
    };

    let id = request.get("id").cloned().unwrap_or(Value::Null);

    // RFC-044 §6.7: audit every RPC method invocation. Metadata only — the
    // payload is already inside the Noise session, so the trail never sees
    // plaintext content, just the method name and the authenticated actor.
    if let Some(kernel) = ctx.kernel.as_ref() {
        let actor = conn
            .authenticated_device()
            .unwrap_or_else(|| "anonymous".to_string());
        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        kernel.security.audit(
            &actor,
            oxios_kernel::AuditAction::Other {
                detail: format!("remote_rpc:{method}"),
            },
            "remote",
        );
    }
    match dispatch(request, &ctx, &conn).await {
        Ok(rpc::RpcOutcome::Resp(value)) => render_success(&id, value),
        Ok(rpc::RpcOutcome::Stream {
            subscription_id,
            kind,
        }) => {
            // Send the subscription confirmation to the client.
            let confirm = render_success(
                &id,
                json!({ "subscription_id": subscription_id, "subscribed": true }),
            );

            // Spawn the push task for this subscription.
            spawn_subscription(kind, subscription_id, ctx.clone(), conn);

            confirm
        }
        Err(error) => render_error(&id, error),
    }
}

/// Spawn a background task that pushes kernel events to the companion.
fn spawn_subscription(
    kind: SubscriptionKind,
    subscription_id: String,
    ctx: Arc<RpcCtx>,
    conn: Arc<ConnectionCtx>,
) {
    let Some(kernel) = ctx.kernel.clone() else {
        return;
    };
    let conn_for_task = Arc::clone(&conn);

    match kind {
        SubscriptionKind::Chat { session_id } => {
            tokio::spawn(async move {
                let mut rx = kernel.infra.subscribe();
                tracing::info!(%subscription_id, %session_id, "chat subscription started");
                loop {
                    tokio::select! {
                        biased;
                        Ok(event) = rx.recv() => {
                            if !event_matches_session(&event, &session_id) {
                                continue;
                            }
                            let notification = json!({
                                "jsonrpc": "2.0",
                                "method": "chat.event",
                                "params": {
                                    "subscription_id": &subscription_id,
                                    "event": &event,
                                }
                            });
                            let bytes = serde_json::to_vec(&notification).unwrap_or_default();
                            if conn_for_task.try_push(bytes).is_err() {
                                break; // Connection closed or queue full
                            }
                        }
                        else => break,
                    }
                }
                tracing::info!(%subscription_id, "chat subscription ended");
            });
        }
        SubscriptionKind::AgentStatus => {
            tokio::spawn(async move {
                let mut rx = kernel.infra.subscribe();
                tracing::info!(%subscription_id, "agent.status subscription started");
                loop {
                    tokio::select! {
                        biased;
                        Ok(event) = rx.recv() => {
                            if !is_agent_lifecycle_event(&event) {
                                continue;
                            }
                            let notification = json!({
                                "jsonrpc": "2.0",
                                "method": "agent.event",
                                "params": {
                                    "subscription_id": &subscription_id,
                                    "event": &event,
                                }
                            });
                            let bytes = serde_json::to_vec(&notification).unwrap_or_default();
                            if conn_for_task.try_push(bytes).is_err() {
                                break;
                            }
                        }
                        else => break,
                    }
                }
                tracing::info!(%subscription_id, "agent.status subscription ended");
            });
        }
    }
}

/// Check if a kernel event belongs to the given session.
fn event_matches_session(event: &KernelEvent, session_id: &str) -> bool {
    match event {
        KernelEvent::ToolExecutionStarted {
            session_id: sid, ..
        }
        | KernelEvent::ToolExecutionFinished {
            session_id: sid, ..
        }
        | KernelEvent::AgentOutput {
            session_id: sid, ..
        } => sid == session_id,
        KernelEvent::ApprovalRequested {
            session_id: Some(sid),
            ..
        }
        | KernelEvent::PathAccessRequested {
            session_id: Some(sid),
            ..
        } => sid == session_id,
        _ => false,
    }
}

/// Check if a kernel event is an agent lifecycle event.
fn is_agent_lifecycle_event(event: &KernelEvent) -> bool {
    matches!(
        event,
        KernelEvent::AgentCreated { .. }
            | KernelEvent::AgentStarted { .. }
            | KernelEvent::AgentStopped { .. }
            | KernelEvent::AgentFailed { .. }
    )
}

/// Type alias for the per-frame handler the transport expects.
pub type RpcFrameHandler = Box<
    dyn Fn(Vec<u8>, Arc<ConnectionCtx>) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send>>
        + Send
        + Sync,
>;

/// Build the per-frame RPC handler used by both production and tests.
pub fn build_rpc_handler(ctx: Arc<RpcCtx>) -> RpcFrameHandler {
    Box::new(move |frame: Vec<u8>, conn: Arc<ConnectionCtx>| {
        let ctx = Arc::clone(&ctx);
        Box::pin(async move { handle_app_frame(frame, ctx, conn).await })
            as Pin<Box<dyn Future<Output = Vec<u8>> + Send>>
    })
}

/// True iff the bind host is a loopback address (`127.0.0.1`, `::1`, or `localhost`).
/// Used to decide whether to emit a security warning when widening the bind.
fn is_loopback_bind(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

/// Companion session audit hook backed by the kernel's AuditTrail
/// (RFC-044 §6.7). Logs connect/disconnect events with metadata only — the
/// payload is inside the Noise session.
struct KernelCompanionAudit {
    kernel: Arc<oxios_kernel::KernelHandle>,
}

impl transport::CompanionAudit for KernelCompanionAudit {
    fn on_connect(&self, peer: SocketAddr) {
        self.kernel.security.audit(
            "companion",
            oxios_kernel::AuditAction::Other {
                detail: format!("connect:{peer}"),
            },
            "remote",
        );
    }

    fn on_disconnect(&self, peer: SocketAddr, device_id: Option<String>) {
        let actor = device_id.as_deref().unwrap_or("anonymous");
        self.kernel.security.audit(
            actor,
            oxios_kernel::AuditAction::Other {
                detail: format!("disconnect:{peer}"),
            },
            "remote",
        );
    }
}
#[async_trait::async_trait]
impl Surface for RemoteRpcSurface {
    fn name(&self) -> &str {
        "remote"
    }

    async fn start(&self, ctx: SurfaceContext) -> Result<SurfaceHandle> {
        if !ctx.config.read().remote.enabled {
            tracing::info!("RemoteRpcSurface disabled by config; not binding listener");
            return Ok(SurfaceHandle {
                channel: None,
                tasks: Vec::new(),
            });
        }

        let port = ctx.config.read().remote.port;
        let bind_host = ctx.config.read().remote.bind_address.clone();
        // IPv6 hosts must be bracketed (`[::1]:6768`); IPv4/hostname don't need it.
        let bind_str = if bind_host.contains(':') {
            format!("[{bind_host}]:{port}")
        } else {
            format!("{bind_host}:{port}")
        };
        let bind_addr: std::net::SocketAddr = bind_str
            .parse()
            .with_context(|| format!("invalid remote bind {bind_str}"))?;
        if !is_loopback_bind(&bind_host) {
            tracing::warn!(
                bind = %bind_addr,
                "RemoteRpcSurface binding to a non-loopback address; the E2EE WS \
                 listener is reachable from the network. Companion auth is enforced \
                 (Noise_XX + device token) but you must trust the network path."
            );
        }

        let workspace = ctx.config.read().kernel.workspace.clone();
        let state_dir = std::path::PathBuf::from(&workspace).join("state");
        std::fs::create_dir_all(&state_dir)
            .with_context(|| format!("create remote state dir {}", state_dir.display()))?;

        let identity = DeviceIdentity::load_or_create(&state_dir)
            .with_context(|| format!("load remote identity from {}", state_dir.display()))?;
        let registry = DeviceRegistry::load_or_create(&state_dir)
            .with_context(|| format!("load remote registry from {}", state_dir.display()))?;
        let registry = Arc::new(Mutex::new(registry));

        tracing::info!(
            device_id = %identity.device_id(),
            state_dir = %state_dir.display(),
            "remote identity + registry loaded"
        );

        // Create the gateway bridge for chat routing.
        let bridge = RemoteBridge::new(256);
        let bridge_handle = Arc::new(bridge.handle()) as Arc<dyn rpc::GatewayBridge>;

        let rpc_ctx = Arc::new(build_rpc_ctx(
            &identity,
            registry,
            Some(ctx.kernel.clone()),
            Some(bridge_handle),
        ));

        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("bind remote WebSocket listener at {bind_addr}"))?;
        let local_addr = listener
            .local_addr()
            .context("remote WebSocket listener local_addr")?;
        tracing::info!(%local_addr, "RemoteRpcSurface listener bound");

        let handler = build_rpc_handler(rpc_ctx);

        let shutdown: CancellationToken = ctx.shutdown.clone();
        let server_static = identity.snow_static().to_vec();
        let audit: Option<Arc<dyn transport::CompanionAudit>> =
            Some(Arc::new(KernelCompanionAudit {
                kernel: Arc::clone(&ctx.kernel),
            }));

        let handle = tokio::spawn(async move {
            if let Err(error) =
                transport::run_listener(listener, server_static, shutdown, handler, audit).await
            {
                tracing::error!(%error, "RemoteRpcSurface listener terminated");
            }
        });

        Ok(SurfaceHandle {
            channel: Some(Box::new(bridge)),
            tasks: vec![handle],
        })
    }
}

// ── Test helpers ──────────────────────────────────────────────────────────

/// Drive `transport::run_listener` against a pre-bound `TcpListener`
/// without requiring a full `KernelHandle`.
#[cfg(test)]
pub(crate) async fn run_for_test(
    listener: TcpListener,
    server_static: Vec<u8>,
    shutdown: CancellationToken,
) -> Result<()> {
    transport::run_listener(
        listener,
        server_static,
        shutdown,
        |_frame, _conn| async move { Vec::new() },
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::noise::{FrameType, Transport, decode_frame, encode_frame};

    const NOISE_XX: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
    /// WebSocket type returned by `connect_async` (client side).
    type ClientWs = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    /// Full Noise_XX initiator that mirrors what a companion client does:
    /// write msg1, read msg2, write msg3, then enter transport mode.
    struct ClientSession {
        transport: Transport,
    }

    impl ClientSession {
        async fn connect(socket: &mut ClientWs) -> Result<Self> {
            use futures::{SinkExt, StreamExt};

            let client_kp = snow::Builder::new(NOISE_XX.parse().unwrap())
                .generate_keypair()
                .unwrap();
            let mut initiator = snow::Builder::new(NOISE_XX.parse().unwrap())
                .local_private_key(&client_kp.private)
                .unwrap()
                .build_initiator()
                .unwrap();

            // msg1: -> e
            let mut buf = vec![0u8; 1024];
            let n = initiator.write_message(&[], &mut buf).unwrap();
            buf.truncate(n);
            let frame = encode_frame(FrameType::Noise, &buf).unwrap();
            socket
                .send(tokio_tungstenite::tungstenite::Message::Binary(frame))
                .await?;

            // msg2: <- e, ee, s, es
            let msg = socket.next().await.unwrap().unwrap();
            let data = msg.into_data();
            let (_, payload) = decode_frame(&data).unwrap();
            let mut tmp = [0u8; 65536];
            initiator.read_message(payload, &mut tmp).unwrap();

            // msg3: -> s, se
            let mut buf = vec![0u8; 1024];
            let n = initiator.write_message(&[], &mut buf).unwrap();
            buf.truncate(n);
            let frame = encode_frame(FrameType::Noise, &buf).unwrap();
            socket
                .send(tokio_tungstenite::tungstenite::Message::Binary(frame))
                .await?;

            let ts = initiator.into_transport_mode().unwrap();
            Ok(Self {
                transport: Transport::from_snow_state(ts),
            })
        }

        async fn send_rpc(&mut self, socket: &mut ClientWs, request: &Value) -> Result<Value> {
            use futures::{SinkExt, StreamExt};
            let plaintext = serde_json::to_vec(request)?;
            let ciphertext = self.transport.encrypt(&plaintext)?;
            let frame = encode_frame(FrameType::App, &ciphertext).unwrap();
            socket
                .send(tokio_tungstenite::tungstenite::Message::Binary(frame))
                .await?;
            let msg = socket.next().await.unwrap().unwrap();
            let data = msg.into_data();
            let (_, payload) = decode_frame(&data).unwrap();
            let plaintext = self.transport.decrypt(payload)?;
            Ok(serde_json::from_slice(&plaintext)?)
        }
    }

    /// Helper: start a listener with a real RPC handler (no kernel), connect
    /// a client over Noise_XX, authenticate, then run assertions.
    async fn setup_paired_listener() -> (
        tokio::task::JoinHandle<()>,
        CancellationToken,
        String,
        String,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let identity = DeviceIdentity::load_or_create(tmp.path()).unwrap();
        let registry = DeviceRegistry::load_or_create(tmp.path()).unwrap();
        let registry = Arc::new(Mutex::new(registry));
        let (device_id, token) = {
            let mut reg = registry.lock().await;
            reg.pair("test-device", "mobile").unwrap()
        };

        let ctx = Arc::new(RpcCtx {
            registry,
            device_id: identity.device_id(),
            kernel: None,
            bridge: None,
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = CancellationToken::new();
        let handler = build_rpc_handler(ctx);
        let server_static = identity.snow_static().to_vec();
        let shutdown_clone = shutdown.clone();

        let handle = tokio::spawn(async move {
            transport::run_listener(listener, server_static, shutdown_clone, handler, None)
                .await
                .unwrap();
        });

        // Give the listener a moment to start.
        tokio::time::sleep(Duration::from_millis(50)).await;

        (
            handle,
            shutdown,
            addr.to_string(),
            format!("{device_id}:{token}"),
        )
    }

    #[tokio::test]
    async fn paired_client_round_trip_status_get() {
        let (handle, shutdown, addr, _) = setup_paired_listener().await;
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
            .await
            .unwrap();
        let mut client = ClientSession::connect(&mut socket).await.unwrap();
        let req = json!({"jsonrpc":"2.0","id":1,"method":"status.get"});
        let response = client.send_rpc(&mut socket, &req).await.unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert_eq!(
            response["result"]["protocol_version"],
            rpc::PROTOCOL_VERSION
        );
        shutdown.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn auth_verify_then_session_list() {
        let (handle, shutdown, addr, creds) = setup_paired_listener().await;
        let (device_id, token) = creds.split_once(':').unwrap();
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
            .await
            .unwrap();
        let mut client = ClientSession::connect(&mut socket).await.unwrap();

        let req = json!({"jsonrpc":"2.0","id":1,"method":"session.list"});
        let resp = client.send_rpc(&mut socket, &req).await.unwrap();
        assert_eq!(resp["error"]["code"], rpc::RPC_AUTH_REQUIRED);

        let auth_req = json!({
            "jsonrpc": "2.0", "id": 2,
            "method": "auth.verify",
            "params": { "device_id": device_id, "token": token }
        });
        let resp = client.send_rpc(&mut socket, &auth_req).await.unwrap();
        assert_eq!(resp["result"]["verified"], true);

        let req = json!({"jsonrpc":"2.0","id":3,"method":"session.list"});
        let resp = client.send_rpc(&mut socket, &req).await.unwrap();
        assert_eq!(resp["error"]["code"], rpc::RPC_INTERNAL_ERROR);

        shutdown.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn bad_token_rejected() {
        let (handle, shutdown, addr, _) = setup_paired_listener().await;
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
            .await
            .unwrap();
        let mut client = ClientSession::connect(&mut socket).await.unwrap();

        let req = json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "auth.verify",
            "params": { "device_id": "bogus", "token": "wrong" }
        });
        let resp = client.send_rpc(&mut socket, &req).await.unwrap();
        assert_eq!(resp["error"]["code"], rpc::RPC_AUTH_FAILED);

        shutdown.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn plaintext_is_refused_with_policy_close() {
        use futures::{SinkExt, StreamExt};
        let tmp = tempfile::tempdir().unwrap();
        let identity = DeviceIdentity::load_or_create(tmp.path()).unwrap();
        let server_static = identity.snow_static().to_vec();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = CancellationToken::new();
        let shutdown_clone = shutdown.clone();
        let ss = server_static.clone();

        let handle = tokio::spawn(async move {
            let _ = run_for_test(listener, ss, shutdown_clone).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
            .await
            .unwrap();

        // Send plaintext (not a Noise frame) → should be refused.
        socket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                b"plaintext hello".to_vec(),
            ))
            .await
            .unwrap();

        let msg = socket.next().await.unwrap().unwrap();
        match msg {
            tokio_tungstenite::tungstenite::Message::Close(Some(frame)) => {
                assert_eq!(
                    frame.code,
                    tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Policy
                );
            }
            other => panic!("expected Policy close, got {other:?}"),
        }

        shutdown.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn chat_subscribe_returns_subscription_id() {
        let (handle, shutdown, addr, creds) = setup_paired_listener().await;
        let (device_id, token) = creds.split_once(':').unwrap();

        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
            .await
            .unwrap();
        let mut client = ClientSession::connect(&mut socket).await.unwrap();

        // Auth first
        let auth_req = json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "auth.verify",
            "params": { "device_id": device_id, "token": token }
        });
        client.send_rpc(&mut socket, &auth_req).await.unwrap();

        // chat.subscribe
        let sub_req = json!({
            "jsonrpc": "2.0", "id": 2,
            "method": "chat.subscribe",
            "params": { "session_id": "test-session" }
        });
        let resp = client.send_rpc(&mut socket, &sub_req).await.unwrap();

        assert_eq!(resp["result"]["subscribed"], true);
        assert!(
            resp["result"]["subscription_id"]
                .as_str()
                .unwrap()
                .starts_with("chat_")
        );

        shutdown.cancel();
        let _ = handle.await;
    }
}
