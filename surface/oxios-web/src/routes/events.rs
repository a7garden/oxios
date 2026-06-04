use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, Sse};
use axum::Json;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as TokioStreamExt;

use crate::error::AppError;
use crate::routes::{paginate, PageParams};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// Session summary for listing (lightweight version without full history).
#[derive(Debug, Serialize, Clone)]
pub(crate) struct SessionListItem {
    id: String,
    user_id: String,
    project_id: Option<String>,
    message_count: usize,
    active_seed_id: Option<String>,
    created_at: String,
    updated_at: String,
}

/// GET /api/sessions — List recent sessions (paginated).
pub(crate) async fn handle_sessions_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    match state.kernel.state.list_sessions().await {
        Ok(sessions) => {
            let items: Vec<SessionListItem> = sessions
                .into_iter()
                .map(|s| SessionListItem {
                    id: s.id,
                    user_id: s.user_id,
                    project_id: s.project_id,
                    message_count: s.message_count,
                    active_seed_id: s.active_seed_id,
                    created_at: s.created_at.to_rfc3339(),
                    updated_at: s.updated_at.to_rfc3339(),
                })
                .collect();
            Json(paginate(&items, &params))
        }
        Err(_) => Json(paginate(&Vec::<SessionListItem>::new(), &params)),
    }
}

