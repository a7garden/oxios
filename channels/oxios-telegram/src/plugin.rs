//! Telegram channel plugin.
//!
//! Factory for creating the Telegram channel. Implements
//! [`ChannelPlugin`](oxios_gateway::plugin::ChannelPlugin) so the
//! main binary can activate the Telegram channel from configuration.

use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::plugin::{ChannelBundle, ChannelContext, ChannelPlugin};

use crate::{TelegramChannel, TelegramSessionSettings};

/// Telegram channel plugin — creates a Telegram Bot channel.
#[derive(Default)]
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
        let token = std::env::var(&config.channels.telegram.bot_token_env)
            .ok()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Telegram bot token not found. Set {} env var.",
                    config.channels.telegram.bot_token_env
                )
            })?;
        let allowed = config.channels.telegram.allowed_users.clone();

        let session_settings = TelegramSessionSettings {
            rotation_hours: config.channels.telegram.session.rotation_hours,
            max_messages_per_session: config.channels.telegram.session.max_messages,
        };

        let rotation_hours = session_settings.rotation_hours;
        let max_messages = session_settings.max_messages_per_session;

        let channel = TelegramChannel::new(token, allowed).with_session_settings(session_settings);

        tracing::info!(
            rotation_hours = rotation_hours,
            max_messages = max_messages,
            "Telegram channel created with session management"
        );

        Ok(ChannelBundle {
            channel: Box::new(channel),
            tasks: vec![],
        })
    }
}
