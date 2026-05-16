//! Gateway: routes messages between channels and the kernel.
//!
//! The gateway is channel-agnostic. It receives messages from any
//! registered channel, dispatches them to the kernel via the
//! orchestrator, and returns responses through the appropriate channel.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

use crate::channel::Channel;
use crate::message::{IncomingMessage, OutgoingMessage};

/// The message gateway connecting channels to the kernel.
pub struct Gateway {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
    /// Shared orchestrator for the Ouroboros lifecycle.
    orchestrator: Arc<oxios_kernel::Orchestrator>,
}

impl Gateway {
    /// Creates a new gateway with the given orchestrator.
    pub fn new(orchestrator: Arc<oxios_kernel::Orchestrator>) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            orchestrator,
        }
    }

    /// Registers a channel with the gateway.
    pub async fn register(&self, channel: Box<dyn Channel>) {
        let name = channel.name().to_owned();
        let mut channels = self.channels.write().await;
        tracing::info!(channel = %name, "Channel registered");
        channels.insert(name, channel);
    }

    /// Routes an incoming message through the Ouroboros orchestrator.
    ///
    /// The message goes: Channel → Gateway → Orchestrator → Kernel → Result.
    /// The result is then sent back as an outgoing message via the same channel.
    pub async fn route(&self, msg: IncomingMessage) -> Result<()> {
        tracing::info!(
            channel = %msg.channel,
            user = %msg.user_id,
            content_len = msg.content.len(),
            "Routing incoming message"
        );

        // Extract session_id from metadata if present.
        let session_id = msg.metadata.get("session_id").cloned();

        // Run the full Ouroboros loop.
        let result = self
            .orchestrator
            .handle_message(&msg.user_id, &msg.content, session_id.as_deref())
            .await;

        match result {
            Ok(orchestration) => {
                tracing::info!(
                    phase = %orchestration.phase_reached,
                    seed_id = ?orchestration.seed_id,
                    "Orchestration complete"
                );

                // Build response metadata.
                let mut response_metadata = HashMap::new();
                if let Some(ref sid) = orchestration.session_id {
                    response_metadata.insert("session_id".to_owned(), sid.clone());
                }
                response_metadata
                    .insert("phase".to_owned(), orchestration.phase_reached.to_string());
                response_metadata.insert(
                    "evaluation_passed".to_owned(),
                    orchestration.evaluation_passed.to_string(),
                );

                let outgoing = OutgoingMessage::with_metadata(
                    &msg.channel,
                    &msg.user_id,
                    &orchestration.response,
                    response_metadata,
                );
                self.send_to(&msg.channel, outgoing).await?;
            }
            Err(e) => {
                tracing::error!(error = %e, "Orchestration failed");

                let outgoing = OutgoingMessage::new(
                    &msg.channel,
                    &msg.user_id,
                    format!("An error occurred: {e}"),
                );
                self.send_to(&msg.channel, outgoing).await?;
            }
        }

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

    /// Runs the gateway event loop, polling registered channels for incoming messages.
    ///
    /// Each channel is drained completely before moving to the next.
    /// Adaptive sleep: yields immediately when messages are flowing,
    /// pauses for 50 ms when idle — balancing latency and CPU usage.
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Gateway event loop started");

        loop {
            let mut received_any = false;

            // Snapshot channel names to minimise lock hold time.
            let channel_names = {
                let channels = self.channels.read().await;
                channels.keys().cloned().collect::<Vec<_>>()
            };

            for name in &channel_names {
                // Drain every pending message from this channel.
                loop {
                    let msg = {
                        let channels = self.channels.read().await;
                        if let Some(ch) = channels.get(name) {
                            ch.receive().await.ok().flatten()
                        } else {
                            break;
                        }
                    };

                    if let Some(msg) = msg {
                        received_any = true;
                        if let Err(e) = self.route(msg).await {
                            tracing::error!(
                                channel = %name,
                                error = %e,
                                "Failed to route message"
                            );
                        }
                    } else {
                        break; // No more messages from this channel.
                    }
                }
            }

            // Adaptive back-off: busy when messages flow, pause when idle.
            if received_any {
                tokio::task::yield_now().await;
            } else {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
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

// Default impl removed — Gateway always requires an orchestrator.
