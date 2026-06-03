//! Exec API — execution configuration and access management facade.

use crate::access_manager::AccessManager;
use crate::config::ExecConfig;
use std::sync::Arc;

/// Shared, hot-reloadable execution configuration.
///
/// `Arc<RwLock<...>>` so that runtime config changes (via `PUT /api/config`)
/// take effect immediately for all subscribers, including the `ExecTool`
/// embedded in agent CSpace registries.
pub type SharedExecConfig = Arc<parking_lot::RwLock<ExecConfig>>;

/// Execution management system calls.
///
/// Wraps [`ExecConfig`] for execution policy and [`AccessManager`] for
/// RBAC / path sandboxing enforcement.
pub struct ExecApi {
    config: SharedExecConfig,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
}

impl ExecApi {
    /// Create a new ExecApi.
    pub fn new(
        config: SharedExecConfig,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    ) -> Self {
        Self {
            config,
            access_manager,
        }
    }

    /// Take a snapshot of the current execution configuration.
    ///
    /// Returns a cloned `ExecConfig` so callers never hold the RwLock guard
    /// across an await point or a long-running operation.
    pub fn config_snapshot(&self) -> ExecConfig {
        self.config.read().clone()
    }

    /// Access manager reference.
    pub fn access_manager(&self) -> &Arc<parking_lot::Mutex<AccessManager>> {
        &self.access_manager
    }

    /// Shared config reference (for wiring into ExecTool).
    pub fn shared_config(&self) -> SharedExecConfig {
        Arc::clone(&self.config)
    }
}
