//! Event bus: inter-agent communication via `oxi_sdk::EventBus<KernelEvent>`.
//!
//! The event bus is the "pipe" of Oxios. All agents communicate
//! through kernel events published on the bus.
//!
//! After RFC-014 Phase C, this module no longer owns the broadcast channel —
//! it reuses `oxi_sdk::EventBus<E>`, which is a generic wrapper over
//! `tokio::sync::broadcast`. The only Oxios-specific bits are:
//!
//! - `KernelEvent` enum (oxios-internal event vocabulary)
//! - `kernel_event_to_audit_action` mapping for the audit trail
//! - `attach_audit_trail` helper (subscribes the bus to the trail)

use oxi_sdk::observability::{AuditAction, AuditTrail};
use oxi_sdk::EventBus as SdkEventBus;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::types::AgentId;

/// Kernel event bus — generic SDK bus specialised for `KernelEvent`.
///
/// The broadcast channel is owned by `oxi_sdk::EventBus`; this type alias
/// just makes the call sites read more naturally (`crate::event_bus::EventBus`
/// instead of `oxi_sdk::EventBus<KernelEvent>`).
pub type EventBus = SdkEventBus<KernelEvent>;

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
    /// A new Project has been created (RFC-011).
    ProjectCreated {
        /// The project's ID.
        project_id: uuid::Uuid,
        /// The project's name.
        name: String,
        /// How it was created.
        source: String,
    },
    /// A Project has been activated (RFC-011).
    ProjectActivated {
        /// The project's ID.
        project_id: uuid::Uuid,
        /// The project's name.
        name: String,
    },
    /// Evolution has started (evaluate → evolve → re-execute loop).
    EvolutionStarted {
        /// Seed ID before evolution.
        seed_id: uuid::Uuid,
        /// Seed ID after evolution.
        new_seed_id: uuid::Uuid,
        /// Current iteration (0-based).
        iteration: u32,
    },
    /// Evolution loop reached max iterations.
    EvolutionMaxReached {
        /// The final seed ID.
        seed_id: uuid::Uuid,
        /// Final evaluation score.
        final_score: f64,
        /// Number of iterations completed.
        iterations: u32,
    },
}

/// Convert a KernelEvent to an AuditAction for the audit trail.
pub fn kernel_event_to_audit_action(event: &KernelEvent) -> AuditAction {
    match event {
        KernelEvent::AgentCreated { name, .. } => AuditAction::AgentSpawn {
            task_type: name.clone(),
        },
        KernelEvent::AgentStarted { .. } => AuditAction::AgentSpawn {
            task_type: "started".to_string(),
        },
        KernelEvent::AgentStopped { .. } => AuditAction::AgentExit {
            reason: "stopped".to_string(),
        },
        KernelEvent::AgentFailed { error, .. } => AuditAction::AgentExit {
            reason: error.clone(),
        },
        KernelEvent::MessageReceived { content, .. } => AuditAction::Other {
            detail: format!("message: {content}"),
        },
        KernelEvent::SeedCreated { seed_id, .. } => AuditAction::Other {
            detail: format!("seed_created:{seed_id}"),
        },
        KernelEvent::EvaluationComplete { seed_id, passed } => AuditAction::Other {
            detail: format!("evaluation:{seed_id}:{passed}"),
        },
        KernelEvent::PhaseStarted { session_id, phase } => AuditAction::Other {
            detail: format!("phase_started:{session_id}:{phase}"),
        },
        KernelEvent::PhaseCompleted {
            session_id,
            phase,
            result_summary,
        } => AuditAction::Other {
            detail: format!("phase_completed:{session_id}:{phase}:{result_summary}"),
        },
        KernelEvent::AgentOutput { output, .. } => AuditAction::Other {
            detail: format!("agent_output:{output}"),
        },
        KernelEvent::ApprovalRequested {
            id,
            action,
            resource,
            reason: _,
        } => AuditAction::Other {
            detail: format!("approval_requested:{id}:{action}:{resource}"),
        },
        KernelEvent::ApprovalResolved { id, approved } => AuditAction::Other {
            detail: format!("approval_resolved:{id}:{approved}"),
        },
        KernelEvent::MemoryStored {
            id, memory_type, ..
        } => AuditAction::MemoryWrite {
            entry_id: format!("{id}:{memory_type}"),
        },
        KernelEvent::MemoryRecalled { query, count } => AuditAction::MemoryRead {
            entry_id: format!("query:{query}:{count}results"),
        },
        KernelEvent::AgentGroupCreated {
            group_id,
            agent_count,
        } => AuditAction::Other {
            detail: format!("group_created:{group_id}:{agent_count}agents"),
        },
        KernelEvent::AgentGroupMemberCompleted {
            group_id,
            agent_id,
            success,
        } => AuditAction::Other {
            detail: format!("group_member_completed:{group_id}:{agent_id}:{success}"),
        },
        KernelEvent::EvolutionStarted {
            seed_id,
            new_seed_id,
            iteration,
        } => AuditAction::Other {
            detail: format!("evolution:{seed_id}->{new_seed_id}:iter{iteration}"),
        },
        KernelEvent::EvolutionMaxReached {
            seed_id,
            final_score,
            iterations,
        } => AuditAction::Other {
            detail: format!("evolution_max:{seed_id}:score={final_score}:iters={iterations}"),
        },
        KernelEvent::ProjectCreated {
            project_id: _,
            name,
            source,
        } => AuditAction::Other {
            detail: format!("project_created:{name}:{source}"),
        },
        KernelEvent::ProjectActivated {
            project_id: _,
            name,
        } => AuditAction::Other {
            detail: format!("project_activated:{name}"),
        },
    }
}

