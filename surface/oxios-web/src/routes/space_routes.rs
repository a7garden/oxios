//! Project routes — REST API endpoints for Project management (RFC-011).
//!
//! Endpoints:
//! - GET  /api/projects           → List all projects
//! - POST /api/projects           → Create project
//! - GET  /api/projects/:id       → Get project details
//! - PUT  /api/projects/:id       → Update project
//! - DELETE /api/projects/:id     → Remove project
//! - GET  /api/projects/:id/memories → Get project-associated memories

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;

/// GET /api/projects — List all registered projects.
pub(crate) async fn handle_projects_list(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match &state.kernel.projects {
        Some(api) => {
            let projects = api.list_projects();
            Json(serde_json::json!({
                "items": projects,
                "total": projects.len(),
            }))
        }
        None => Json(serde_json::json!({
            "items": [],
            "total": 0,
            "error": "Project system not available (SQLite not enabled)"
        })),
    }
}

/// GET /api/projects/:id — Get project details.
pub(crate) async fn handle_project_get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    match &state.kernel.projects {
        Some(api) => match api.get_project(&id) {
            Some(info) => Json(serde_json::json!(info)),
            None => Json(serde_json::json!({"error": "Project not found"})),
        },
        None => Json(serde_json::json!({"error": "Project system not available"})),
    }
}

// Legacy Space route stubs — return empty/deprecated responses.
// These are kept temporarily for frontend compatibility during migration.

/// GET /api/spaces — Deprecated, returns empty.
#[allow(dead_code)]
pub(crate) async fn handle_spaces_list(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "items": [],
        "total": 0,
        "deprecated": "Use /api/projects instead"
    }))
}

/// GET /api/spaces/current — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_space_current(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"deprecated": "Use /api/projects instead"}))
}

/// GET /api/spaces/:id — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_space_get(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"error": "Deprecated, use /api/projects/:id"}))
}

/// POST /api/spaces/:id/activate — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_space_activate(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true, "project_id": id}))
}

/// POST /api/spaces/:id/archive — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_space_archive(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true, "project_id": id}))
}

/// POST /api/spaces/merge — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_space_merge(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true}))
}

/// POST /api/spaces/:id/restore — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_space_restore(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true, "project_id": id}))
}

/// GET /api/spaces/memory-flow — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_memory_flow(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"items": [], "deprecated": true}))
}

/// GET /api/spaces/:id/memory-flow — Deprecated.
#[allow(dead_code)]
pub(crate) async fn handle_memory_flow_for(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"items": [], "deprecated": true}))
}
