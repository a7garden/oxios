//! Event bus: inter-agent communication via tokio broadcast channels.
//!
//! The event bus is the "pipe" of Oxios. All agents communicate
//! through kernel events published on the bus.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::audit_trail::{AuditAction, AuditTrail};
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
    /// A new Space has been created.
    SpaceCreated {
        /// The Space's ID.
        space_id: uuid::Uuid,
        /// The Space's name.
        name: String,
        /// How it was created (auto_resource, auto_topic, manual).
        source: String,
    },
    /// Active Space has changed.
    SpaceActivated {
        /// The Space's ID.
        space_id: uuid::Uuid,
        /// The Space's name.
        name: String,
    },
    /// A Space has been archived.
    SpaceArchived {
        /// The Space's ID.
        space_id: uuid::Uuid,
        /// The Space's name.
        name: String,
    },
    /// Cross-Space knowledge was accessed.
    KnowledgeCrossReferenced {
        /// Source Space.
        from_space: uuid::Uuid,
        /// Target Space.
        to_space: uuid::Uuid,
        /// Number of entries accessed.
        entries: usize,
        /// Flow type (reference, transfer, synthesis).
        flow: String,
    },
    /// Spaces have been merged.
    SpacesMerged {
        /// The surviving Space.
        survivor: uuid::Uuid,
        /// The absorbed Space.
        absorbed: uuid::Uuid,
        /// Number of entries migrated.
        entries_migrated: usize,
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
            detail: format!("message: {}", content),
        },
        KernelEvent::SeedCreated { seed_id, .. } => AuditAction::Other {
            detail: format!("seed_created:{}", seed_id),
        },
        KernelEvent::EvaluationComplete { seed_id, passed } => AuditAction::Other {
            detail: format!("evaluation:{}:{}", seed_id, passed),
        },
        KernelEvent::PhaseStarted { session_id, phase } => AuditAction::Other {
            detail: format!("phase_started:{}:{}", session_id, phase),
        },
        KernelEvent::PhaseCompleted {
            session_id,
            phase,
            result_summary,
        } => AuditAction::Other {
            detail: format!(
                "phase_completed:{}:{}:{}",
                session_id, phase, result_summary
            ),
        },
        KernelEvent::AgentOutput { output, .. } => AuditAction::Other {
            detail: format!("agent_output:{}", output),
        },
        KernelEvent::ApprovalRequested {
            id,
            action,
            resource,
            reason: _,
        } => AuditAction::Other {
            detail: format!("approval_requested:{}:{}:{}", id, action, resource),
        },
        KernelEvent::ApprovalResolved { id, approved } => AuditAction::Other {
            detail: format!("approval_resolved:{}:{}", id, approved),
        },
        KernelEvent::MemoryStored {
            id, memory_type, ..
        } => AuditAction::MemoryWrite {
            entry_id: format!("{}:{}", id, memory_type),
        },
        KernelEvent::MemoryRecalled { query, count } => AuditAction::MemoryRead {
            entry_id: format!("query:{}:{}results", query, count),
        },
        KernelEvent::AgentGroupCreated {
            group_id,
            agent_count,
        } => AuditAction::Other {
            detail: format!("group_created:{}:{}agents", group_id, agent_count),
        },
        KernelEvent::AgentGroupMemberCompleted {
            group_id,
            agent_id,
            success,
        } => AuditAction::Other {
            detail: format!(
                "group_member_completed:{}:{}:{}",
                group_id, agent_id, success
            ),
        },
        KernelEvent::SpaceCreated {
            space_id,
            name,
            source,
        } => AuditAction::Other {
            detail: format!("space_created:{}:{}:{}", space_id, name, source),
        },
        KernelEvent::SpaceActivated { space_id, name } => AuditAction::Other {
            detail: format!("space_activated:{}:{}", space_id, name),
        },
        KernelEvent::SpaceArchived { space_id, name } => AuditAction::Other {
            detail: format!("space_archived:{}:{}", space_id, name),
        },
        KernelEvent::KnowledgeCrossReferenced {
            from_space,
            to_space,
            entries,
            flow,
        } => AuditAction::Other {
            detail: format!(
                "knowledge_xref:{}->{}:{}:{}entries",
                from_space, to_space, flow, entries
            ),
        },
        KernelEvent::SpacesMerged {
            survivor,
            absorbed,
            entries_migrated,
        } => AuditAction::Other {
            detail: format!(
                "spaces_merged:{}<-{}:{}entries",
                survivor, absorbed, entries_migrated
            ),
        },
        KernelEvent::EvolutionStarted {
            seed_id,
            new_seed_id,
            iteration,
        } => AuditAction::Other {
            detail: format!("evolution:{}->{}:iter{}", seed_id, new_seed_id, iteration),
        },
        KernelEvent::EvolutionMaxReached {
            seed_id,
            final_score,
            iterations,
        } => AuditAction::Other {
            detail: format!(
                "evolution_max:{}:score={}:iters={}",
                seed_id, final_score, iterations
            ),
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
        KernelEvent::SpaceActivated { space_id, .. } => format!("space:{}", space_id),
        _ => "system".to_string(),
    }
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

    /// Subscribe the audit trail to all kernel events.
    /// This forwards all events to the audit trail as background tasks.
    pub fn attach_audit_trail(&self, audit: Arc<AuditTrail>) {
        let mut rx = self.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let actor = extract_agent_id(&event);
                        let action = kernel_event_to_audit_action(&event);
                        let resource = format!("{:?}", event);
                        audit.append(actor, action, resource);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "Audit trail subscriber lagged, skipping events");
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
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus").finish()
    }
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
    fn test_event_bus_new_and_debug() {
        let bus = EventBus::new(256);
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

        bus.publish(sample_event("multi")).unwrap();

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_subscriber_receives_only_post_subscribe() {
        let bus = EventBus::new(16);

        // Publish before subscribing
        bus.publish(sample_event("before")).unwrap();

        let mut rx = bus.subscribe();

        // Publish after subscribing
        bus.publish(sample_event("after")).unwrap();

        // Should only get the "after" event
        let received = rx.try_recv().expect("should receive event");
        match received {
            KernelEvent::AgentCreated { name, .. } => assert_eq!(name, "after"),
            _ => panic!("wrong event type"),
        }
        // No more events
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_kernel_event_serialization_roundtrip() {
        let events = vec![
            KernelEvent::AgentCreated {
                id: AgentId::new_v4(),
                name: "agent-1".to_string(),
            },
            KernelEvent::AgentFailed {
                id: AgentId::new_v4(),
                error: "timeout".to_string(),
            },
            KernelEvent::SeedCreated {
                seed_id: uuid::Uuid::new_v4(),
            },
            KernelEvent::EvaluationComplete {
                seed_id: uuid::Uuid::new_v4(),
                passed: true,
            },
            KernelEvent::MemoryStored {
                id: "mem-123".to_string(),
                memory_type: "fact".to_string(),
                source: "session".to_string(),
            },
            KernelEvent::MemoryRecalled {
                query: "test query".to_string(),
                count: 5,
            },
            KernelEvent::SpaceCreated {
                space_id: uuid::Uuid::new_v4(),
                name: "my-space".to_string(),
                source: "manual".to_string(),
            },
            KernelEvent::EvolutionStarted {
                seed_id: uuid::Uuid::new_v4(),
                new_seed_id: uuid::Uuid::new_v4(),
                iteration: 2,
            },
            KernelEvent::EvolutionMaxReached {
                seed_id: uuid::Uuid::new_v4(),
                final_score: 0.85,
                iterations: 10,
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let restored: KernelEvent = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&restored).unwrap();
            assert_eq!(json, json2, "roundtrip failed for {:?}", event);
        }
    }

    #[test]
    fn test_kernel_event_to_audit_action() {
        // AgentCreated
        let event = KernelEvent::AgentCreated {
            id: AgentId::new_v4(),
            name: "worker".to_string(),
        };
        let action = kernel_event_to_audit_action(&event);
        match action {
            AuditAction::AgentSpawn { task_type } => assert_eq!(task_type, "worker"),
            _ => panic!("expected AgentSpawn"),
        }

        // AgentFailed
        let event = KernelEvent::AgentFailed {
            id: AgentId::new_v4(),
            error: "OOM".to_string(),
        };
        let action = kernel_event_to_audit_action(&event);
        match action {
            AuditAction::AgentExit { reason } => assert_eq!(reason, "OOM"),
            _ => panic!("expected AgentExit"),
        }

        // MemoryStored
        let event = KernelEvent::MemoryStored {
            id: "m1".to_string(),
            memory_type: "fact".to_string(),
            source: "auto".to_string(),
        };
        let action = kernel_event_to_audit_action(&event);
        match action {
            AuditAction::MemoryWrite { entry_id } => {
                assert!(entry_id.contains("m1"));
                assert!(entry_id.contains("fact"));
            }
            _ => panic!("expected MemoryWrite"),
        }

        // MemoryRecalled
        let event = KernelEvent::MemoryRecalled {
            query: "rust".to_string(),
            count: 3,
        };
        let action = kernel_event_to_audit_action(&event);
        match action {
            AuditAction::MemoryRead { entry_id } => {
                assert!(entry_id.contains("rust"));
                assert!(entry_id.contains("3"));
            }
            _ => panic!("expected MemoryRead"),
        }
    }

    #[test]
    fn test_extract_agent_id() {
        let id = AgentId::new_v4();

        // AgentCreated
        let event = KernelEvent::AgentCreated {
            id,
            name: "a".to_string(),
        };
        assert_eq!(extract_agent_id(&event), id.to_string());

        // AgentStarted
        let event = KernelEvent::AgentStarted { id };
        assert_eq!(extract_agent_id(&event), id.to_string());

        // AgentStopped
        let event = KernelEvent::AgentStopped { id };
        assert_eq!(extract_agent_id(&event), id.to_string());

        // AgentFailed
        let event = KernelEvent::AgentFailed {
            id,
            error: "err".to_string(),
        };
        assert_eq!(extract_agent_id(&event), id.to_string());

        // MessageReceived
        let event = KernelEvent::MessageReceived {
            from: id,
            content: "hello".to_string(),
        };
        assert_eq!(extract_agent_id(&event), id.to_string());

        // SeedCreated → system
        let event = KernelEvent::SeedCreated {
            seed_id: uuid::Uuid::new_v4(),
        };
        assert_eq!(extract_agent_id(&event), "system");

        // SpaceActivated → space: prefix
        let space_id = uuid::Uuid::new_v4();
        let event = KernelEvent::SpaceActivated {
            space_id,
            name: "test".to_string(),
        };
        assert_eq!(extract_agent_id(&event), format!("space:{}", space_id));
    }

    #[tokio::test]
    async fn test_attach_audit_trail_forwards_events() {
        let bus = EventBus::new(64);
        let audit = Arc::new(AuditTrail::new(1000));

        bus.attach_audit_trail(audit.clone());

        bus.publish(KernelEvent::AgentCreated {
            id: AgentId::new_v4(),
            name: "audit-test".to_string(),
        })
        .unwrap();

        // Give the spawned task time to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Audit trail should have at least one entry
        assert!(audit.len() >= 1, "audit trail should have recorded the event");
    }
}
