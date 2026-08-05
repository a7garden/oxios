//! JSON-RPC 2.0 dispatch for the remote companion surface (RFC-044 §6.5).
//!
//! Phase 2 methods:
//! - `auth.verify` — device-token verification (security gate)
//! - `status.get` — protocol version gate
//! - `echo` — test
//! - `session.list` / `session.create` — session management
//! - `persona.list` / `persona.activate` — persona switching
//! - `chat.send` — enqueue a user message via the gateway bridge
//! - `chat.subscribe` / `chat.unsubscribe` — streaming transcript
//! - `agent.status` — agent lifecycle events
//!
//! `status.get`, `echo`, and `auth.verify` are available pre-auth; all other
//! methods require a verified device token. Subscriptions return
//! [`RpcOutcome::Stream`] — the transport layer spawns the push task.
#![allow(dead_code)]

use std::pin::Pin;
use std::sync::Arc;

use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::remote::devices::DeviceRegistry;
use crate::remote::transport::ConnectionCtx;

// ── error codes ───────────────────────────────────────────────────────────

/// JSON-RPC 2.0 error codes used by the remote companion surface.
pub const RPC_METHOD_NOT_FOUND: i32 = -32601;
pub const RPC_INVALID_REQUEST: i32 = -32600;
pub const RPC_INTERNAL_ERROR: i32 = -32603;
/// Custom: authentication required for this method.
pub const RPC_AUTH_REQUIRED: i32 = -32001;
/// Custom: the device token was rejected.
pub const RPC_AUTH_FAILED: i32 = -32002;

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

    pub fn auth_required() -> Self {
        Self::new(
            RPC_AUTH_REQUIRED,
            "authentication required: call auth.verify first",
        )
    }

    pub fn auth_failed(message: impl Into<String>) -> Self {
        Self::new(RPC_AUTH_FAILED, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(RPC_INTERNAL_ERROR, message.into())
    }
}

// ── protocol versioning ───────────────────────────────────────────────────

pub const PROTOCOL_VERSION: u32 = 1;
pub const MIN_CLIENT_VERSION: u32 = 1;

// ── outcomes ──────────────────────────────────────────────────────────────

/// What kind of server-pushed subscription to spawn.
#[derive(Debug, Clone)]
pub enum SubscriptionKind {
    /// Streaming transcript for a session (tool events, text deltas, thinking).
    Chat { session_id: String },
    /// Agent lifecycle events (working / blocked / waiting / done).
    AgentStatus,
}

/// Outcome of dispatching a single JSON-RPC request.
#[derive(Debug)]
pub enum RpcOutcome {
    /// Single-shot response payload (without the envelope).
    Resp(Value),
    /// Server-pushed subscription. The transport sends the response, then
    /// spawns a push task of the given `kind` using `conn_ctx.push_tx`.
    Stream {
        subscription_id: String,
        kind: SubscriptionKind,
    },
}

pub trait GatewayBridge: Send + Sync {
    fn send_and_wait(
        &self,
        content: String,
        session_id: Option<String>,
        persona_id: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

    /// Fire-and-forget: enqueue a message without waiting for a response.
    /// Used by worktree fan-out to spawn N concurrent agents.
    fn send_fire_and_forget(
        &self,
        content: String,
        persona_id: Option<String>,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<(), String>;
}

/// Per-connection context handed to every dispatch invocation.
#[derive(Clone)]
pub struct RpcCtx {
    pub registry: Arc<Mutex<DeviceRegistry>>,
    pub device_id: String,
    pub kernel: Option<Arc<oxios_kernel::KernelHandle>>,
    pub bridge: Option<Arc<dyn GatewayBridge>>,
}

impl RpcCtx {
    /// Returns `true` if the method requires prior `auth.verify`.
    fn requires_auth(method: &str) -> bool {
        !matches!(method, "status.get" | "echo" | "auth.verify")
    }
}

// ── dispatch ──────────────────────────────────────────────────────────────

/// Dispatch a single JSON-RPC 2.0 request.
///
/// `conn` carries per-connection auth state and the push sender for
/// subscriptions. Auth-gated methods check `conn.is_authenticated()`.
pub async fn dispatch(
    req: Value,
    ctx: &RpcCtx,
    conn: &ConnectionCtx,
) -> Result<RpcOutcome, RpcError> {
    let method = req
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_request("missing string `method`"))?;

    // Auth gate
    if RpcCtx::requires_auth(method) && !conn.is_authenticated() {
        return Err(RpcError::auth_required());
    }

    let params = req.get("params").cloned().unwrap_or(Value::Null);

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

        "echo" => Ok(RpcOutcome::Resp(json!({ "echo": params }))),

        "auth.verify" => {
            let device_id = params
                .get("device_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `device_id`"))?;
            let token = params
                .get("token")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `token`"))?;

            let registry = ctx.registry.lock().await;
            if registry.verify(device_id, token) {
                drop(registry);
                let _ = ctx.registry.lock().await.touch(device_id);
                conn.set_authenticated(device_id.to_string());
                Ok(RpcOutcome::Resp(json!({
                    "verified": true,
                    "device_id": device_id,
                })))
            } else {
                Err(RpcError::auth_failed("invalid device token"))
            }
        }

