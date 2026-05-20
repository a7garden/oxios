//! Space management routes — list, activate, archive, merge, knowledge flow.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::error::AppError;
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Space list
// ---------------------------------------------------------------------------

/// GET /api/spaces — List all Spaces.
pub(crate) async fn handle_spaces_list(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let spaces = state.kernel.spaces.list_spaces();
    Ok(Json(serde_json::json!({
        "items": spaces,
        "total": spaces.len(),
    })))
}

// ---------------------------------------------------------------------------
// Current Space
// ---------------------------------------------------------------------------

/// GET /api/spaces/current — Get the active Space.
pub(crate) async fn handle_space_current(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.kernel.spaces.current_space() {
        Some(space) => Ok(Json(serde_json::to_value(&space).unwrap_or_default())),
        None => Ok(Json(serde_json::json!(null))),
    }
}

// ---------------------------------------------------------------------------
// Space detail
// ---------------------------------------------------------------------------

/// GET /api/spaces/:id — Get Space details.
pub(crate) async fn handle_space_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.kernel.spaces.get_space(&id).await {
        Some(space) => Ok(Json(serde_json::to_value(&space).unwrap_or_default())),
        None => Err(AppError::NotFound(format!("Space {id} not found"))),
    }
}

// ---------------------------------------------------------------------------
// Activate
// ---------------------------------------------------------------------------

/// POST /api/spaces/:id/activate — Activate a Space.
pub(crate) async fn handle_space_activate(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .activate_space(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "space_id": id })))
}

// ---------------------------------------------------------------------------
// Archive
// ---------------------------------------------------------------------------

/// POST /api/spaces/:id/archive — Archive a Space.
pub(crate) async fn handle_space_archive(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .spaces
        .archive(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "space_id": id })))
}

// ---------------------------------------------------------------------------
// Restore
// ---------------------------------------------------------------------------

/// POST /api/spaces/:id/restore — Restore an archived Space.
pub(crate) async fn handle_space_restore(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .spaces
        .restore(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "space_id": id })))
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge request body.
#[derive(Debug, Deserialize)]
pub(crate) struct MergeRequest {
    /// Survivor Space ID (absorbs the other).
    pub survivor_id: String,
    /// Absorbed Space ID (will be merged into survivor).
    pub absorbed_id: String,
}

/// POST /api/spaces/merge — Merge two Spaces.
pub(crate) async fn handle_space_merge(
    state: State<Arc<AppState>>,
    Json(body): Json<MergeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .spaces
        .merge(&body.survivor_id, &body.absorbed_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "survivor_id": body.survivor_id,
        "absorbed_id": body.absorbed_id,
    })))
}

// ---------------------------------------------------------------------------
// Knowledge Flow
// ---------------------------------------------------------------------------

/// GET /api/spaces/knowledge-flow — Get recent knowledge flow.
pub(crate) async fn handle_knowledge_flow(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let flows = state.kernel.spaces.knowledge_flow();
    Ok(Json(serde_json::json!({
        "items": flows,
        "total": flows.len(),
    })))
}

/// GET /api/spaces/:id/knowledge-flow — Knowledge flow for a specific Space.
pub(crate) async fn handle_knowledge_flow_for(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.kernel.spaces.knowledge_flow_for(&id) {
        Some(flows) => Ok(Json(serde_json::json!({
            "items": flows,
            "total": flows.len(),
        }))),
        None => Err(AppError::NotFound(format!(
            "Space {id} not found for knowledge flow"
        ))),
    }
}
