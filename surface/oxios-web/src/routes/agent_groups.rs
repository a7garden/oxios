//! Agent Group API endpoints.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use oxios_kernel::agent_group::{OxiosAgentGroup, OxiosAgentGroupStatus};
use crate::error::AppError;
use crate::server::AppState;

/// GET /api/agent-groups — List all agent groups from the state store.
pub(crate) async fn handle_agent_groups_list(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let names = state
        .kernel
        .state
        .list_category("agent_groups")
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut groups = Vec::new();
    for name in &names {
        if let Ok(Some(val)) = state
            .kernel
            .load_json::<serde_json::Value>("agent_groups", name)
            .await
        {
            groups.push(val);
        }
    }

    Ok(Json(groups))
}

/// GET /api/agent-groups/:id — Get a specific agent group by ID.
pub(crate) async fn handle_agent_group_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state
        .kernel
        .load_json::<serde_json::Value>("agent_groups", &id)
        .await
    {
        Ok(Some(val)) => Ok(Json(val)),
        Ok(None) => Err(AppError::NotFound(format!("agent group '{id}' not found"))),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

/// Derive overall group status from agent statuses.
fn derive_group_status(group: &OxiosAgentGroup) -> &'static str {
    if group.all_completed() {
        "Completed"
    } else if group.any_failed() {
        "Failed"
    } else if group.agents.iter().any(|a| a.status == OxiosAgentGroupStatus::Running) {
        "Running"
    } else {
        "Pending"
    }
}

/// GET /api/agent-groups/{id}/progress — Real-time progress for a group.
pub(crate) async fn handle_agent_group_progress(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let group: OxiosAgentGroup = state
        .kernel
        .load_json::<OxiosAgentGroup>("agent_groups", &id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("agent group '{id}' not found")))?;

    let total = group.agents.len();
    let completed = group.completed_agents().len();
    let failed = group.failed_agents().len();
    let running = group
        .agents
        .iter()
        .filter(|a| matches!(a.status, OxiosAgentGroupStatus::Running))
        .count();
    let pending = group.pending_agents().len();

    Ok(Json(serde_json::json!({
        "id": group.id.to_string(),
        "status": derive_group_status(&group),
        "total_agents": total,
        "completed": completed,
        "failed": failed,
        "pending": pending,
        "running": running,
        "completion_pct": group.completion_pct(),
        "combined_results": group.combined_results(),
    })))
}