        "session.list" => {
            let kernel = ctx
                .kernel
                .as_ref()
                .ok_or_else(|| RpcError::internal("kernel not available"))?;
            let sessions = kernel
                .state
                .list_sessions()
                .await
                .map_err(|e| RpcError::internal(format!("list_sessions: {e}")))?;
            let rows: Vec<Value> = sessions
                .into_iter()
                .map(|s| {
                    json!({
                        "id": s.id,
                        "title": s.title,
                        "project_id": s.project_id,
                        "message_count": s.message_count,
                        "created_at": s.created_at.to_rfc3339(),
                        "updated_at": s.updated_at.to_rfc3339(),
                    })
                })
                .collect();
            Ok(RpcOutcome::Resp(json!({ "sessions": rows })))
        }

        "session.create" => {
            let kernel = ctx
                .kernel
                .as_ref()
                .ok_or_else(|| RpcError::internal("kernel not available"))?;
            let persona_id = params.get("persona_id").and_then(Value::as_str);
            let project_id = params.get("project_id").and_then(Value::as_str);

            let mut session = oxios_kernel::state_store::Session::new("companion");
            if let Some(pid) = persona_id {
                session.active_persona_id = Some(pid.to_string());
            }
            if let Some(pid) = project_id {
                session.project_id = Some(pid.to_string());
            }
            kernel
                .state
                .save_session(&session)
                .await
                .map_err(|e| RpcError::internal(format!("save_session: {e}")))?;

            Ok(RpcOutcome::Resp(json!({
                "id": session.id.0,
                "active_persona_id": session.active_persona_id,
                "project_id": session.project_id,
                "created_at": session.created_at.to_rfc3339(),
            })))
        }

