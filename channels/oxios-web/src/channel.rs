//! Web channel implementation.
//!
//! Implements the [`Channel`] trait for the web interface, allowing
//! the gateway to route messages to and from the HTTP API.

use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::channel::Channel;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use tokio::sync::mpsc;

/// The web channel adapter.
///
/// Bridges the axum HTTP server with the gateway's channel interface.
pub struct WebChannel {
    /// Receiver for incoming messages from the HTTP layer.
    #[allow(dead_code)] // TODO: wire up with channel receive()
    incoming_rx: mpsc::Receiver<IncomingMessage>,
    /// Sender to pass to the HTTP layer for injecting messages.
    incoming_tx: mpsc::Sender<IncomingMessage>,
}

impl WebChannel {
    /// Creates a new web channel with a bounded message buffer.
    pub fn new(buffer: usize) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::channel(buffer);
        Self {
            incoming_rx,
            incoming_tx,
        }
    }

    /// Returns a sender that can be used by HTTP handlers to inject messages.
    pub fn sender(&self) -> mpsc::Sender<IncomingMessage> {
        self.incoming_tx.clone()
    }
}

#[async_trait]
impl Channel for WebChannel {
    fn name(&self) -> &str {
        "web"
    }

    async fn receive(&self) -> Result<Option<IncomingMessage>> {
        // Note: in a real implementation, this would need interior mutability.
        // For the skeleton, we return None.
        // TODO: wire up with actual mpsc receiver
        Ok(None)
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        tracing::info!(user = %msg.user_id, content_len = msg.content.len(), "Web channel sending message");
        // TODO: push to SSE/WebSocket for real-time delivery
        Ok(())
    }
}

impl std::fmt::Debug for WebChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebChannel").finish()
    }
}
