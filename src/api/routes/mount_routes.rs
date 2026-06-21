//! Mount management API routes (RFC-025).
//!
//! Provides CRUD endpoints for Mounts (path aliases). Minimal input model:
//! create needs only `name` + `paths`.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;

use oxios_kernel::MountInfo;

use crate::api::error::AppError;
use crate::api::server::AppState;

// ─── Request / Query types ──────────────────────────────────

/// List query parameters with search.
#[derive(Debug, Deserialize)]
pub(crate) struct MountListParams {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 검색어 (name, auto_description, auto_meta keywords).
    pub search: Option<String>,
}

fn default_page() -> usize {
    1
}

fn default_limit() -> usize {
    50
}

/// Create request — minimal RFC-025 input (name + paths only).
#[derive(Debug, Deserialize)]
pub(crate) struct CreateMountRequest {
    pub name: String,
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Update request. Renaming is the only user-level mutation besides
/// enrichment (which is agent-driven via the `mount` tool).
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMountRequest {
    pub name: Option<String>,
}

// ─── Helpers ────────────────────────────────────────────────

/// Shorthand to get MountApi from AppState, or error.
macro_rules! mount_api {
    ($state:expr) => {
        $state
            .kernel
            .mounts
            .as_ref()
            .ok_or_else(|| AppError::Internal("Mounts not available".into()))?
    };
}

// ─── Handlers ───────────────────────────────────────────────

/// GET /api/mounts — List all Mounts with pagination and search.
pub(crate) async fn handle_mounts_list(
    state: State<Arc<AppState>>,
    Query(params): Query<MountListParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = mount_api!(state);
    let all = api.list_mounts();

    let filtered: Vec<MountInfo> = match &params.search {
        Some(search) => {
            let lower = search.to_lowercase();
            all.into_iter()
                .filter(|m| {
                    m.name.to_lowercase().contains(&lower)
                        || m.auto_description.to_lowercase().contains(&lower)
                        || m.auto_meta
                            .languages
                            .iter()
                            .any(|l| l.to_lowercase().contains(&lower))
                        || m.auto_meta
                            .stack
                            .iter()
                            .any(|s| s.to_lowercase().contains(&lower))
                        || m.auto_meta.summary.to_lowercase().contains(&lower)
                })
                .collect()
        }
        None => all,
    };

    let mut sorted = filtered;
    sorted.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));

    let total = sorted.len();
    let limit = params.limit.min(500);
    let offset = (params.page.saturating_sub(1)) * limit;
    let items: Vec<&MountInfo> = sorted.iter().skip(offset).take(limit).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": params.page,
        "limit": limit,
    })))
}

/// GET /api/mounts/:id — Get a single Mount.
pub(crate) async fn handle_mount_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<MountInfo>, AppError> {
    let api = mount_api!(state);
    api.get_mount(&id)
        .ok_or_else(|| AppError::NotFound("Mount not found".into()))
        .map(Json)
}

/// POST /api/mounts — Create a Mount (name + paths only).
pub(crate) async fn handle_mount_create(
    state: State<Arc<AppState>>,
    Json(body): Json<CreateMountRequest>,
) -> Result<(StatusCode, Json<MountInfo>), AppError> {
    let api = mount_api!(state);

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Mount name is required".into()));
    }
    if body.paths.is_empty() {
        return Err(AppError::BadRequest("At least one path is required".into()));
    }

    let mount = api
        .create_mount(body.name, body.paths)
        .map_err(|e| AppError::Internal(format!("Failed to create mount: {e}")))?;
    Ok((StatusCode::CREATED, Json(mount)))
}

/// PUT /api/mounts/:id — Update a Mount (rename).
pub(crate) async fn handle_mount_update(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateMountRequest>,
) -> Result<Json<MountInfo>, AppError> {
    let api = mount_api!(state);
    let name_opt = body.name;
    if let Some(ref name) = name_opt
        && name.trim().is_empty()
    {
        return Err(AppError::BadRequest("Mount name cannot be empty".into()));
    }
    if let Some(name) = name_opt {
        api.rename_mount(&id, name)
            .map_err(|e| AppError::Internal(format!("Failed to update mount: {e}")))
            .map(Json)
    } else {
        api.get_mount(&id)
            .ok_or_else(|| AppError::NotFound("Mount not found".into()))
            .map(Json)
    }
}

/// DELETE /api/mounts/:id — Remove a Mount.
pub(crate) async fn handle_mount_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let api = mount_api!(state);
    api.remove_mount(&id)
        .map_err(|e| AppError::Internal(format!("Failed to delete mount: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/mounts/:id/rescan — Re-seed auto_meta from the filesystem (RFC-025).
pub(crate) async fn handle_mount_rescan(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<MountInfo>, AppError> {
    let api = mount_api!(state);
    api.rescan(&id)
        .map_err(|e| AppError::Internal(format!("Failed to rescan: {e}")))
        .map(Json)
}
