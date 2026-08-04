//! Remote companion surface (RFC-044): E2EE WS for paired mobile/web clients.
#![cfg(feature = "remote")]

use anyhow::Result;
use oxios_gateway::surface::{Surface, SurfaceContext, SurfaceHandle};

/// Kernel-connected E2EE companion surface.
pub struct RemoteRpcSurface;

impl RemoteRpcSurface {
    /// Create a new remote surface instance.
    pub fn new() -> Self {
        Self
    }
}
impl Default for RemoteRpcSurface {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Surface for RemoteRpcSurface {
    fn name(&self) -> &str {
        "remote"
    }
    async fn start(&self, _ctx: SurfaceContext) -> Result<SurfaceHandle> {
        // Wired in Task 9.
        Err(anyhow::anyhow!("RemoteRpcSurface not yet implemented"))
    }
}
