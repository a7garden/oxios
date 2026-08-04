//! Remote companion surface (RFC-044): E2EE WS for paired mobile/web clients.
#![cfg(feature = "remote")]

pub mod devices;
pub mod endpoints;
pub mod identity;
pub mod noise;
pub mod pairing;
pub mod rpc;
pub mod serve;
pub mod transport;

use std::pin::Pin;
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
/// Type alias for the per-frame handler the transport expects.
///
/// Each call returns a boxed, pinned, `Send` future so the underlying
/// closure is dyn-compatible (`run_listener` takes `H: Fn(...) -> Fut`).
/// `Box<dyn Fn>` is callable through deref coercion at the call site, so
/// we wrap the closure in a `Box` rather than an `Arc` (the handler is
/// already `Sync` because it only borrows from an `Arc<RpcCtx>`).
pub type RpcFrameHandler =
    Box<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send>> + Send + Sync>;

/// Build the per-frame RPC handler used by both production (`start()`) and
/// integration tests (the in-process E2E and the `remote_probe` example).
///
/// Keeping this single function as the canonical seam avoids a class of
/// bugs where the test handler and the production handler drift apart —
/// they MUST serialise / deserialise the exact same JSON-RPC envelope to
/// round-trip a real request.
pub fn build_rpc_handler(ctx: Arc<RpcCtx>) -> RpcFrameHandler {
    Box::new(move |frame: Vec<u8>| {
        let ctx = Arc::clone(&ctx);
        Box::pin(async move { handle_app_frame(frame, ctx).await })
            as Pin<Box<dyn Future<Output = Vec<u8>> + Send>>
    })
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

        let handler = build_rpc_handler(rpc_ctx);

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
    /// End-to-end paired-client round-trip: drive a Noise_XX handshake
    /// against the live `transport::run_listener` (with the SAME factored
    /// handler used by `RemoteRpcSurface::start`), then encrypt and
    /// exchange a `status.get` JSON-RPC request over the AEAD transport.
    /// This is the RFC-044 §12 Phase-1 acceptance: a paired client can
    /// reach the daemon over E2EE and call `status.get`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn paired_client_round_trip_status_get() {
        const NOISE_XX: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

        // 1) Server-side state: throwaway temp dir → DeviceIdentity (server
        //    static key + device_id) + DeviceRegistry. RpcCtx is what
        //    `build_rpc_handler` will hand to `handle_app_frame`.
        let state = tempfile::TempDir::new().expect("tempdir");
        let identity = DeviceIdentity::load_or_create(state.path()).expect("identity");
        let registry = DeviceRegistry::load_or_create(state.path()).expect("registry");
        let registry = Arc::new(tokio::sync::Mutex::new(registry));
        let server_device_id = identity.device_id();
        let server_static_secret = identity.snow_static().to_vec();
        let server_static_public = identity.keypair.public.clone();
        let rpc_ctx = Arc::new(RpcCtx {
            registry,
            device_id: server_device_id.clone(),
            kernel: None,
        });

        // 2) Bind a loopback listener on an ephemeral port — the test
        //    learns the bound address before handing the listener off, so
        //    the listener never has to rebind.
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral listener");
        let server_addr = listener.local_addr().expect("local_addr");

        // 3) Spawn the listener with the FACTORED handler — same code
        //    path as production.
        let shutdown = CancellationToken::new();
        let handler = build_rpc_handler(rpc_ctx);
        let server_task = tokio::spawn({
            let shutdown = shutdown.clone();
            async move {
                transport::run_listener(listener, server_static_secret, shutdown, handler).await
            }
        });

        // 4) Build the paired-client side. We model the QR-pairing offer
        //    by simply holding the server's static public key as the pin
        //    the initiator authenticates against. Phase 2 will replace
        //    this with a real device-token check; the wire-level Noise
        //    pin is identical.
        let client_kp = snow::Builder::new(NOISE_XX.parse().unwrap())
            .generate_keypair()
            .expect("client kp");
        // The XX initiator learns the server's static key ON THE WIRE
        // (in msg2) — this is the whole point of the pin: the client
        // must NOT pre-feed `remote_public_key`, otherwise we'd be
        // asserting a value we already knew. Compare to the existing
        // `noise_xx_handshake_and_transport` test (noise.rs), which
        // only sets `.local_private_key` on the initiator.
        let mut initiator = snow::Builder::new(NOISE_XX.parse().unwrap())
            .local_private_key(&client_kp.private)
            .expect("set client static")
            .build_initiator()
            .expect("build initiator");

        // 5) Open a WebSocket and drive the 3-message Noise_XX handshake
        //    over WS Binary frames. Each handshake payload is wrapped in
        //    a Noise frame and sent verbatim.
        let url = format!("ws://{server_addr}");
        let (mut client_ws, _response) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS upgrade must succeed");

        // msg1: -> e, es
        let mut msg1 = vec![0u8; 1024];
        let n = initiator
            .write_message(&[], &mut msg1)
            .expect("init write msg1");
        msg1.truncate(n);
        let frame1 = noise::encode_frame(noise::FrameType::Noise, &msg1).expect("encode msg1");
        client_ws
            .send(Message::Binary(frame1))
            .await
            .expect("send msg1");

        // Recv msg2: <- e, ee, s, es
        let msg2_msg = tokio::time::timeout(Duration::from_secs(5), client_ws.next())
            .await
            .expect("server must reply msg2 within 5s")
            .expect("server closed early")
            .expect("ws err on msg2");
        let msg2_bytes = msg2_msg.into_data();
        let (ft2, msg2) = noise::decode_frame(&msg2_bytes).expect("decode msg2");
        assert_eq!(ft2, noise::FrameType::Noise, "msg2 must be Noise frame");
        let mut buf2 = [0u8; 1024];
        initiator
            .read_message(msg2, &mut buf2)
            .expect("init read msg2");

        // PIN VERIFICATION: the XX initiator learns the server's static
        // public key on the wire in msg2. We assert it byte-equals the
        // identity's public key — without this, an AEAD round-trip
        // would only prove "encryption works with someone", not that
        // "we reached the *pinned* daemon". This is the whole point of
        // the QR pairing offer's `public_key_b64` field.
        let learned_static = initiator
            .get_remote_static()
            .expect("XX initiator must know the server static after msg2");
        assert_eq!(
            learned_static,
            &server_static_public[..],
            "server static learned in XX must equal the identity's public key (the pairing pin)"
        );

        // msg3: -> s, se
        let mut msg3 = vec![0u8; 1024];
        let n = initiator
            .write_message(&[], &mut msg3)
            .expect("init write msg3");
        msg3.truncate(n);
        let frame3 = noise::encode_frame(noise::FrameType::Noise, &msg3).expect("encode msg3");
        client_ws
            .send(Message::Binary(frame3))
            .await
            .expect("send msg3");

        // XX has 3 messages; both sides now switch to transport mode.
        assert!(
            initiator.is_handshake_finished(),
            "initiator must consider XX finished after msg3"
        );
        let mut client_transport = noise::Transport::from_snow_state(
            initiator.into_transport_mode().expect("init -> transport"),
        );

        // 6) Encrypt a `status.get` JSON-RPC request, send as App frame.
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "status.get",
        });
        let request_bytes = serde_json::to_vec(&request).expect("encode request");
        let ciphertext = client_transport
            .encrypt(&request_bytes)
            .expect("encrypt req");
        let app_frame =
            noise::encode_frame(noise::FrameType::App, &ciphertext).expect("encode app frame");
        client_ws
            .send(Message::Binary(app_frame))
            .await
            .expect("send app");

        // 7) Recv the encrypted reply, decrypt with our client transport,
        //    parse JSON, assert protocol_version + device_id match.
        let reply_msg = tokio::time::timeout(Duration::from_secs(5), client_ws.next())
            .await
            .expect("server must reply within 5s")
            .expect("server closed")
            .expect("ws err on reply");
        let reply_bytes = reply_msg.into_data();
        let (ft, payload) = noise::decode_frame(&reply_bytes).expect("decode reply");
        assert_eq!(ft, noise::FrameType::App, "reply must be App frame");
        let plaintext = client_transport
            .decrypt(payload)
            .expect("decrypt reply (proves E2EE: only the paired Noise peer can produce this)");
        let resp: Value = serde_json::from_slice(&plaintext).expect("parse JSON-RPC reply");

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], json!(1));
        let result = &resp["result"];
        assert_eq!(
            result["protocol_version"],
            json!(rpc::PROTOCOL_VERSION),
            "status.get must echo PROTOCOL_VERSION=1"
        );
        assert_eq!(
            result["min_client_version"],
            json!(rpc::MIN_CLIENT_VERSION),
            "status.get must echo MIN_CLIENT_VERSION=1"
        );
        assert_eq!(
            result["device_id"],
            json!(server_device_id),
            "status.get device_id must match the server's identity (proves the wire reached the right daemon)"
        );
        assert_eq!(
            result["paired_count"],
            json!(0),
            "paired_count must be 0 in a fresh registry"
        );

        // 8) Clean shutdown — drop the WS, cancel the token, join.
        drop(client_ws);
        shutdown.cancel();
        let outcome = tokio::time::timeout(Duration::from_secs(5), server_task)
            .await
            .expect("server task must exit within 5s")
            .expect("server task joined Ok");
        outcome.expect("run_listener returns Ok on clean shutdown");
    }
}
