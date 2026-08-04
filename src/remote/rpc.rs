//! JSON-RPC 2.0 dispatch for the remote companion surface (RFC-044 §6.5).
//!
//! Phase 1 supports `status.get` and `echo`. Subscriptions (`RpcOutcome::Stream`)
//! are reserved for Phase 2.
//!
//! Tests live in `mod tests` below and build a real `DeviceRegistry` backed by a
//! `tempfile::TempDir`. A live `KernelHandle` is impractical for unit tests, so
//! `RpcCtx::kernel` is `Option<Arc<…>>`; `status.get` never touches the kernel.
//!
//! Symbols are intentionally forward-declared; `#[allow(dead_code)]` silences
//! the warnings until the surface wiring (Task 9) lands.
#![allow(dead_code)]
use anyhow::Result;
use serde_json::{Value, json};
use std::sync::Arc;
use thiserror::Error;

use crate::remote::devices::DeviceRegistry;

/// JSON-RPC 2.0 error codes used by the remote companion surface.
///
/// Only the standard `method not found` is encoded as a structured variant
/// here; every other failure surfaces a generic `-32603 internal error` with
/// the underlying message attached.
pub const RPC_METHOD_NOT_FOUND: i32 = -32601;
pub const RPC_INVALID_REQUEST: i32 = -32600;
pub const RPC_INTERNAL_ERROR: i32 = -32603;

/// Structured RPC error that the transport can render directly on the wire.
///
/// `code` follows the JSON-RPC 2.0 pre-defined error code space; `message` is
/// the human-readable explanation suitable for inclusion in the wire
/// `error.message` field. The transport serialises this as
/// `{"jsonrpc":"2.0","id":<id>,"error":{"code":..,"message":..}}`.
#[derive(Debug, Clone, Error)]
#[error("rpc error {code}: {message}")]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(RPC_METHOD_NOT_FOUND, format!("method not found: {method}"))
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(RPC_INVALID_REQUEST, message)
    }
}

/// Wire-protocol version of the companion RPC surface. Bump together with
/// `MIN_CLIENT_VERSION` when the contract changes.
pub const PROTOCOL_VERSION: u32 = 1;

/// Oldest client protocol version still accepted by this build. Clients
/// advertising `protocol_version < MIN_CLIENT_VERSION` must upgrade.
pub const MIN_CLIENT_VERSION: u32 = 1;

/// Outcome of dispatching a single JSON-RPC request.
///
/// Phase 1 only produces `Resp`. `Stream` is reserved for Phase 2 subscription
/// methods and carries the subscription id the client should track.
#[derive(Debug)]
pub enum RpcOutcome {
    /// Single-shot JSON-RPC response payload (without the `jsonrpc`/`id`
    /// envelope — the transport adds that).
    Resp(Value),
    /// Reserved for Phase 2: opaque subscription id the transport keeps
    /// alive across multiple result frames.
    Stream(String),
}

/// Per-connection context handed to every dispatch invocation.
///
/// `kernel` is `Option` so unit tests can pass `None`; Task 9 injects the
/// real handle once the surface is wired.
#[derive(Clone)]
pub struct RpcCtx {
    pub registry: Arc<tokio::sync::Mutex<DeviceRegistry>>,
    pub device_id: String,
    pub kernel: Option<Arc<oxios_kernel::KernelHandle>>,
}