        "persona.list" => {
            let kernel = ctx
                .kernel
                .as_ref()
                .ok_or_else(|| RpcError::internal("kernel not available"))?;
            let active = kernel.persona.active_id();
            let personas: Vec<Value> = kernel
                .persona
                .list()
                .into_iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "name": p.name,
                        "role": p.role,
                        "description": p.description,
                        "model": p.model,
                        "enabled": p.enabled,
                        "capabilities": p.capabilities,
                        "is_active": active.as_deref() == Some(&p.id),
                    })
                })
                .collect();
            Ok(RpcOutcome::Resp(json!({ "personas": personas })))
        }

        "persona.activate" => {
            let kernel = ctx
                .kernel
                .as_ref()
                .ok_or_else(|| RpcError::internal("kernel not available"))?;
            let persona_id = params
                .get("persona_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `persona_id`"))?;
            kernel
                .persona
                .set_active(persona_id)
                .await
                .map_err(|e| RpcError::internal(format!("set_active: {e}")))?;
            Ok(RpcOutcome::Resp(json!({ "active_persona_id": persona_id })))
        }

        "chat.send" => {
            let content = params
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `content`"))?
                .to_string();
            let session_id = params
                .get("session_id")
                .and_then(Value::as_str)
                .map(String::from);
            let persona_id = params
                .get("persona_id")
                .and_then(Value::as_str)
                .map(String::from);

            let bridge = ctx
                .bridge
                .as_ref()
                .ok_or_else(|| RpcError::internal("gateway bridge not available"))?;

            let response = bridge
                .send_and_wait(content, session_id, persona_id)
                .await
                .map_err(RpcError::internal)?;

            Ok(RpcOutcome::Resp(json!({ "response": response })))
        }

        "chat.subscribe" => {
            let session_id = params
                .get("session_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `session_id`"))?;
            let sub_id = format!("chat_{}", uuid::Uuid::new_v4());
            Ok(RpcOutcome::Stream {
                subscription_id: sub_id,
                kind: SubscriptionKind::Chat {
                    session_id: session_id.to_string(),
                },
            })
        }

        "chat.unsubscribe" => {
            // Subscriptions are per-connection: closing the connection or
            // sending unsubscribe terminates them. The push task exits when
            // its push_tx is dropped (connection close) or when it receives
            // an unsubscribe for its subscription id.
            Ok(RpcOutcome::Resp(json!({ "unsubscribed": true })))
        }

        "agent.status" => {
            let sub_id = format!("agent_{}", uuid::Uuid::new_v4());
            Ok(RpcOutcome::Stream {
                subscription_id: sub_id,
                kind: SubscriptionKind::AgentStatus,
            })
        }

        "worktree.list" => {
            let project_path = params
                .get("project_path")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `project_path`"))?;
            let worktrees = list_git_worktrees(project_path)
                .map_err(|e| RpcError::internal(format!("git worktree list: {e}")))?;
            let rows: Vec<Value> = worktrees
                .into_iter()
                .map(|(path, branch, commit)| {
                    json!({ "path": path, "branch": branch, "commit": commit })
                })
                .collect();
            Ok(RpcOutcome::Resp(json!({ "worktrees": rows })))
        }

        "worktree.create" => {
            let project_path = params
                .get("project_path")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `project_path`"))?;
            let branch = params
                .get("branch")
                .and_then(Value::as_str)
                .unwrap_or("main");
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `name`"))?;

            let wt_path = create_git_worktree(project_path, branch, name)
                .map_err(|e| RpcError::internal(format!("git worktree add: {e}")))?;
            Ok(RpcOutcome::Resp(json!({
                "path": wt_path,
                "branch": format!("oxios/{name}"),
            })))
        }

        "worktree.fanout" => {
            let project_path = params
                .get("project_path")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `project_path`"))?
                .to_string();
            let prompt = params
                .get("prompt")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_request("missing `prompt`"))?
                .to_string();
            let count = params.get("count").and_then(Value::as_u64).unwrap_or(3) as usize;
            let persona_id = params
                .get("persona_id")
                .and_then(Value::as_str)
                .unwrap_or("dev");

            let bridge = ctx
                .bridge
                .as_ref()
                .ok_or_else(|| RpcError::internal("gateway bridge not available"))?;

            // Cap N at a reasonable limit (RFC-044 §11.6: respect max_agents).
            let n = count.min(8);
            let mut worktrees = Vec::new();

            for i in 0..n {
                let name = format!("fanout-{i}-{}", chrono::Utc::now().timestamp());
                let wt_path = create_git_worktree(&project_path, "HEAD", &name)
                    .map_err(|e| RpcError::internal(format!("worktree {i}: {e}")))?;

                // Fire-and-forget: each agent runs concurrently.
                // Responses stream back via chat.subscribe / agent.status.
                let metadata = std::collections::HashMap::from([
                    ("worktree_path".to_string(), wt_path.clone()),
                    ("fanout_index".to_string(), i.to_string()),
                ]);
                bridge
                    .send_fire_and_forget(prompt.clone(), Some(persona_id.to_string()), metadata)
                    .map_err(RpcError::internal)?;

                worktrees.push(json!({
                    "index": i,
                    "path": wt_path,
                }));
            }

            Ok(RpcOutcome::Resp(json!({
                "fanout_count": n,
                "worktrees": worktrees,
            })))
        }

        other => Err(RpcError::method_not_found(other)),
    }
}

// ── git worktree helpers (Phase 4) ────────────────────────────────────────

/// List git worktrees for a repository. Returns `(path, branch, commit)`.
fn list_git_worktrees(
    repo_path: &str,
) -> std::result::Result<Vec<(String, String, String)>, String> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut path = String::new();
    let mut branch = String::new();
    let mut commit = String::new();
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if !path.is_empty() {
                worktrees.push((
                    std::mem::take(&mut path),
                    std::mem::take(&mut branch),
                    std::mem::take(&mut commit),
                ));
            }
            path = p.to_string();
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            branch = b.to_string();
        } else if let Some(c) = line.strip_prefix("HEAD ") {
            commit = c.to_string();
        }
    }
    if !path.is_empty() {
        worktrees.push((path, branch, commit));
    }
    Ok(worktrees)
}

