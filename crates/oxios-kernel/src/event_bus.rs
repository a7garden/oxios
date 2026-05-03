//! Event bus: inter-agent communication via tokio broadcast channels.
//!
//! The event bus is the "pipe" of Oxios. All agents communicate
//! through kernel events published on the bus.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::types::AgentId;

/// Events that flow through the kernel event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KernelEvent {
    /// A new agent has been created.
    AgentCreated {
        /// The new agent's ID.
        id: AgentId,
        /// The agent's name/goal.
        name: String,
    },
    /// An agent has started executing.
    AgentStarted {
        /// The agent's ID.
        id: AgentId,
    },
    /// An agent has been stopped.
    AgentStopped {
        /// The agent's ID.
        id: AgentId,
    },
    /// An agent has encountered a failure.
    AgentFailed {
        /// The agent's ID.
        id: AgentId,
        /// Description of the error.
        error: String,
    },
    /// A message has been received from an agent.
    MessageReceived {
        /// The sending agent's ID.
        from: AgentId,
        /// Message content.
        content: String,
    },
    /// A new seed has been created.
    SeedCreated {
        /// The seed's ID.
        seed_id: uuid::Uuid,
    },
    /// An evaluation has completed.
    EvaluationComplete {
        /// The seed that was evaluated.
        seed_id: uuid::Uuid,
        /// Whether the evaluation passed.
        passed: bool,
    },
}

/// A broadcast-based event bus for kernel events.
///
/// Subscribers receive all events published after they subscribe.
/// Late subscribers do not receive historical events.
pub struct EventBus {
    sender: broadcast::Sender<KernelEvent>,
}

impl EventBus {
    /// Creates a new event bus with the given broadcast capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to receive kernel events.
    pub fn subscribe(&self) -> broadcast::Receiver<KernelEvent> {
        self.sender.subscribe()
    }

    /// Publish a kernel event to all subscribers.
    pub fn publish(&self, event: KernelEvent) -> Result<()> {
        // It's okay if there are no subscribers.
        let _ = self.sender.send(event);
        Ok(())
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus").finish()
    }
}
