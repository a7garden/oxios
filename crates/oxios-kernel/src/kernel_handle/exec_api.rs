//! Exec API — execution configuration and access management facade.

use crate::access_manager::AccessManager;
use crate::config::ExecConfig;
use std::sync::Arc;

/// Execution management system calls.
///
/// Wraps [`ExecConfig`] for execution policy and [`AccessManager`] for
/// RBAC / path sandboxing enforcement.
pub struct ExecApi {
    config: Arc<ExecConfig>,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
}

impl ExecApi {
    /// Create a new ExecApi.
    pub fn new(
        config: Arc<ExecConfig>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    ) -> Self {
        Self {
            config,
            access_manager,
        }
    }

    /// Execution configuration reference.
    pub fn config(&self) -> &ExecConfig {
        &self.config
    }

    /// Access manager reference.
    pub fn access_manager(&self) -> &Arc<parking_lot::Mutex<AccessManager>> {
        &self.access_manager
    }
}
