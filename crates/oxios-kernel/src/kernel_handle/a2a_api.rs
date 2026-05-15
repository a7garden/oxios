//! A2A API — agent-to-agent communication facade.

use std::sync::Arc;
use crate::a2a::A2AProtocol;

/// Agent-to-agent communication system calls.
///
/// Wraps [`A2AProtocol`] for inter-agent task delegation and messaging.
pub struct A2aApi {
    protocol: Arc<A2AProtocol>,
}

impl A2aApi {
    /// Create a new A2aApi.
    pub fn new(protocol: Arc<A2AProtocol>) -> Self {
        Self { protocol }
    }

    /// A2A protocol reference.
    pub fn protocol(&self) -> &Arc<A2AProtocol> {
        &self.protocol
    }
}
