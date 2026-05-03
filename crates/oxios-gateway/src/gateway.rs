//! Gateway: routes messages between channels and the kernel.
//!
//! The gateway is channel-agnostic. It receives messages from any
//! registered channel, dispatches them to the kernel, and returns
//! responses through the appropriate channel.

use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::channel::Channel;
use crate::message::{IncomingMessage, OutgoingMessage};

/// The message gateway connecting channels to the kernel.
pub struct Gateway {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
}

impl Gateway {
    /// Creates a new gateway.
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a channel with the gateway.
    pub async fn register(&self, channel: Box<dyn Channel>) {
        let name = channel.name().to_owned();
        let mut channels = self.channels.write().await;
        tracing::info!(channel = %name, "Channel registered");
        channels.insert(name, channel);
    }

    /// Routes an incoming message to the appropriate handler.
    ///
    /// In the full implementation, this will dispatch to the kernel's
    /// ouroboros protocol. For now, it logs the message.
    pub async fn route(&self, msg: IncomingMessage) -> Result<()> {
        tracing::info!(
            channel = %msg.channel,
            user = %msg.user_id,
            content_len = msg.content.len(),
            "Routing incoming message"
        );
        // TODO: dispatch to kernel supervisor
        Ok(())
    }

    /// Sends a message through the named channel.
    pub async fn send_to(&self, channel_name: &str, msg: OutgoingMessage) -> Result<()> {
        let channels = self.channels.read().await;
        if let Some(ch) = channels.get(channel_name) {
            ch.send(msg).await?;
        } else {
            tracing::warn!(channel = %channel_name, "No such channel registered");
        }
        Ok(())
    }

    /// Runs the main event loop, receiving from all channels.
    ///
    /// This polls each registered channel for incoming messages
    /// and routes them through the kernel.
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Gateway event loop started");
        // TODO: implement multi-channel polling
        // For now, just sleep indefinitely.
        tokio::signal::ctrl_c().await?;
        tracing::info!("Gateway shutting down");
        Ok(())
    }

    /// Returns the names of all registered channels.
    pub async fn channel_names(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway").finish()
    }
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}
