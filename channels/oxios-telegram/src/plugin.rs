//! Telegram channel plugin.
//!
//! Factory for creating the Telegram channel. Implements
//! [`ChannelPlugin`](oxios_gateway::plugin::ChannelPlugin) so the
//! main binary can activate the Telegram channel from configuration.

use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::plugin::{ChannelBundle, ChannelContext, ChannelPlugin};

use crate::TelegramChannel;

/// Telegram channel plugin — creates a Telegram Bot channel.
pub struct TelegramPlugin;

impl TelegramPlugin {
    /// Create a new Telegram plugin instance.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ChannelPlugin for TelegramPlugin {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn setup(&self, ctx: ChannelContext) -> Result<ChannelBundle> {
        let config = ctx.config.read().clone();
        let token = std::env::var(&config.channels.telegram.bot_token_env).map_err(|_| {
            anyhow::anyhow!(
                "Telegram bot token not found. Set {} env var.",
                config.channels.telegram.bot_token_env
            )
        })?;
        let allowed = config.channels.telegram.allowed_users.clone();

        let channel = TelegramChannel::new(token, allowed);

        tracing::info!("Telegram channel created");

        Ok(ChannelBundle {
            channel: Box::new(channel),
            tasks: vec![],
        })
    }
}
