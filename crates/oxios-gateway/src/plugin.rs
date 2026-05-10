//! Channel plugin system.
//!
//! Provides a factory pattern for channels. Each channel (web, cli, telegram)
//! implements ChannelPlugin so the main binary can activate channels from
//! configuration without importing concrete types.

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::Channel;

/// Shared context provided to channel plugins during setup.
pub struct ChannelContext {
    /// Kernel subsystem handle (supervisor, state store, etc.).
    pub kernel: Arc<oxios_kernel::KernelHandle>,
    /// Hot-reloadable configuration.
    pub config: Arc<parking_lot::RwLock<oxios_kernel::OxiosConfig>>,
    /// Path to the config file.
    pub config_path: PathBuf,
}

/// Result of channel plugin setup.
pub struct ChannelBundle {
    /// The channel to register with the gateway.
    pub channel: Box<dyn Channel>,
    /// Background task handles (servers, event loops).
    pub tasks: Vec<JoinHandle<()>>,
}

/// Factory for creating and setting up a channel.
///
/// Implementors are compiled into the binary based on feature flags.
/// The main binary discovers plugins via the channel registry and calls
/// `setup()` for each enabled channel.
#[async_trait]
pub trait ChannelPlugin: Send + Sync {
    /// Unique name for this channel type (e.g., "web", "cli", "telegram").
    fn name(&self) -> &str;

    /// Create and set up the channel.
    ///
    /// Returns a bundle with the channel (for gateway registration)
    /// and optional background tasks (e.g., axum server, interactive loop).
    async fn setup(&self, ctx: ChannelContext) -> Result<ChannelBundle>;
}