/// Create a new git worktree at `{repo}/.oxios-worktrees/{name}` on a new branch.
fn create_git_worktree(
    repo_path: &str,
    base_ref: &str,
    name: &str,
) -> std::result::Result<String, String> {
    let wt_dir = std::path::Path::new(repo_path).join(".oxios-worktrees");
    std::fs::create_dir_all(&wt_dir).map_err(|e| format!("mkdir worktrees: {e}"))?;
    let wt_path = wt_dir.join(name);
    let branch = format!("oxios/{name}");

    let output = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &branch,
            wt_path.to_str().ok_or("invalid path")?,
            base_ref,
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(wt_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn() -> (tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, ConnectionCtx) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (rx, ConnectionCtx::new(tx))
    }

    async fn make_ctx() -> RpcCtx {
        let tmp = tempfile::tempdir().unwrap();
        let registry = DeviceRegistry::load_or_create(tmp.path()).unwrap();
        // Leak the temp dir so its path stays valid for the test duration.
        std::mem::forget(tmp);
        RpcCtx {
            registry: Arc::new(Mutex::new(registry)),
            device_id: "test-device".to_string(),
            kernel: None,
            bridge: None,
        }
    }

    #[tokio::test]
    async fn status_get_works_pre_auth() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        let req = json!({"method": "status.get"});
        let outcome = dispatch(req, &ctx, &conn).await.unwrap();
        match outcome {
            RpcOutcome::Resp(v) => {
                assert_eq!(v["protocol_version"], PROTOCOL_VERSION);
                assert_eq!(v["device_id"], "test-device");
            }
            _ => panic!("expected Resp"),
        }
    }

    #[tokio::test]
    async fn session_list_requires_auth() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        let req = json!({"method": "session.list"});
        let err = dispatch(req, &ctx, &conn).await.unwrap_err();
        assert_eq!(err.code, RPC_AUTH_REQUIRED);
    }

    #[tokio::test]
    async fn auth_verify_with_valid_token() {
        let ctx = make_ctx().await;
        let (device_id, token) = {
            let mut reg = ctx.registry.lock().await;
            reg.pair("test-phone", "mobile").unwrap()
        };
        let (_rx, conn) = make_conn();
        let req = json!({
            "method": "auth.verify",
            "params": { "device_id": device_id, "token": token }
        });
        let outcome = dispatch(req, &ctx, &conn).await.unwrap();
        match outcome {
            RpcOutcome::Resp(v) => {
                assert_eq!(v["verified"], true);
                assert!(conn.is_authenticated());
            }
            _ => panic!("expected Resp"),
        }
    }

    #[tokio::test]
    async fn auth_verify_with_bad_token_fails() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        let req = json!({
            "method": "auth.verify",
            "params": { "device_id": "fake", "token": "wrong" }
        });
        let err = dispatch(req, &ctx, &conn).await.unwrap_err();
        assert_eq!(err.code, RPC_AUTH_FAILED);
        assert!(!conn.is_authenticated());
    }

    #[tokio::test]
    async fn after_auth_sensitive_methods_pass_auth_gate() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        conn.set_authenticated("dev-1".to_string());
        let req = json!({"method": "session.list"});
        let err = dispatch(req, &ctx, &conn).await.unwrap_err();
        assert_eq!(err.code, RPC_INTERNAL_ERROR);
        assert!(err.message.contains("kernel"));
    }

    #[tokio::test]
    async fn echo_works_pre_auth() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        let req = json!({"method": "echo", "params": {"hello": "world"}});
        let outcome = dispatch(req, &ctx, &conn).await.unwrap();
        match outcome {
            RpcOutcome::Resp(v) => assert_eq!(v["echo"]["hello"], "world"),
            _ => panic!("expected Resp"),
        }
    }

    #[tokio::test]
    async fn unknown_method_returns_not_found() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        conn.set_authenticated("dev-1".to_string());
        let req = json!({"method": "bogus.method"});
        let err = dispatch(req, &ctx, &conn).await.unwrap_err();
        assert_eq!(err.code, RPC_METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn chat_subscribe_returns_stream() {
        let ctx = make_ctx().await;
        let (_rx, conn) = make_conn();
        conn.set_authenticated("dev-1".to_string());
        let req = json!({
            "method": "chat.subscribe",
            "params": { "session_id": "sess-123" }
        });
        let outcome = dispatch(req, &ctx, &conn).await.unwrap();
        match outcome {
            RpcOutcome::Stream { kind, .. } => match kind {
                SubscriptionKind::Chat { session_id } => {
                    assert_eq!(session_id, "sess-123");
                }
                _ => panic!("expected Chat subscription"),
            },
            _ => panic!("expected Stream"),
        }
    }
}
