//! Agent Group API endpoints.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use crate::error::AppError;
use crate::server::AppState;

/// GET /api/agent-groups — List all agent groups from the state store.
pub(crate) async fn handle_agent_groups_list(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let names = state.kernel.list_category("agent_groups").await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut groups = Vec::new();
    for name in &names {
        if let Ok(Some(val)) = state.kernel.load_json::<serde_json::Value>("agent_groups", name).await {
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
    match state.kernel.load_json::<serde_json::Value>("agent_groups", &id).await {
        Ok(Some(val)) => Ok(Json(val)),
        Ok(None) => Err(AppError::NotFound(format!(
            "agent group '{id}' not found"
        ))),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}