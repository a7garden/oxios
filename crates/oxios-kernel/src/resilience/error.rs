//! Agent run error — carries the provider error AND the agent's exported
//! conversation state so the recovery coordinator can snapshot→restore
//! rather than re-fork from scratch (RFC-029 §3.2, P2b).
//!
//! Used by `run_agent` on failure: before returning Err, the agent's
//! `export_state()` is captured and wrapped. The supervisor's Err arm
//! downcasts back to extract both the source error (for `classify`) and
//! the restore state (for `ExecutionResult.restore_state`).

use std::fmt;

/// Error wrapper carrying the agent's exported state alongside the
/// original provider error.
///
/// Implementing `std::error::Error` + `Display` makes it compatible with
/// `anyhow::Error::downcast_ref`. The `source()` returns the original
/// error so `classify`'s cause-chain walk still works.
#[derive(Debug)]
pub struct AgentRunError {
    /// The original error (e.g. from `run_streaming()`).
    pub source: anyhow::Error,
    /// The agent's exported state at the point of failure, if available.
    /// `None` when the agent failed before accumulating any state (e.g.
    /// model-resolution error before the first LLM call).
    pub restore_state: Option<serde_json::Value>,
}

impl AgentRunError {
    /// Wrap a streaming error, capturing the agent's exported state.
    pub fn wrap(source: anyhow::Error, restore_state: Option<serde_json::Value>) -> Self {
        Self {
            source,
            restore_state,
        }
    }
}

impl fmt::Display for AgentRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for AgentRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
