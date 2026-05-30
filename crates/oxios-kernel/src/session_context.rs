//! Session-level context for managing conversation state across Seed executions.
//!
//! Introduced by RFC-020 Phase 1. Holds state that persists across multiple
//! Seed executions within a single user session.

use crate::memory::RecallTiming;
use crate::project::ProjectId;

/// Session-level context for managing conversation state.
///
/// Holds project associations and recall timing.
///
/// Created when a new session starts, passed to `AgentRuntime::execute()`.
#[derive(Debug)]
pub struct SessionContext {
    /// Proactive recall timing tracker (session-scoped).
    /// Tracks message count and topic changes to decide when
    /// to trigger proactive memory injection.
    pub recall_timing: Option<RecallTiming>,

    /// Primary project for this session (sets CWD, provides main context).
    pub primary_project_id: Option<ProjectId>,

    /// Secondary projects for cross-project work.
    pub secondary_project_ids: Vec<ProjectId>,
}

impl SessionContext {
    /// Create a new session context with default settings.
    pub fn new() -> Self {
        Self {
            recall_timing: Some(RecallTiming::new()),
            primary_project_id: None,
            secondary_project_ids: Vec::new(),
        }
    }

    /// Create a session context with a primary project.
    pub fn with_project(project_id: ProjectId) -> Self {
        Self {
            recall_timing: Some(RecallTiming::new()),
            primary_project_id: Some(project_id),
            secondary_project_ids: Vec::new(),
        }
    }

    /// Create a session context with multiple projects.
    pub fn with_projects(primary: ProjectId, secondary: Vec<ProjectId>) -> Self {
        Self {
            recall_timing: Some(RecallTiming::new()),
            primary_project_id: Some(primary),
            secondary_project_ids: secondary,
        }
    }

    /// Get all project IDs (primary first, then secondary).
    pub fn all_project_ids(&self) -> Vec<ProjectId> {
        let mut ids = Vec::new();
        if let Some(primary) = self.primary_project_id {
            ids.push(primary);
        }
        ids.extend(self.secondary_project_ids.iter().copied());
        ids
    }

    /// Whether this session has any project context.
    pub fn has_project(&self) -> bool {
        self.primary_project_id.is_some()
    }
}

impl Default for SessionContext {
    fn default() -> Self {
        Self::new()
    }
}
