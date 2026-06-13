//! Pending tool approvals — runtime capability escalation via user consent.
//!
//! When a `GatedTool` denies a tool call due to missing CSpace capabilities,
//! it can register a pending approval here and block on a oneshot. The frontend
//! renders an approval card; the user's decision resolves the oneshot.
//!
//! Pattern: identical to `PendingQuestionnaires` (RFC-016).

use parking_lot::Mutex;
use std::collections::HashMap;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Result of a tool approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolApprovalResult {
    /// User approved — retry the tool call.
    Approved,
    /// User denied — return error to agent.
    Denied,
}

struct PendingEntry {
    tool_name: String,
    sender: oneshot::Sender<ToolApprovalResult>,
}

/// Thread-safe registry of in-flight tool approval requests.
#[derive(Default)]
pub struct PendingToolApprovals {
    inner: Mutex<HashMap<Uuid, PendingEntry>>,
}

impl PendingToolApprovals {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new pending tool approval.
    /// Returns the approval ID and a receiver to await the user's decision.
    pub fn register(&self, tool_name: String) -> (Uuid, oneshot::Receiver<ToolApprovalResult>) {
        let id = Uuid::new_v4();
        let (tx, rx) = oneshot::channel();
        self.inner.lock().insert(
            id,
            PendingEntry {
                tool_name,
                sender: tx,
            },
        );
        (id, rx)
    }

    /// Resolve a pending approval with the user's decision.
    /// Returns the tool name if the entry existed.
    pub fn resolve(&self, id: Uuid, result: ToolApprovalResult) -> Option<String> {
        let entry = self.inner.lock().remove(&id)?;
        let _ = entry.sender.send(result);
        Some(entry.tool_name)
    }

    /// Cancel all pending entries (e.g., on shutdown).
    pub fn cancel_all(&self) {
        let mut guard = self.inner.lock();
        for (_, entry) in guard.drain() {
            let _ = entry.sender.send(ToolApprovalResult::Denied);
        }
    }
}
