//! Git version control API routes.

use crate::api::error::AppError;
use crate::api::server::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json;
use std::sync::Arc;

/// GET /api/git/log — Return commit log entries.
pub(crate) async fn handle_git_log(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let log = state
        .kernel
        .infra
        .git_log(100)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "entries": log })))
}

/// GET /api/git/tags — List all tags.
pub(crate) async fn handle_git_tags(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tags = state
        .kernel
        .infra
        .git_tags()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "tags": tags })))
}

/// POST /api/git/verify — Verify repository integrity.
pub(crate) async fn handle_git_verify(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let valid = state
        .kernel
        .infra
        .git_verify()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "valid": valid })))
}

/// Request body for file restore.
#[derive(Debug, Deserialize)]
pub struct RestoreRequest {
    /// Commit hash to restore from.
    pub hash: String,
    /// Relative path to restore.
    pub path: String,
}

/// POST /api/git/restore — Restore a file to its state in a specific commit.
pub(crate) async fn handle_git_restore(
    state: State<Arc<AppState>>,
    Json(body): Json<RestoreRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .infra
        .git_restore(&body.path, &body.hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "restored": body.path,
        "from": body.hash
    })))
}

/// DELETE /api/git/tags/{name} — Delete a tag.
pub(crate) async fn handle_git_tag_delete(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .infra
        .git_delete_tag(&name)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "deleted": name })))
}
