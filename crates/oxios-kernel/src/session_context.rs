//! Session-level context for managing conversation state across Seed executions.
//!
//! Introduced by RFC-020 Phase 1. Holds state that persists across multiple
//! Seed executions within a single user session (e.g., RecallTiming for
//! proactive recall, future SonaManager reference).

use crate::memory::RecallTiming;

/// Session-level context for managing conversation state.
///
/// Holds RecallTiming for proactive recall, and will hold
/// SonaManager reference after RFC-020 Phase 2.
///
/// Created when a new session starts, passed to `AgentRuntime::execute()`.
#[derive(Debug)]
pub struct SessionContext {
    /// Proactive recall timing tracker (session-scoped).
    /// Tracks message count and topic changes to decide when
    /// to trigger proactive memory injection.
    pub recall_timing: Option<RecallTiming>,
}

impl SessionContext {
    /// Create a new session context with default settings.
    pub fn new() -> Self {
        Self {
            recall_timing: Some(RecallTiming::new()),
        }
    }
}

impl Default for SessionContext {
    fn default() -> Self {
        Self::new()
    }
}