/// GET /api/sessions/:id — Get session with full message history.
pub(crate) async fn handle_session_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use oxios_kernel::state_store::SessionId;
    let session_id = SessionId(id);
    match state.kernel.state.load_session(&session_id).await {
        Ok(Some(session)) => Ok(Json(serde_json::json!({
            "id": session.id.0,
            "user_id": session.user_id,
            "project_id": session.metadata.get("project_ids").and_then(|v| v.as_str()).map(String::from),
            "user_messages": session.user_messages,
            "agent_responses": session.agent_responses,
            "active_seed_id": session.active_seed_id,
            "active_persona_id": session.active_persona_id,
            "created_at": session.created_at.to_rfc3339(),
            "updated_at": session.updated_at.to_rfc3339(),
            "metadata": session.metadata,
            // RFC-015: trajectory for chat transparency replay.
            "trajectory_steps": session.trajectory_steps,
        }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// DELETE /api/sessions/:id — Delete a session.
pub(crate) async fn handle_session_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use oxios_kernel::state_store::SessionId;
    let session_id = SessionId(id);
    match state.kernel.state.delete_session(&session_id).await {
        Ok(true) => Ok(Json(serde_json::json!({
            "status": "deleted",
            "id": session_id.0,
        }))),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// POST /api/sessions/prune — Prune sessions based on config.
///
/// Removes sessions that exceed TTL or exceed the maximum count.
pub(crate) async fn handle_sessions_prune(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    use oxios_kernel::state_store::PruneConfig;
    let prune_config = {
        let cfg = state.config.read();
        PruneConfig {
            max_sessions: cfg.session.max_sessions,
            ttl_hours: cfg.session.ttl_hours,
        }
    }; // cfg guard dropped here

    let pruned = state
        .kernel
        .state
        .prune_sessions(&prune_config)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "pruned",
        "count": pruned,
        "config": {
            "max_sessions": prune_config.max_sessions,
            "ttl_hours": prune_config.ttl_hours,
        },
    })))
}

// ---------------------------------------------------------------------------
// Events (SSE)
// ---------------------------------------------------------------------------

/// GET /api/events — SSE stream of KernelEvent.
pub(crate) async fn handle_events(
    state: State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let receiver = state.kernel.infra.subscribe();
    let stream = BroadcastStream::new(receiver);
    let stream = TokioStreamExt::filter_map(stream, |result| {
        match result {
            Ok(event) => {
                // Sanitize events: include type and basic metadata only.
                // Detailed data (full seed content, LLM responses) is excluded.
                let sanitized = sanitize_event(&event);
                let data = serde_json::to_string(&sanitized).unwrap_or_default();
                Some(Ok(SseEvent::default().data(data)))
            }
            Err(_) => None, // Skip lagged messages
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("ping"),
    )
}

/// Sanitize a kernel event for SSE broadcast.
/// Returns only the event type and non-sensitive metadata.
pub(crate) fn sanitize_event(event: &oxios_kernel::event_bus::KernelEvent) -> serde_json::Value {
    use oxios_kernel::event_bus::KernelEvent;
    let now = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let base = serde_json::json!({
        "id": id,
        "timestamp": now,
    });
    let payload = match event {
        KernelEvent::AgentCreated { id, name } => serde_json::json!({
            "type": "agent_created",
            "agent_id": id.to_string(),
            "name": name,
        }),
        KernelEvent::AgentStarted { id } => serde_json::json!({
            "type": "agent_started",
            "agent_id": id.to_string(),
        }),
        KernelEvent::AgentStopped { id } => serde_json::json!({
            "type": "agent_stopped",
            "agent_id": id.to_string(),
        }),
        KernelEvent::AgentFailed { id, error } => serde_json::json!({
            "type": "agent_failed",
            "agent_id": id.to_string(),
            "error": error,
        }),
        KernelEvent::MessageReceived { from, .. } => serde_json::json!({
            "type": "message_received",
            "from": from.to_string(),
            // content excluded — may contain sensitive data
        }),
        KernelEvent::SeedCreated { seed_id } => serde_json::json!({
            "type": "seed_created",
            "seed_id": seed_id.to_string(),
        }),
        KernelEvent::EvaluationComplete { seed_id, passed } => serde_json::json!({
            "type": "evaluation_complete",
            "seed_id": seed_id.to_string(),
            "passed": passed,
        }),
        KernelEvent::PhaseStarted { phase, .. } => serde_json::json!({
            "type": "phase_started",
            "phase": format!("{phase:?}"),
        }),
        KernelEvent::PhaseCompleted { phase, .. } => serde_json::json!({
            "type": "phase_completed",
            "phase": format!("{phase:?}"),
        }),
        KernelEvent::AgentOutput {
            session_id,
            agent_id,
            ..
        } => serde_json::json!({
            "type": "agent_output",
            "session_id": session_id,
            "agent_id": agent_id.to_string(),
            // content excluded
        }),
        KernelEvent::ApprovalRequested {
            id,
            action,
            resource,
            ..
        } => serde_json::json!({
            "type": "approval_requested",
            "id": id.to_string(),
            "action": action,
            "resource": resource,
        }),
        KernelEvent::ApprovalResolved { id, approved } => serde_json::json!({
            "type": "approval_resolved",
            "id": id.to_string(),
            "approved": approved,
        }),
        KernelEvent::MemoryStored {
            id,
            memory_type,
            source,
        } => serde_json::json!({
            "type": "memory_stored",
            "id": id,
            "memory_type": memory_type,
            "source": source,
        }),
        KernelEvent::MemoryRecalled { query, count } => serde_json::json!({
            "type": "memory_recalled",
            "query": query,
            "count": count,
        }),
        KernelEvent::AgentGroupCreated {
            group_id,
            agent_count,
        } => serde_json::json!({
            "type": "agent_group_created",
            "group_id": group_id.to_string(),
            "agent_count": agent_count,
        }),
        KernelEvent::AgentGroupMemberCompleted {
            group_id,
            agent_id,
            success,
        } => serde_json::json!({
            "type": "agent_group_member_completed",
            "group_id": group_id.to_string(),
            "agent_id": agent_id.to_string(),
            "success": success,
        }),
        KernelEvent::ProjectCreated {
            project_id,
            name,
            source,
        } => serde_json::json!({
            "type": "project_created",
            "project_id": project_id.to_string(),
            "name": name,
            "source": source,
        }),
        KernelEvent::ProjectActivated { project_id, name } => serde_json::json!({
            "type": "project_activated",
            "project_id": project_id.to_string(),
            "name": name,
        }),
        KernelEvent::EvolutionStarted {
            seed_id,
            new_seed_id,
            iteration,
        } => serde_json::json!({
            "type": "evolution_started",
            "seed_id": seed_id.to_string(),
            "new_seed_id": new_seed_id.to_string(),
            "iteration": iteration,
        }),
        KernelEvent::EvolutionMaxReached {
            seed_id,
            final_score,
            iterations,
        } => serde_json::json!({
            "type": "evolution_max_reached",
            "seed_id": seed_id.to_string(),
            "final_score": final_score,
            "iterations": iterations,
        }),
        // ── RFC-015: chat transparency events (forwarded to /api/events too) ──
        KernelEvent::ToolExecutionStarted {
            session_id,
            tool_name,
            tool_call_id,
            tool_args,
        } => serde_json::json!({
            "type": "tool_started",
            "session_id": session_id,
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "tool_args": tool_args,
        }),
        KernelEvent::ToolExecutionFinished {
            session_id,
            tool_call_id,
            tool_name,
            duration_ms,
            is_error,
            output_summary,
        } => serde_json::json!({
            "type": "tool_finished",
            "session_id": session_id,
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "duration_ms": duration_ms,
            "is_error": is_error,
            "output_summary": output_summary,
        }),
        KernelEvent::ToolExecutionProgress {
            session_id,
            tool_call_id,
            tool_name,
            progress,
        } => serde_json::json!({
            "type": "tool_progress",
            "session_id": session_id,
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "progress": progress,
        }),
        KernelEvent::MemoryRecallUsed {
            session_id,
            query,
            count,
            source,
        } => serde_json::json!({
            "type": "memory_recall_used",
            "session_id": session_id,
            "query": query,
            "count": count,
            "source": source,
        }),
        KernelEvent::TokenUsageUpdate {
            session_id,
            input_tokens,
            output_tokens,
        } => serde_json::json!({
            "type": "token_usage_update",
            "session_id": session_id,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        }),
        KernelEvent::ReasoningFragment {
            session_id,
            content,
            source,
        } => serde_json::json!({
            "type": "reasoning_fragment",
            "session_id": session_id,
            "content": content,
            "source": source,
        }),
    };
    // Merge payload into base
    if let serde_json::Value::Object(mut map) = base {
        if let serde_json::Value::Object(payload_map) = payload {
            for (k, v) in payload_map {
                map.insert(k, v);
            }
        }
        serde_json::Value::Object(map)
    } else {
        payload
    }
}
// ---------------------------------------------------------------------------

/// Approval request for the API response.
#[derive(Debug, Serialize)]
pub(crate) struct ApprovalResponse {
    id: String,
    subject: String,
    action: String,
    resource: String,
    reason: String,
    created_at: String,
    status: String,
}

/// GET /api/approvals — List all approval requests (pending + history).
pub(crate) async fn handle_approvals_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<ApprovalResponse>> {
    let approvals: Vec<ApprovalResponse> = state
        .kernel
        .security
        .list_approvals()
        .iter()
        .map(|(p, s)| {
            let subject_str = match &p.subject {
                oxios_kernel::access_manager::Subject::User(n) => format!("user:{n}"),
                oxios_kernel::access_manager::Subject::Agent(id) => format!("agent:{id}"),
                oxios_kernel::access_manager::Subject::System => "system".into(),
            };
            let action_str = match &p.action {
                oxios_kernel::access_manager::Action::UseTool(t) => format!("use_tool:{t}"),
                oxios_kernel::access_manager::Action::AccessPath(p) => format!("access_path:{p}"),
                oxios_kernel::access_manager::Action::ManageAgents => "manage_agents".into(),
                oxios_kernel::access_manager::Action::ManagePrograms => "manage_programs".into(),
                oxios_kernel::access_manager::Action::ManageWorkspaces => {
                    "manage_workspaces".into()
                }
                oxios_kernel::access_manager::Action::ManageRBAC => "manage_rbac".into(),
                oxios_kernel::access_manager::Action::ViewAuditLog => "view_audit_log".into(),
                oxios_kernel::access_manager::Action::SystemConfig => "system_config".into(),
            };
            let status_str = match s {
                oxios_kernel::access_manager::ApprovalStatus::Pending => "pending",
                oxios_kernel::access_manager::ApprovalStatus::Approved => "approved",
                oxios_kernel::access_manager::ApprovalStatus::Rejected => "rejected",
                oxios_kernel::access_manager::ApprovalStatus::Expired => "expired",
            };
            ApprovalResponse {
                id: p.id.to_string(),
                subject: subject_str,
                action: action_str,
                resource: p.resource.clone(),
                reason: p.reason.clone(),
                created_at: p.created_at.to_rfc3339(),
                status: status_str.to_string(),
            }
        })
        .collect();
    Json(approvals)
}

/// POST /api/approvals/:id/approve — Approve a pending request.
pub(crate) async fn handle_approval_approve(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    if state.kernel.security.approve(uuid) {
        tracing::info!(approval_id = %uuid, "Approval granted");
        // Publish event so SSE clients update automatically
        let _ =
            state
                .kernel
                .infra
                .publish(oxios_kernel::event_bus::KernelEvent::ApprovalResolved {
                    id: uuid,
                    approved: true,
                });
        Ok(Json(serde_json::json!({
            "status": "approved",
            "id": id,
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// POST /api/approvals/:id/reject — Reject a pending request.
pub(crate) async fn handle_approval_reject(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    if state.kernel.security.reject(uuid) {
        tracing::info!(approval_id = %uuid, "Approval rejected");
        // Publish event so SSE clients update automatically
        let _ =
            state
                .kernel
                .infra
                .publish(oxios_kernel::event_bus::KernelEvent::ApprovalResolved {
                    id: uuid,
                    approved: false,
                });
        Ok(Json(serde_json::json!({
            "status": "rejected",
            "id": id,
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
