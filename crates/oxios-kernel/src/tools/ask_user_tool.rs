//! `ask_user` tool — RFC-027 agent-driven clarification.
//!
//! When the agent encounters ambiguity during execution, it can call this tool
//! to surface a question to the user via a oneshot channel. The tool:
//!
//! 1. Generates a unique request ID.
//! 2. Registers a `oneshot::Sender<String>` in the shared
//!    [`PendingAskUser`] registry.
//! 3. Publishes a [`KernelEvent::AskUserRequest`] on the event bus so the
//!    frontend can render an input/picker.
//! 4. Awaits the user's response (delivered via the API response handler
//!    that resolves the oneshot) and returns the answer to the agent.
//!
//! The full WebSocket/API integration for resolving the oneshot is the
//! gateway phase. This module covers the kernel-side plumbing.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::oneshot;
use uuid::Uuid;

use oxi_sdk::{AgentTool, AgentToolResult, ToolContext, ToolError};

use crate::event_bus::{EventBus, KernelEvent};
// ─── Pending Registry ──────────────────────────────────────────────────

struct PendingEntry {
    sender: oneshot::Sender<String>,
}

/// Thread-safe registry of in-flight `ask_user` requests.
///
/// Mirrors the [`PendingToolApprovals`](crate::tools::pending_tool_approvals::PendingToolApprovals)
/// pattern: agents register a oneshot, the API/WS response handler resolves it.
#[derive(Default)]
pub struct PendingAskUser {
    inner: Mutex<HashMap<String, PendingEntry>>,
}

impl PendingAskUser {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new pending question. Returns the request ID and the
    /// receiver the tool will await.
    pub fn register(&self) -> (String, oneshot::Receiver<String>) {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.inner
            .lock()
            .insert(id.clone(), PendingEntry { sender: tx });
        (id, rx)
    }

    /// Resolve a pending question with the user's answer.
    /// Returns `true` if the entry existed and was resolved.
    pub fn resolve(&self, id: &str, answer: String) -> bool {
        let Some(entry) = self.inner.lock().remove(id) else {
            return false;
        };
        // The receiver may already have been dropped (e.g., on shutdown).
        // Ignore the send error — there's nothing actionable.
        let _ = entry.sender.send(answer);
        true
    }

    /// Cancel all pending entries (e.g., on shutdown). Tools awaiting the
    /// oneshot will observe a `RecvError` and translate that into a tool
    /// error so the agent loop can recover.
    pub fn cancel_all(&self) {
        let mut guard = self.inner.lock();
        for (_, entry) in guard.drain() {
            // Dropping the sender closes the channel without sending —
            // receivers see RecvError::Closed.
            drop(entry.sender);
        }
    }
}

// ─── Tool ──────────────────────────────────────────────────────────────

/// Tool that lets an agent ask the user a clarifying question during execution.
///
/// The frontend subscribes to [`KernelEvent::AskUserRequest`] events and
/// resolves the pending oneshot via the response handler wired into
/// [`PendingAskUser`].
pub struct AskUserTool {
    pending: Arc<PendingAskUser>,
    event_bus: EventBus,
}

impl AskUserTool {
    /// Create a new `AskUserTool` bound to the shared registry and event bus.
    pub fn new(pending: Arc<PendingAskUser>, event_bus: EventBus) -> Self {
        Self { pending, event_bus }
    }
}

impl std::fmt::Debug for AskUserTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AskUserTool").finish()
    }
}

#[derive(Debug, Deserialize)]
struct AskUserArgs {
    question: String,
    #[serde(default)]
    options: Vec<String>,
}

#[async_trait]
impl AgentTool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn label(&self) -> &str {
        "Ask User"
    }

    fn description(&self) -> &'static str {
        "Ask the user a clarifying question during task execution. \
         Provide a `question` and optionally a list of `options` for a \
         structured picker. Execution blocks until the user responds or \
         the request is cancelled."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user."
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of choices for a structured picker. \
                                    Omit for an open-ended text input."
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, ToolError> {
        let args: AskUserArgs =
            serde_json::from_value(params).map_err(|e| format!("Invalid arguments: {e}"))?;

        if args.question.trim().is_empty() {
            return Err("question must not be empty".to_string());
        }

        // Register the oneshot BEFORE publishing so a fast frontend cannot
        // resolve a request that hasn't been registered yet.
        let (id, rx) = self.pending.register();

        let event = KernelEvent::AskUserRequest {
            id: id.clone(),
            question: args.question.clone(),
            options: args.options.clone(),
        };
        if let Err(e) = self.event_bus.publish(event) {
            // Best-effort cleanup if the bus is closed.
            self.pending.resolve(&id, String::new());
            return Err(format!("Failed to publish AskUserRequest event: {e}"));
        }

        tracing::info!(
            request_id = %id,
            options = args.options.len(),
            "ask_user: question published, awaiting user response"
        );

        // Block until the user (or a cancellation) resolves the oneshot.
        let answer = match rx.await {
            Ok(answer) => answer,
            Err(_) => {
                tracing::warn!(request_id = %id, "ask_user: receiver dropped before response");
                return Err("ask_user request was cancelled before the user responded".to_string());
            }
        };

        Ok(AgentToolResult::success(answer))
    }
}
