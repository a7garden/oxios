//! Code session types and state.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::change_tracker::ChangeTracker;
use super::checkpoint::CheckpointManager;

/// A coding session — tied to a project directory on the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSession {
    /// Unique session ID.
    pub id: String,
    /// Absolute path to the project directory.
    pub project_path: PathBuf,
    /// Optional model override for this session.
    pub model: Option<String>,
    /// Session creation time.
    pub created_at: DateTime<Utc>,
    /// Display title (defaults to directory name).
    pub title: String,
}

/// Agent-generated todo item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

/// Mutable per-session state held behind Arc<RwLock>.
pub struct CodeSessionState {
    pub session: CodeSession,
    pub changes: Arc<RwLock<ChangeTracker>>,
    pub checkpoints: Arc<RwLock<CheckpointManager>>,
    pub todos: Arc<RwLock<Vec<TodoItem>>>,
    pub git_branch: Option<String>,
}

impl CodeSessionState {
    pub fn new(session: CodeSession) -> Self {
        let project_root = session.project_path.clone();
        Self {
            changes: Arc::new(RwLock::new(ChangeTracker::new(project_root.clone()))),
            checkpoints: Arc::new(RwLock::new(CheckpointManager::new(project_root))),
            todos: Arc::new(RwLock::new(Vec::new())),
            session,
            git_branch: detect_git_branch(),
        }
    }
}

fn detect_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
