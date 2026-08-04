//! Remote companion surface (RFC-044): E2EE WS for paired mobile/web clients.
#![cfg(feature = "remote")]

pub mod devices;
pub mod endpoints;
pub mod identity;
pub mod noise;
pub mod pairing;
pub mod rpc;
pub mod transport;

use std::sync::Arc;

use anyhow::{Context, Result};
use oxios_gateway::surface::{Surface, SurfaceContext, SurfaceHandle};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::remote::devices::DeviceRegistry;
use crate::remote::identity::DeviceIdentity;
use crate::remote::rpc::{RpcCtx, RpcError, dispatch};

/// Kernel-connected E2EE companion surface.
pub struct RemoteRpcSurface;

impl RemoteRpcSurface {
    /// Create a new remote surface instance.
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
    registry: Arc<tokio::sync::Mutex<DeviceRegistry>>,
    kernel: Option<Arc<oxios_kernel::KernelHandle>>,
) -> RpcCtx {
    RpcCtx {
        registry,
        device_id: identity.device_id(),
        kernel,
    }
}

/// Render the wire JSON-RPC 2.0 envelope for a successful `Resp` payload.
///
/// The transport already manages encryption + framing; this function only
/// assembles `{"jsonrpc":"2.0","id":..,"result":..}` so the handler can be a
/// simple `Fn(Vec<u8>) -> Vec<u8>`.
fn render_success(id: &Value, result: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .unwrap_or_else(|_| {
        // Last-resort fallback: empty object. Serializing our own Value can
        // only fail on exotic map keys, which we never emit.
        b"{}".to_vec()
    })
}

/// Render the wire JSON-RPC 2.0 envelope for a structured RPC error.
fn render_error(id: &Value, error: RpcError) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": error.code,
            "message": error.message,
        },
    }))
    .unwrap_or_else(|_| b"{}".to_vec())
}

/// Deserialize + dispatch + serialize one application-frame request.
///
/// `id` falls back to `Value::Null` when the request omits it (JSON-RPC 2.0
/// permits this for notifications, but Phase 1 always echoes whatever we got).
async fn handle_app_frame(frame: Vec<u8>, ctx: Arc<RpcCtx>) -> Vec<u8> {
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

    match dispatch(request, &ctx).await {
        Ok(rpc::RpcOutcome::Resp(value)) => render_success(&id, value),
        Ok(rpc::RpcOutcome::Stream(_)) => {
            // Phase 1 advertises no streaming methods; reaching this branch
            // means dispatch returned an outcome we cannot serialise. Surface
            // it as an internal error rather than silently dropping the frame.
            render_error(
                &id,
                RpcError::new(
                    rpc::RPC_INTERNAL_ERROR,
                    "streaming not supported in Phase 1",
                ),
            )
        }
        Err(error) => render_error(&id, error),
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
        let bind_addr: std::net::SocketAddr = format!("127.0.0.1:{port}")
            .parse()
            .with_context(|| format!("invalid remote port {port}"))?;

        let workspace = ctx.config.read().kernel.workspace.clone();
        let state_dir = std::path::PathBuf::from(&workspace).join("state");
        std::fs::create_dir_all(&state_dir)
            .with_context(|| format!("create remote state dir {}", state_dir.display()))?;

        let identity = DeviceIdentity::load_or_create(&state_dir)
            .with_context(|| format!("load remote identity from {}", state_dir.display()))?;
        let registry = DeviceRegistry::load_or_create(&state_dir)
            .with_context(|| format!("load remote registry from {}", state_dir.display()))?;
        let registry = Arc::new(tokio::sync::Mutex::new(registry));

        tracing::info!(
            device_id = %identity.device_id(),
            state_dir = %state_dir.display(),
            "remote identity + registry loaded"
        );

        let rpc_ctx = Arc::new(build_rpc_ctx(&identity, registry, Some(ctx.kernel.clone())));

        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("bind remote WebSocket listener at {bind_addr}"))?;
        let local_addr = listener
            .local_addr()
            .context("remote WebSocket listener local_addr")?;
        tracing::info!(%local_addr, "RemoteRpcSurface listener bound");

        let handler = {
            let rpc_ctx = Arc::clone(&rpc_ctx);
            move |frame: Vec<u8>| {
                let rpc_ctx = Arc::clone(&rpc_ctx);
                async move { handle_app_frame(frame, rpc_ctx).await }
            }
        };

        let shutdown: CancellationToken = ctx.shutdown.clone();
        let server_static = identity.snow_static().to_vec();

        let handle = tokio::spawn(async move {
            if let Err(error) =
                transport::run_listener(listener, server_static, shutdown, handler).await
            {
                tracing::error!(%error, "RemoteRpcSurface listener terminated");
            }
        });

        Ok(SurfaceHandle {
            channel: None,
            tasks: vec![handle],
        })
    }
}

