//! State API — data persistence, session management.

use std::sync::Arc;
use serde::{Serialize, de::DeserializeOwned};
use crate::state_store::{StateStore, Session, SessionId, SessionSummary};

/// State management system calls.
///
/// All data persistence: file (JSON/Markdown) storage, session management.
pub struct StateApi {
    pub(crate) state_store: Arc<StateStore>,
}

impl StateApi {
    /// Save JSON data.
    pub async fn save<T: Serialize>(&self, category: &str, name: &str, data: &T) -> anyhow::Result<()> {
        self.state_store.save_json(category, name, data).await
    }

    /// Save markdown content.
    pub async fn save_markdown(&self, category: &str, name: &str, content: &str) -> anyhow::Result<()> {
        self.state_store.save_markdown(category, name, content).await
    }

    /// Load JSON data.
    pub async fn load<T: DeserializeOwned>(&self, category: &str, name: &str) -> anyhow::Result<Option<T>> {
        self.state_store.load_json(category, name).await
    }

    /// Load markdown content.
    pub async fn load_markdown(&self, category: &str, name: &str) -> anyhow::Result<Option<String>> {
        self.state_store.load_markdown(category, name).await
    }

    /// Delete a file.
    pub async fn delete(&self, category: &str, name: &str) -> anyhow::Result<bool> {
        self.state_store.delete_file(category, name).await
    }

    /// List files in a category.
    pub async fn list_category(&self, category: &str) -> anyhow::Result<Vec<String>> {
        self.state_store.list_category(category).await
    }

    /// Commit all changes to git via the provided GitLayer.
    pub fn commit_all(&self, git: &crate::git_layer::GitLayer, message: &str) -> anyhow::Result<Option<crate::git_layer::CommitInfo>> {
        if !git.is_enabled() {
            return Ok(None);
        }
        git.commit_file(".", message).ok()
            .map_or(Ok(None), |info| Ok(Some(info)))
    }

    /// Save session.
    pub async fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        self.state_store.save_session(session).await
    }

    /// Load session.
    pub async fn load_session(&self, id: &SessionId) -> anyhow::Result<Option<Session>> {
        self.state_store.load_session(id).await
    }

    /// List sessions.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        self.state_store.list_sessions().await
    }

    /// Delete session.
    pub async fn delete_session(&self, id: &SessionId) -> anyhow::Result<bool> {
        self.state_store.delete_session(id).await
    }

    /// Get workspace base path.
    pub fn workspace_path(&self) -> &std::path::Path {
        &self.state_store.base_path
    }
}