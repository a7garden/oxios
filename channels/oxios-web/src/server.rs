//! Web application state for the dashboard channel.
//!
//! `AppState` is the shared state accessible to all route handlers.
//! The server lifecycle is managed by [`WebPlugin`](crate::plugin::WebPlugin).

use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::channel::WebChannelHandle;
use crate::error::AppError;
use crate::middleware::RateLimiter;
use oxios_kernel::{config, KernelHandle, OxiosConfig};
use oxios_markdown::KnowledgeBase;

/// Shared application state for the web dashboard.
///
/// All subsystems are accessible through `state.kernel.xxx()`.
#[derive(Clone)]
pub struct AppState {
    /// Base URL for API responses.
    pub base_url: String,
    /// Knowledge base for markdown CRUD (no kernel dependency).
    pub knowledge: Arc<KnowledgeBase>,
    /// Handle to the kernel subsystem (provides access to all kernel components).
    pub kernel: Arc<KernelHandle>,
    /// Handle to the web channel for message passing.
    pub channel: WebChannelHandle,
    /// Loaded configuration (hot-reloadable via RwLock).
    pub config: Arc<RwLock<OxiosConfig>>,
    /// Path to the config file (for persistence on PUT /api/config).
    pub config_path: PathBuf,
    /// Server start time (for uptime calculation).
    pub start_time: Instant,
    /// Rate limiter for API endpoints.
    #[allow(dead_code)]
    pub rate_limiter: RateLimiter,
    /// Override web dist directory for auto-update UI. `None` = embedded only.
    pub web_dist: Option<PathBuf>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("base_url", &self.base_url)
            .field("kernel", &"...")
            .field("channel", &"...")
            .field("config", &"...")
            .field("config_path", &self.config_path)
            .finish()
    }
}

impl AppState {
    /// Reload config from disk and update in-memory state.
    pub async fn reload_config(&self) -> Result<(), AppError> {
        let config = config::load_config(&self.config_path)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        *self.config.write() = config;

        tracing::info!("Config hot-reloaded from {}", self.config_path.display());
        Ok(())
    }
}
