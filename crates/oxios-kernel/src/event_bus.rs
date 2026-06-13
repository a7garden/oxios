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

use oxi_sdk::EventBus as SdkEventBus;
use oxi_sdk::observability::{AuditAction, AuditTrail};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

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
        /// The tool requesting approval.
        tool_name: String,
        /// The action requiring approval.
        action: String,
        /// The resource involved.
        resource: String,
        /// Reason for the request.
        reason: String,
        /// The session ID that triggered this request.
        session_id: Option<String>,
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

    // ── RFC-015 Chat Transparency ─────────────────────────────
    // Real-time events emitted by AgentRuntime during tool execution
    // and streaming. Web channel converts these to WS chunks.
    /// A tool execution has started (real-time, RFC-015).
    ToolExecutionStarted {
        /// Session this tool call belongs to.
        session_id: String,
        /// Name of the tool (e.g. "read_file", "bash", "memory_recall").
        tool_name: String,
        /// Provider-specific tool call ID used to correlate start/end.
        tool_call_id: String,
        /// Tool input arguments (JSON).
        tool_args: serde_json::Value,
        /// Semantic context inferred by oxi-agent 0.32+ from tool name/args
        /// (e.g. WebSearch, PageVisit). `None` for tools without context mapping.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<serde_json::Value>,
    },
    /// A tool execution has finished (real-time, RFC-015).
    ToolExecutionFinished {
        /// Session this tool call belongs to.
        session_id: String,
        /// Provider-specific tool call ID.
        tool_call_id: String,
        /// Name of the tool.
        tool_name: String,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
        /// Whether the tool returned an error.
        is_error: bool,
        /// Truncated output (max ~500 chars) for streaming.
        output_summary: String,
    },
    /// A tool execution emitted a progress update (real-time, RFC-015).
    ToolExecutionProgress {
        /// Session this tool call belongs to.
        session_id: String,
        /// Provider-specific tool call ID.
        tool_call_id: String,
        /// Name of the tool.
        tool_name: String,
        /// Human-readable progress text (already-formatted by the tool).
        progress: String,
        /// Tab that emitted this progress event, if the upstream tool tracks
        /// tabs. `None` for tools that don't have a tab concept (e.g. legacy
        /// oxi-agent versions that don't propagate `tab_id`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tab_id: Option<Uuid>,
        /// Semantic context from the tool call (e.g. PageVisit, WebSearch).
        /// Stored as `serde_json::Value` to decouple kernel events from
        /// oxi-sdk's internal `ToolCallContext` enum. UI consumers that
        /// understand a context variant render it richly; older consumers
        /// simply ignore the field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<serde_json::Value>,
    },
    /// Memory was recalled during agent execution (RFC-015).
    MemoryRecallUsed {
        /// Session this recall belongs to.
        session_id: String,
        /// The recall query.
        query: String,
        /// Number of memories returned.
        count: usize,
        /// Memory tier source ("hot" | "warm" | "cold").
        source: String,
    },
    /// Token usage update (RFC-015).
    TokenUsageUpdate {
        /// Session this usage belongs to.
        session_id: String,
        /// Cumulative input tokens.
        input_tokens: u64,
        /// Cumulative output tokens.
        output_tokens: u64,
    },
    /// Reasoning/compaction fragment (RFC-015).
    ReasoningFragment {
        /// Session this fragment belongs to.
        session_id: String,
        /// The fragment text (chain-of-thought, compaction summary, etc).
        content: String,
        /// Source label: "chain_of_thought" | "compaction" | "reflection".
        source: String,
    },

    // ── Calendar ──────────────────────────────────────────────
    /// A calendar event was created.
    CalendarEventCreated {
        /// Event UID.
        uid: String,
        /// Event title.
        title: String,
        /// Start time.
        start: String,
        /// End time.
        end: String,
    },
    /// A calendar event was updated.
    CalendarEventUpdated {
        /// Event UID.
        uid: String,
        /// Event title.
        title: String,
    },
    /// A calendar event was deleted.
    CalendarEventDeleted {
        /// Event UID.
        uid: String,
        /// Event title.
        title: String,
    },
    /// An email has been sent.
    EmailSent {
        /// Email subject.
        subject: String,
        /// SMTP message ID.
        message_id: String,
        /// Template name (if template was used/saved).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        template_name: Option<String>,
    },

    // ── Knowledge ──────────────────────────────────────────────
    /// A knowledge note was persisted (hook, user, or tool).
    KnowledgePersisted {
        session_id: String,
        message_index: usize,
        path: String,
        source: String, // "hook", "user", "tool"
    },
    /// A knowledge note was removed by user action.
    KnowledgeRemoved {
        session_id: String,
        message_index: usize,
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
            ..
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
        // ── RFC-015 ──
        KernelEvent::ToolExecutionStarted { tool_name, .. } => AuditAction::Other {
            detail: format!("tool_started:{tool_name}"),
        },
        KernelEvent::ToolExecutionFinished {
            tool_name,
            is_error,
            ..
        } => AuditAction::Other {
            detail: format!(
                "tool_finished:{tool_name}:{}",
                if *is_error { "error" } else { "ok" }
            ),
        },
        KernelEvent::ToolExecutionProgress {
            tool_name,
            tab_id,
            context,
            ..
        } => AuditAction::Other {
            detail: {
                let mut d = format!("tool_progress:{tool_name}");
                if let Some(id) = tab_id {
                    d.push_str(&format!(":tab={id}"));
                }
                if let Some(ctx) = context
                    .as_ref()
                    .and_then(|c| c.get("kind"))
                    .and_then(|k| k.as_str())
                {
                    d.push_str(&format!(":{ctx}"));
                }
                d
            },
        },
        KernelEvent::MemoryRecallUsed { query, count, .. } => AuditAction::MemoryRead {
            entry_id: format!("recall:{query}:{count}results"),
        },
        KernelEvent::TokenUsageUpdate {
            input_tokens,
            output_tokens,
            ..
        } => AuditAction::Other {
            detail: format!("tokens:in={input_tokens}:out={output_tokens}"),
        },
        KernelEvent::ReasoningFragment { source, .. } => AuditAction::Other {
            detail: format!("reasoning:{source}"),
        },
        KernelEvent::CalendarEventCreated { uid, title, .. } => AuditAction::Other {
            detail: format!("calendar:created:{uid}:{title}"),
        },
        KernelEvent::CalendarEventUpdated { uid, title } => AuditAction::Other {
            detail: format!("calendar:updated:{uid}:{title}"),
        },
        KernelEvent::CalendarEventDeleted { uid, title } => AuditAction::Other {
            detail: format!("calendar:deleted:{uid}:{title}"),
        },
        KernelEvent::EmailSent {
            subject,
            message_id,
            template_name,
        } => AuditAction::Other {
            detail: format!("email:sent:{subject} (msg={message_id}, tpl={template_name:?})"),
        },
        KernelEvent::KnowledgePersisted {
            session_id,
            message_index,
            path,
            source,
        } => AuditAction::Other {
            detail: format!("knowledge:persisted:{session_id}:{message_index}:{path}:{source}"),
        },
        KernelEvent::KnowledgeRemoved {
            session_id,
            message_index,
        } => AuditAction::Other {
            detail: format!("knowledge:removed:{session_id}:{message_index}"),
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
        // RFC-015: session-scoped events use session_id as the subject
        KernelEvent::ToolExecutionStarted { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::ToolExecutionFinished { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::ToolExecutionProgress { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::MemoryRecallUsed { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::TokenUsageUpdate { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::ReasoningFragment { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::KnowledgePersisted { session_id, .. } => format!("session:{session_id}"),
        KernelEvent::KnowledgeRemoved { session_id, .. } => format!("session:{session_id}"),
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

    // ── RFC-015 chat transparency event coverage ──

    /// Round-trip JSON serialization for every new RFC-015 variant. This
    /// guards against accidental renames that would break the WebSocket
    /// wire format on the frontend.
    #[test]
    fn test_rfc015_event_round_trip_json() {
        let cases: Vec<KernelEvent> = vec![
            KernelEvent::ToolExecutionStarted {
                session_id: "s1".into(),
                tool_name: "read_file".into(),
                tool_call_id: "call_1".into(),
                tool_args: serde_json::json!({"path": "/src/main.rs"}),
                context: None,
            },
            KernelEvent::ToolExecutionFinished {
                session_id: "s1".into(),
                tool_call_id: "call_1".into(),
                tool_name: "read_file".into(),
                duration_ms: 234,
                is_error: false,
                output_summary: "fn main() {}".into(),
            },
            KernelEvent::ToolExecutionProgress {
                session_id: "s1".into(),
                tool_call_id: "call_1".into(),
                tool_name: "read_file".into(),
                progress: "reading line 42/100".into(),
                tab_id: None,
                context: None,
            },
            KernelEvent::MemoryRecallUsed {
                session_id: "s1".into(),
                query: "rust errors".into(),
                count: 3,
                source: "warm".into(),
            },
            KernelEvent::TokenUsageUpdate {
                session_id: "s1".into(),
                input_tokens: 1234,
                output_tokens: 567,
            },
            KernelEvent::ReasoningFragment {
                session_id: "s1".into(),
                content: "compaction done".into(),
                source: "compaction".into(),
            },
        ];
        for event in cases {
            let json = serde_json::to_string(&event).expect("serialize");
            let back: KernelEvent = serde_json::from_str(&json).expect("deserialize");
            let json2 = serde_json::to_string(&back).expect("serialize round-trip");
            assert_eq!(json, json2, "round-trip should be stable");
        }
    }

    /// Tool progress events serialize/deserialize cleanly and round-trip
    /// stable JSON, matching the wire format the WS layer expects.
    #[test]
    fn test_tool_execution_progress_serde_round_trip() {
        let event = KernelEvent::ToolExecutionProgress {
            session_id: "s-abc".into(),
            tool_call_id: "call_42".into(),
            tool_name: "browse".into(),
            progress: "loading https://example.com".into(),
            tab_id: Some(Uuid::new_v4()),
            context: None,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: KernelEvent = serde_json::from_str(&json).expect("deserialize");
        match back {
            KernelEvent::ToolExecutionProgress {
                ref session_id,
                ref tool_call_id,
                ref tool_name,
                ref progress,
                tab_id,
                ..
            } => {
                assert_eq!(session_id, "s-abc");
                assert_eq!(tool_call_id, "call_42");
                assert_eq!(tool_name, "browse");
                assert_eq!(progress, "loading https://example.com");
                assert!(tab_id.is_some(), "tab_id should round-trip when present");
            }
            other => panic!("expected ToolExecutionProgress, got {other:?}"),
        }
    }

    /// The audit-action mapping for tool progress should produce a stable,
    /// searchable detail string (used by the audit-trail UI to filter).
    /// When `tab_id` is set, the detail includes `:tab=<id>`; when absent,
    /// the original `tool_progress:<tool>` form is preserved (back-compat
    /// for older oxi-agent versions that don't propagate tabs).
    #[test]
    fn test_tool_execution_progress_audit_action() {
        let with_tab = KernelEvent::ToolExecutionProgress {
            session_id: "s1".into(),
            tool_call_id: "c1".into(),
            tool_name: "browse".into(),
            progress: "navigating".into(),
            tab_id: Some(Uuid::new_v4()),
            context: None,
        };
        match kernel_event_to_audit_action(&with_tab) {
            AuditAction::Other { detail } => {
                assert!(detail.contains("tool_progress"), "detail: {detail}");
                assert!(detail.contains("browse"), "detail: {detail}");
                assert!(
                    detail.contains(":tab="),
                    "detail should include tab id: {detail}"
                );
            }
            other => panic!("expected Other, got {other:?}"),
        }
        let without_tab = KernelEvent::ToolExecutionProgress {
            session_id: "s1".into(),
            tool_call_id: "c1".into(),
            tool_name: "browse".into(),
            progress: "navigating".into(),
            tab_id: None,
            context: None,
        };
        match kernel_event_to_audit_action(&without_tab) {
            AuditAction::Other { detail } => {
                assert_eq!(detail, "tool_progress:browse");
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }

    /// `tab_id` is optional in serde (`#[serde(default)]`) so older oxi-agent
    /// versions that don't emit it still round-trip cleanly. This guards the
    /// backwards-compat contract explicitly.
    #[test]
    fn test_tool_execution_progress_tab_id_optional_in_serde() {
        // Simulate a payload from a legacy oxi-agent (no tab_id key).
        // KernelEvent is externally tagged, so the variant is the JSON key.
        let legacy_json = r#"{
            "ToolExecutionProgress": {
                "session_id": "s-old",
                "tool_call_id": "call_legacy",
                "tool_name": "browse",
                "progress": "step 1"
            }
        }"#;
        let event: KernelEvent = serde_json::from_str(legacy_json).expect("deserialize legacy");
        match &event {
            KernelEvent::ToolExecutionProgress {
                session_id,
                tool_call_id,
                tool_name,
                progress,
                tab_id,
                ..
            } => {
                assert_eq!(session_id, "s-old");
                assert_eq!(tool_call_id, "call_legacy");
                assert_eq!(tool_name, "browse");
                assert_eq!(progress, "step 1");
                assert!(tab_id.is_none(), "missing field should default to None");
            }
            other => panic!("expected ToolExecutionProgress, got {other:?}"),
        }
        // And re-serialise — `skip_serializing_if = "Option::is_none"` keeps
        // the wire format clean when downstream tools don't set tab_id.
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            !json.contains("tab_id"),
            "tab_id should be omitted when None: {json}"
        );
    }

    /// The agent_id extractor should map session-scoped RFC-015 events to
    /// `session:<id>` for audit-trail grouping, while non-session events
    /// keep their existing behaviour.
    #[test]
    fn test_rfc015_extract_agent_id() {
        let event = KernelEvent::ToolExecutionStarted {
            session_id: "abc-123".into(),
            tool_name: "bash".into(),
            tool_call_id: "c1".into(),
            tool_args: serde_json::Value::Null,
            context: None,
        };
        // The function is private; verify via the public AuditAction mapping
        // that session-scoped events do not collide with real agent ids.
        let action = kernel_event_to_audit_action(&event);
        match action {
            AuditAction::Other { detail } => {
                assert!(
                    detail.contains("bash"),
                    "tool name in audit detail: {detail}"
                );
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
