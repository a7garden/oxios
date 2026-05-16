//! A2A API — agent-to-agent communication facade.

use std::sync::Arc;
use crate::a2a::A2AProtocol;

/// Agent-to-agent communication system calls.
///
/// Wraps [`A2AProtocol`] for inter-agent task delegation and messaging.
/// Also exposes `oxi_sdk::MessageBus` for broadcast-based inter-agent communication.
pub struct A2aApi {
    protocol: Arc<A2AProtocol>,
    /// Broadcast-based message bus for inter-agent communication.
    message_bus: oxi_sdk::MessageBus,
}

impl A2aApi {
    /// Create a new A2aApi with a MessageBus (capacity 256).
    pub fn new(protocol: Arc<A2AProtocol>) -> Self {
        Self {
            protocol,
            message_bus: oxi_sdk::MessageBus::new(256),
        }
    }

    /// A2A protocol reference.
    pub fn protocol(&self) -> &Arc<A2AProtocol> {
        &self.protocol
    }

    /// Message bus reference for inter-agent broadcast messaging.
    pub fn message_bus(&self) -> &oxi_sdk::MessageBus {
        &self.message_bus
    }

    /// Subscribe to messages on the bus.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<oxi_sdk::InterAgentMessage> {
        self.message_bus.subscribe()
    }
}