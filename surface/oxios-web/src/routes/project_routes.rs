//! Project management API routes (RFC-011 Phase 4).
//!
//! Provides CRUD endpoints for Projects and Project-Memory linking.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use oxios_kernel::{memory::MemoryEntry, ProjectInfo};

use crate::error::AppError;
use crate::routes::paginate;
use crate::routes::PageParams;
use crate::server::AppState;

// ─── Request / Query types ──────────────────────────────────

/// List query parameters with search.
#[derive(Debug, Deserialize)]
pub(crate) struct ProjectListParams {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 검색어 (name, description, tags에서 검색).
    pub search: Option<String>,
}

fn default_page() -> usize {
    1
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub emoji: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub memory_visible: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProjectRequest {
    pub name: Option<String>,
    pub paths: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub emoji: Option<String>,
    pub description: Option<String>,
    pub memory_visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LinkMemoryRequest {
    pub memory_id: String,
}

// ─── Helpers ────────────────────────────────────────────────

/// Shorthand to get ProjectApi from AppState, or error.
macro_rules! project_api {
    ($state:expr) => {
        $state.kernel.projects.as_ref()
            .ok_or_else(|| AppError::Internal("Projects not available".into()))?
    };
}

// ─── Handlers ───────────────────────────────────────────────

/// GET /api/projects — List all projects with pagination and search.
pub(crate) async fn handle_projects_list(
    state: State<Arc<AppState>>,
    Query(params): Query<ProjectListParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = project_api!(state);
    let all = api.list_projects();

    // Filter by search
    let filtered: Vec<ProjectInfo> = match &params.search {
        Some(search) => {
            let lower = search.to_lowercase();
            all.into_iter()
                .filter(|p| {
                    p.name.to_lowercase().contains(&lower)
                        || p.description.to_lowercase().contains(&lower)
                        || p.tags.iter().any(|t| t.to_lowercase().contains(&lower))
                })
                .collect()
        }
        None => all,
    };

    // Sort by last_active_at descending
    let mut sorted = filtered;
    sorted.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));

    let total = sorted.len();
    let limit = params.limit.min(500);
    let offset = (params.page.saturating_sub(1)) * limit;
    let items: Vec<&ProjectInfo> = sorted.iter().skip(offset).take(limit).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": params.page,
        "limit": limit,
    })))
}

/// GET /api/projects/:id — Get a single project.
pub(crate) async fn handle_project_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ProjectInfo>, AppError> {
    let api = project_api!(state);
    api.get_project(&id)
        .ok_or_else(|| AppError::NotFound("Project not found".into()))
        .map(Json)
}

/// POST /api/projects — Create a new project.
pub(crate) async fn handle_project_create(
    state: State<Arc<AppState>>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectInfo>), AppError> {
    let api = project_api!(state);

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Project name is required".into()));
    }

    let project = api
        .create_project(
            body.name,
            body.paths,
            body.tags,
            body.emoji,
            body.description,
        )
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(project)))
}

/// PUT /api/projects/:id — Update a project.
pub(crate) async fn handle_project_update(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectInfo>, AppError> {
    let api = project_api!(state);

    let project = api
        .update_project(
            &id,
            body.name,
            body.paths,
            body.tags,
            body.emoji,
            body.description,
            body.memory_visible,
        )
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(Json(project))
}

/// DELETE /api/projects/:id — Remove a project.
pub(crate) async fn handle_project_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let api = project_api!(state);

    api.remove_project(&id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/projects/:id/memories — Get memories linked to a project.
pub(crate) async fn handle_project_memories(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<PageParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = project_api!(state);

    // 1. Get memory IDs for this project
    let memory_ids = api
        .get_project_memory_ids(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // 2. Load MemoryEntry for each ID from StateApi (same pattern as workspace.rs)
    let mut memories = Vec::new();
    for mid in &memory_ids {
        for category in [
            "memory/facts",
            "memory/episodes",
            "memory/knowledge",
            "memory/sessions",
        ] {
            if let Ok(Some(entry)) = state
                .kernel
                .state
                .load::<MemoryEntry>(category, mid)
                .await
            {
                memories.push(serde_json::json!({
                    "id": entry.id,
                    "content": entry.content,
                    "memory_type": entry.memory_type.label(),
                    "importance": entry.importance,
                    "tier": format!("{:?}", entry.tier).to_lowercase(),
                    "tags": entry.tags,
                    "created_at": entry.created_at.to_rfc3339(),
                }));
                break; // Found in this category, move to next memory_id
            }
        }
    }

    Ok(Json(paginate(&memories, &params)))
}

/// POST /api/projects/:id/memories — Link a memory to a project.
pub(crate) async fn handle_project_link_memory(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<LinkMemoryRequest>,
) -> Result<StatusCode, AppError> {
    let api = project_api!(state);

    api.link_memory(&id, &body.memory_id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/projects/:id/memories/:memoryId — Unlink a memory from a project.
pub(crate) async fn handle_project_unlink_memory(
    state: State<Arc<AppState>>,
    Path((project_id, memory_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let api = project_api!(state);

    api.unlink_memory(&project_id, &memory_id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}