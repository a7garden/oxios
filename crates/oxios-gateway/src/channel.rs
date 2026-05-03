//! Channel trait definition.
//!
//! A channel is a plugin that connects the gateway to a specific
//! interface (Web, CLI, Telegram, etc.).

use anyhow::Result;
use async_trait::async_trait;

use crate::message::{IncomingMessage, OutgoingMessage};

/// A communication channel that plugs into the gateway.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Returns the name of this channel (e.g., "web", "telegram").
    fn name(&self) -> &str;

    /// Receive the next incoming message, or None if the channel is closed.
    async fn receive(&self) -> Result<Option<IncomingMessage>>;

    /// Send a message through this channel.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