/// Helper used by the unit test in this module: drive `transport::run_listener`
/// against a pre-bound `TcpListener` without requiring a full `KernelHandle`.
///
/// Production wiring goes through `Surface::start` above; this seam exists so
/// the plaintext-refusal property can be exercised cheaply.
#[cfg(test)]
pub(crate) async fn run_for_test(
    listener: TcpListener,
    server_static: Vec<u8>,
    shutdown: CancellationToken,
) -> Result<()> {
    transport::run_listener(listener, server_static, shutdown, |_frame| async move {
        // Stub handler — should never be reached in the plaintext-refusal test.
        Vec::new()
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{
        Message,
        protocol::{CloseFrame, frame::coding::CloseCode},
    };

    /// Connect to `addr`, send a single plaintext junk frame, and assert that
    /// the server replies with a Policy close (1008). Mirrors the
    /// plaintext-refusal acceptance criterion for Task 9.
    #[tokio::test]
    async fn plaintext_is_refused_with_policy_close() {
        // Listener binds on loopback ephemeral so the test is hermetic.
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral listener");
        let local = listener.local_addr().expect("local_addr");

        // Use any 32-byte blob for the Noise static key — the handshake is
        // never reached because the very first frame is plaintext.
        let server_static = vec![0xAAu8; 32];

        let shutdown = CancellationToken::new();
        let listener_task = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { run_for_test(listener, server_static, shutdown).await }
        });

        // Give the accept loop a tick to start polling.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let url = format!("ws://{local}");
        let (mut client, _response) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WebSocket handshake must succeed (plaintext refusal is post-accept)");

        // Send a plaintext Binary frame whose byte does NOT start with a
        // valid Noise frame header. 0xFF is never a frame-type byte (Noise=1,
        // App=2, Ping=3, Pong=4, Close=5), so the server will call
        // `refuse_plaintext` and reply with a Policy close.
        client
            .send(Message::Binary(vec![0xFF, 0x00, 0x00, 0x00]))
            .await
            .expect("send plaintext junk frame");

        let close = tokio::time::timeout(Duration::from_secs(5), client.next())
            .await
            .expect("server must close within 5s")
            .expect("server closed the stream")
            .expect("server close is Ok");

        let Message::Close(Some(CloseFrame { code, .. })) = close else {
            panic!("server must reply with a Close frame, got: {close:?}");
        };
        assert_eq!(
            code,
            CloseCode::Policy,
            "plaintext must be refused with code 1008 (Policy), got {code:?}"
        );

        shutdown.cancel();
        let _ = listener_task.await;
    }

    /// Sanity: the listener stays bound until the shutdown token fires.
    /// (Quick guard so the plaintext-refusal test above cannot pass for the
    /// wrong reason — e.g. listener crashed and dropped the connection.)
    #[tokio::test]
    async fn listener_shuts_down_on_cancellation() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral listener");
        let server_static = vec![0xBBu8; 32];
        let shutdown = CancellationToken::new();

        let task = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { run_for_test(listener, server_static, shutdown).await }
        });

        // Allow the listener to start.
        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown.cancel();
        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("listener task must exit within 5s of cancel")
            .expect("listener task joined Ok");
        outcome.expect("run_listener returns Ok on clean shutdown");
    }
}
