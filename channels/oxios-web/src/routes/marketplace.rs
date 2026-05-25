//! Marketplace API routes — ClawHub search, skills, install, updates.
//!
//! Route group: `/api/marketplace/*`

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use oxios_kernel::{MarketplaceApi, ClawHubSearchResult, ClawHubSkillDetail};

use crate::error::AppError;
use crate::server::AppState;

// ─── Query types ─────────────────────────────────────────────────────────────

/// Query params for marketplace search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string.
    pub q: String,
    /// Max results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// Request body for installing a skill.
#[derive(Debug, Deserialize)]
pub struct InstallBody {
    /// Specific version to install (None = latest).
    pub version: Option<String>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/marketplace/search — Search ClawHub for skills.
pub(crate) async fn handle_marketplace_search(
    state: State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<ClawHubSearchResult>>, AppError> {
    let results = state
        .kernel
        .marketplace_api()
        .search(&query.q, Some(query.limit))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(results))
}

/// GET /api/marketplace/skills/{slug} — Get skill detail from ClawHub.
pub(crate) async fn handle_marketplace_skill_detail(
    state: State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<ClawHubSkillDetail>, AppError> {
    let detail = state
        .kernel
        .marketplace_api()
        .get_skill(&slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(detail))
}

/// POST /api/marketplace/skills/{slug}/install — Install a skill from ClawHub.
pub(crate) async fn handle_marketplace_install(
    state: State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(body): Json<InstallBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = state
        .kernel
        .marketplace_api()
        .install(&slug, body.version.as_deref())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "ok": result.ok,
        "slug": result.slug,
        "version": result.version,
        "targetDir": result.target_dir.to_string_lossy(),
        "changelog": result.changelog,
    })))
}

/// GET /api/marketplace/updates — Check for updates to installed ClawHub skills.
pub(crate) async fn handle_marketplace_updates(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let updates = state
        .kernel
        .marketplace_api()
        .check_updates()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let results: Vec<_> = updates
        .into_iter()
        .map(|u| {
            serde_json::json!({
                "slug": u.slug,
                "currentVersion": u.current_version,
                "latestVersion": u.latest_version,
                "changelog": u.changelog,
            })
        })
        .collect();

    Ok(Json(results))
}

// ─── Router ───────────────────────────────────────────────────────────────────

/// Add marketplace routes to the given router.
pub fn marketplace_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/marketplace/search", get(handle_marketplace_search))
        .route(
            "/api/marketplace/skills/{slug}",
            get(handle_marketplace_skill_detail),
        )
        .route(
            "/api/marketplace/skills/{slug}/install",
            post(handle_marketplace_install),
        )
        .route("/api/marketplace/updates", get(handle_marketplace_updates))
}