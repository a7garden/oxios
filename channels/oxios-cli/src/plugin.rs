//! CLI channel plugin.
//!
//! Factory for creating the CLI channel. Implements
//! [`ChannelPlugin`](oxios_gateway::plugin::ChannelPlugin) so the
//! main binary can activate the CLI channel from configuration.
//!
//! Note: The interactive readline loop is *not* started here — that
//! is only used by the `oxios chat` subcommand.

use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::plugin::{ChannelBundle, ChannelContext, ChannelPlugin};

use crate::channel::CliChannel;

/// CLI channel plugin — creates a stdin/stdout channel.
pub struct CliPlugin;

impl CliPlugin {
    /// Create a new CLI plugin instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CliPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelPlugin for CliPlugin {
    fn name(&self) -> &str {
        "cli"
    }

    async fn setup(&self, ctx: ChannelContext) -> Result<ChannelBundle> {
        let channel = CliChannel::with_kernel(256, Some(ctx.kernel.clone()));
        Ok(ChannelBundle {
            channel: Box::new(channel),
            tasks: vec![],
        })
    }
}
