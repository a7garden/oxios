//! PtyApi — 14th typed facade alongside ExecApi (RFC-038 §8.1).
use std::sync::Arc;

use parking_lot::RwLock;

use crate::config::PtyConfig;
use crate::pty::{PtyManager, PtySessionInfo, PtySize};
use crate::access_manager::AuditSink;

/// Shared, hot-reloadable PTY config.
pub type SharedPtyConfig = Arc<RwLock<PtyConfig>>;

/// Facade over [`PtyManager`] for kernel consumers and HTTP routes.
pub struct PtyApi {
    pub manager: Arc<PtyManager>,
    pub config: SharedPtyConfig,
    audit: Arc<dyn AuditSink>,
}

impl PtyApi {
    pub fn new(config: SharedPtyConfig, audit: Arc<dyn AuditSink>) -> Self {
        let manager = Arc::new(PtyManager::new(Arc::clone(&config), Arc::clone(&audit)));
        Self {
            manager,
            config,
            audit,
        }
    }

    /// Snapshot of the current PTY config.
    pub fn config_snapshot(&self) -> PtyConfig {
        self.config.read().clone()
    }

    /// True if `[pty] enabled = true` in config.
    pub fn is_enabled(&self) -> bool {
        self.config.read().enabled
    }

    /// Open a new PTY session. Validates shell allowlist + per-principal cap.
    pub fn open(
        &self,
        principal: &str,
        shell: Option<String>,
        size: PtySize,
    ) -> Result<crate::pty::PtySessionId, crate::pty::PtyError> {
        let s = self.manager.open(principal, shell, size)?;
        Ok(s.id.clone())
    }

    /// Re-attach an existing session by id.
    pub fn attach(
        &self,
        principal: &str,
        session_id: &str,
    ) -> Result<crate::pty::PtySessionId, crate::pty::PtyError> {
        let s = self.manager.attach(principal, session_id)?;
        Ok(s.id.clone())
    }

    /// Write bytes to a session's stdin.
    pub fn write(&self, session_id: &str, bytes: &[u8]) -> Result<(), crate::pty::PtyError> {
        self.manager.write(session_id, bytes)
    }

    /// Resize a session's PTY.
    pub fn resize(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), crate::pty::PtyError> {
        self.manager.resize(session_id, cols, rows)
    }

    /// Take a reader for streaming PTY stdout to a WS client.
    pub fn try_clone_reader(
        &self,
        session_id: &str,
    ) -> Result<Box<dyn std::io::Read + Send>, crate::pty::PtyError> {
        self.manager.try_clone_reader(session_id)
    }

    /// Mark session as Attached.
    pub fn mark_attached(&self, session_id: &str) -> bool {
        self.manager.mark_attached(session_id)
    }

    /// Mark session as Detached.
    pub fn mark_detached(&self, session_id: &str) -> bool {
        self.manager.mark_detached(session_id)
    }

    /// Close a session (SIGTERM via Drop).
    pub fn close(&self, session_id: &str) -> Result<(), crate::pty::PtyError> {
        self.manager.close(session_id)
    }

    /// List sessions for a principal (UI listing endpoint).
    pub fn list_sessions(&self, principal: &str) -> Vec<PtySessionInfo> {
        self.manager.list_sessions(principal)
    }

    /// Spawn the GC tick task.
    pub fn start_gc(&self) -> tokio::task::JoinHandle<()> {
        Arc::clone(&self.manager).start_gc()
    }
}