/// Dispatch a single JSON-RPC 2.0 request.
///
/// `req` is expected to be an object with `{"jsonrpc","id","method","params"}`.
/// Only `status.get` and `echo` are implemented in Phase 1; everything else
/// returns `-32601 method not found`.
pub async fn dispatch(req: Value, ctx: &RpcCtx) -> Result<RpcOutcome, RpcError> {
    let method = req
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_request("missing string `method`"))?;

    match method {
        "status.get" => {
            let paired_count = ctx.registry.lock().await.list().len();
            Ok(RpcOutcome::Resp(json!({
                "protocol_version": PROTOCOL_VERSION,
                "min_client_version": MIN_CLIENT_VERSION,
                "device_id": ctx.device_id,
                "paired_count": paired_count,
            })))
        }
        "echo" => {
            let params = req.get("params").cloned().unwrap_or(Value::Null);
            Ok(RpcOutcome::Resp(json!({ "echo": params })))
        }
        other => Err(RpcError::method_not_found(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    async fn build_registry(state_dir: &Path) -> Result<Arc<tokio::sync::Mutex<DeviceRegistry>>> {
        let registry = DeviceRegistry::load_or_create(state_dir)?;
        Ok(Arc::new(tokio::sync::Mutex::new(registry)))
    }
    /// Build a minimal `RpcCtx` backed by a real `DeviceRegistry` rooted in a
    /// throwaway temp dir. `kernel` is `None` — `status.get` never needs it.
    async fn test_ctx() -> (TempDir, RpcCtx) {
        let dir = TempDir::new().expect("tempdir");
        let registry = build_registry(dir.path())
            .await
            .expect("registry load_or_create");
        let ctx = RpcCtx {
            registry,
            device_id: "test-device".to_string(),
            kernel: None,
        };
        (dir, ctx)
    }

    fn run<F: Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(fut)
    }

    #[test]
    fn status_get_returns_version_constants_and_device_id() {
        run(async {
            let (_dir, ctx) = test_ctx().await;
            let req = json!({"jsonrpc": "2.0", "id": 1, "method": "status.get"});
            let out = dispatch(req, &ctx).await.expect("status.get ok");
            match out {
                RpcOutcome::Resp(v) => {
                    assert_eq!(v["protocol_version"], json!(PROTOCOL_VERSION));
                    assert_eq!(v["min_client_version"], json!(MIN_CLIENT_VERSION));
                    assert_eq!(v["device_id"], json!("test-device"));
                    assert_eq!(v["paired_count"], json!(0));
                }
                RpcOutcome::Stream(_) => panic!("status.get must return Resp"),
            }
        });
    }

    #[test]
    fn status_get_paired_count_matches_registry() {
        run(async {
            let (_dir, ctx) = test_ctx().await;
            // Pair two devices; registry persists inside the temp dir.
            {
                let mut reg = ctx.registry.lock().await;
                reg.pair("phone", "owner").expect("pair phone");
                reg.pair("laptop", "owner").expect("pair laptop");
            }
            let req = json!({"jsonrpc": "2.0", "id": 7, "method": "status.get"});
            let out = dispatch(req, &ctx).await.expect("status.get ok");
            match out {
                RpcOutcome::Resp(v) => assert_eq!(v["paired_count"], json!(2)),
                RpcOutcome::Stream(_) => panic!("status.get must return Resp"),
            }
        });
    }

    #[test]
    fn echo_returns_params() {
        run(async {
            let (_dir, ctx) = test_ctx().await;
            let params = json!({"hello": "world", "n": 42});
            let req = json!({"jsonrpc": "2.0", "id": 3, "method": "echo", "params": params});
            let out = dispatch(req, &ctx).await.expect("echo ok");
            match out {
                RpcOutcome::Resp(v) => assert_eq!(v["echo"], params),
                RpcOutcome::Stream(_) => panic!("echo must return Resp"),
            }
        });
    }

    #[test]
    fn echo_without_params_returns_null() {
        run(async {
            let (_dir, ctx) = test_ctx().await;
            let req = json!({"jsonrpc": "2.0", "id": 4, "method": "echo"});
            let out = dispatch(req, &ctx).await.expect("echo ok");
            match out {
                RpcOutcome::Resp(v) => assert_eq!(v["echo"], Value::Null),
                RpcOutcome::Stream(_) => panic!("echo must return Resp"),
            }
        });
    }

    #[test]
    fn unknown_method_returns_32601() {
        run(async {
            let (_dir, ctx) = test_ctx().await;
            let req = json!({"jsonrpc": "2.0", "id": 2, "method": "nope"});
            let err = dispatch(req, &ctx).await.expect_err("must error");
            assert_eq!(err.code, RPC_METHOD_NOT_FOUND);
            assert_eq!(err.code, -32601);
            assert!(
                err.message.contains("nope"),
                "method name must surface in message, got: {}",
                err.message
            );
        });
    }
    #[test]
    fn missing_method_field_is_invalid_request() {
        run(async {
            let (_dir, ctx) = test_ctx().await;
            let req = json!({"jsonrpc": "2.0", "id": 5});
            let err = dispatch(req, &ctx).await.expect_err("must error");
            assert_eq!(err.code, RPC_INVALID_REQUEST);
            assert!(
                err.message.contains("method"),
                "expected invalid-request error mentioning method, got: {}",
                err.message
            );
        });
    }
}