/// Extract agent ID from a KernelEvent variant.
fn extract_agent_id(event: &KernelEvent) -> String {
    match event {
        KernelEvent::AgentCreated { id, .. } => id.to_string(),
        KernelEvent::AgentStarted { id, .. } => id.to_string(),
        KernelEvent::AgentStopped { id, .. } => id.to_string(),
        KernelEvent::AgentFailed { id, .. } => id.to_string(),
        KernelEvent::MessageReceived { from, .. } => from.to_string(),
        KernelEvent::AgentOutput { agent_id, .. } => agent_id.to_string(),
        KernelEvent::AgentGroupMemberCompleted { agent_id, .. } => agent_id.to_string(),
        KernelEvent::ProjectActivated { project_id, .. } => format!("project:{project_id}"),
        _ => "system".to_string(),
    }
}

/// Subscribe the audit trail to all kernel events.
///
/// The bus is broadcast-based; this spawns a long-running task that
/// forwards every event into the audit trail as a structured entry.
/// Lagged subscribers are logged and recovered.
pub fn attach_audit_trail(bus: &EventBus, audit: Arc<AuditTrail>) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let actor = extract_agent_id(&event);
                    let action = kernel_event_to_audit_action(&event);
                    let resource = format!("{event:?}");
                    audit.append(actor, action, resource);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        skipped = n,
                        "Audit trail subscriber lagged, skipping events"
                    );
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("Audit trail event bus closed, exiting");
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(name: &str) -> KernelEvent {
        KernelEvent::AgentCreated {
            id: AgentId::new_v4(),
            name: name.to_string(),
        }
    }

    #[test]
    fn test_event_bus_uses_sdk() {
        let bus: EventBus = EventBus::new(256);
        assert!(format!("{:?}", bus).contains("EventBus"));
    }

    #[tokio::test]
    async fn test_publish_no_subscribers_ok() {
        let bus = EventBus::new(16);
        let result = bus.publish(sample_event("orphan"));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_single_subscriber_receives_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let event = sample_event("test-agent");
        bus.publish(event.clone()).unwrap();

        let received = rx.try_recv().expect("should receive event");
        match received {
            KernelEvent::AgentCreated { name, .. } => assert_eq!(name, "test-agent"),
            _ => panic!("wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers_receive_events() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let event = sample_event("multi");
        bus.publish(event.clone()).unwrap();

        let r1 = rx1.try_recv().expect("rx1 should receive event");
        let r2 = rx2.try_recv().expect("rx2 should receive event");

        assert!(matches!(r1, KernelEvent::AgentCreated { .. }));
        assert!(matches!(r2, KernelEvent::AgentCreated { .. }));
    }

    #[tokio::test]
    async fn test_kernel_event_to_audit_action() {
        let event = KernelEvent::AgentFailed {
            id: AgentId::new_v4(),
            error: "boom".to_string(),
        };
        let action = kernel_event_to_audit_action(&event);
        match action {
            AuditAction::AgentExit { reason } => assert_eq!(reason, "boom"),
            other => panic!("expected AgentExit, got {other:?}"),
        }
    }
}
