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
    /// An Ouroboros phase has started.
    PhaseStarted {
        /// The session this phase belongs to.
        session_id: String,
        /// The phase that started.
        phase: oxios_ouroboros::Phase,
    },
    /// An Ouroboros phase has completed.
    PhaseCompleted {
        /// The session this phase belongs to.
        session_id: String,
        /// The phase that completed.
        phase: oxios_ouroboros::Phase,
        /// A brief summary of the result.
        result_summary: String,
    },
    /// An agent has produced output.
    AgentOutput {
        /// The session this output belongs to.
        session_id: String,
        /// The agent's ID.
        agent_id: AgentId,
        /// The output content.
        output: String,
    },
    /// A HitL approval request has been submitted.
    ApprovalRequested {
        /// The approval request ID.
        id: uuid::Uuid,
        /// The action requiring approval.
        action: String,
        /// The resource involved.
        resource: String,
        /// Reason for the request.
        reason: String,
    },
    /// A HitL approval has been resolved (approved or rejected).
    ApprovalResolved {
        /// The approval request ID.
        id: uuid::Uuid,
        /// Whether it was approved (true) or rejected (false).
        approved: bool,
    },
    /// A memory entry was stored.
    MemoryStored {
        /// Memory entry ID.
        id: String,
        /// Memory type label.
        memory_type: String,
        /// Source of the memory.
        source: String,
    },
    /// Memories were recalled for a new session.
    MemoryRecalled {
        /// The recall query.
        query: String,
        /// Number of memories returned.
        count: usize,
    },
    /// Multi-agent group created.
    AgentGroupCreated {
        /// The group's ID.
        group_id: uuid::Uuid,
        /// Number of agents in the group.
        agent_count: usize,
    },
    /// An agent in a group completed.
    AgentGroupMemberCompleted {
        /// The group's ID.
        group_id: uuid::Uuid,
        /// The agent's ID.
        agent_id: uuid::Uuid,
        /// Whether the agent succeeded.
        success: bool,
    },
}

/// A broadcast-based event bus for kernel events.
///
/// Subscribers receive all events published after they subscribe.
/// Late subscribers do not receive historical events.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<KernelEvent>,
}

impl EventBus {
    /// Creates a new event bus with the given broadcast capacity.
    ///
    /// # Example
    ///
    /// ```
    /// use oxios_kernel::EventBus;
    ///
    /// let bus = EventBus::new(256);
    /// let subscriber = bus.subscribe();
    /// // Subscriber receives all events published after this point.
    /// ```
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
