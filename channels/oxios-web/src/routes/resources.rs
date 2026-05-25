use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::routes::{paginate, PageParams};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Deprecation helper
// ---------------------------------------------------------------------------

/// Deprecation response headers to include in legacy endpoints.
struct DeprecationHeaders {
    deprecation: HeaderName,
    deprecation_val: HeaderValue,
    link: HeaderName,
    link_val: HeaderValue,
}

impl DeprecationHeaders {
    fn new(successor: &str) -> Self {
        Self {
            deprecation: HeaderName::from_static("deprecation"),
            deprecation_val: HeaderValue::from_static("true"),
            link: HeaderName::from_static("link"),
            link_val: HeaderValue::from_str(
                &format!("<{}>; rel=\"successor-version\"", successor),
            )
            .unwrap(),
        }
    }

    /// Attach deprecation headers to an existing response.
    fn apply<B>(&self, mut resp: axum::http::Response<B>) -> axum::http::Response<B> {
        let headers = resp.headers_mut();
        headers.insert(self.deprecation.clone(), self.deprecation_val.clone());
        headers.insert(self.link.clone(), self.link_val.clone());
        resp
    }

    /// Build a JSON response with deprecation headers.
    fn json_response(&self, status: StatusCode, body: serde_json::Value) -> axum::response::Response {
        let resp = (status, Json(body)).into_response();
        self.apply(resp)
    }
}

// ---------------------------------------------------------------------------
// Programs (DEPRECATED — delegates to Skills system)
// ---------------------------------------------------------------------------

/// Program summary for listing (kept for backward compat in deprecated endpoint).
#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub(crate) struct ProgramSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    enabled: bool,
    tools_count: usize,
    has_skill_content: bool,
}

/// GET /api/programs — List all installed programs (DEPRECATED).
///
/// Returns skill data formatted for backward compatibility with deprecation headers.
pub(crate) async fn handle_programs_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> axum::response::Response {
    let entries = state.kernel.extensions.list_skills_entries().await;
    let skills: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.skill.name,
                "version": e.metadata.as_ref().and_then(|m| m.version.clone()).unwrap_or_default(),
                "description": e.skill.description,
                "author": e.metadata.as_ref().and_then(|m| m.author.clone()).unwrap_or_default(),
                "enabled": e.status != oxios_kernel::SkillStatus::Disabled,
                "tools_count": 0,
                "has_skill_content": !e.skill.content.is_empty(),
            })
        })
        .collect();
    let body = paginate(&skills, &params);
    let deprecation = DeprecationHeaders::new("/api/skills");
    deprecation.json_response(StatusCode::OK, body)
}

/// GET /api/programs/:name — Get program details (DEPRECATED).
///
/// Returns skill data with deprecation headers.
pub(crate) async fn handle_program_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    match state.kernel.extensions.get_skill_entry(&name).await {
        Some(entry) => {
            let body = serde_json::json!({
                "name": entry.skill.name,
                "version": entry.metadata.as_ref().and_then(|m| m.version.clone()).unwrap_or_default(),
                "description": entry.skill.description,
                "author": entry.metadata.as_ref().and_then(|m| m.author.clone()).unwrap_or_default(),
                "enabled": entry.status != oxios_kernel::SkillStatus::Disabled,
                "tools": [],
                "skill_content": entry.skill.content,
                "path": entry.skill.path.to_string_lossy(),
            });
            deprecation.json_response(StatusCode::OK, body)
        }
        None => {
            let resp = StatusCode::NOT_FOUND.into_response();
            deprecation.apply(resp)
        }
    }
}

/// Request body for program installation (DEPRECATED — no longer used).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ProgramInstallRequest {
    /// URL (git or tarball) to install from.
    path: String,
}

/// POST /api/programs — Install a program (DEPRECATED).
///
/// Returns 410 Gone with deprecation notice.
pub(crate) async fn handle_program_install(
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    deprecation.json_response(StatusCode::GONE, serde_json::json!({
        "error": "This endpoint is deprecated. Use /api/skills to manage skills.",
        "successor": "/api/skills",
    }))
}

/// DELETE /api/programs/:name — Uninstall a program (DEPRECATED).
///
/// Returns 410 Gone with deprecation notice.
pub(crate) async fn handle_program_uninstall(
    Path(_name): Path<String>,
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    deprecation.json_response(StatusCode::GONE, serde_json::json!({
        "error": "This endpoint is deprecated. Use DELETE /api/skills/:name to delete skills.",
        "successor": "/api/skills",
    }))
}

/// POST /api/programs/:name/enable — Enable a program (DEPRECATED).
///
/// Delegates to skill enable with deprecation headers.
pub(crate) async fn handle_program_enable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    match state.kernel.extensions.enable_skill(&name).await {
        Ok(()) => deprecation.json_response(
            StatusCode::OK,
            serde_json::json!({ "status": "enabled", "name": name }),
        ),
        Err(e) => deprecation.json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

/// POST /api/programs/:name/disable — Disable a program (DEPRECATED).
///
/// Delegates to skill disable with deprecation headers.
pub(crate) async fn handle_program_disable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    match state.kernel.extensions.disable_skill(&name).await {
        Ok(()) => deprecation.json_response(
            StatusCode::OK,
            serde_json::json!({ "status": "disabled", "name": name }),
        ),
        Err(e) => deprecation.json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

/// GET /api/programs/:name/host-requirements — Check host requirements (DEPRECATED).
///
/// Returns skill requirements with deprecation headers.
pub(crate) async fn handle_program_host_requirements(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    match state.kernel.extensions.check_skill_requirements(&name).await {
        Some(reqs) => {
            let body = serde_json::to_value(&reqs).unwrap_or_default();
            deprecation.json_response(StatusCode::OK, body)
        }
        None => deprecation.json_response(
            StatusCode::NOT_FOUND,
            serde_json::json!({ "error": "skill not found" }),
        ),
    }
}

// ---------------------------------------------------------------------------
// Host Tools (DEPRECATED — unified into Skills system)
// ---------------------------------------------------------------------------

/// Host tools status response (legacy, kept for type compat).
#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub(crate) struct HostToolsStatusResponse {
    all_required_present: bool,
    missing_required: Vec<String>,
    optional_available: std::collections::HashMap<String, bool>,
}

/// GET /api/host-tools — Check host tool availability (DEPRECATED).
///
/// Returns empty response with deprecation headers.
/// Host tools have been unified into the skills system (RFC-009).
pub(crate) async fn handle_host_tools_check(
) -> axum::response::Response {
    let deprecation = DeprecationHeaders::new("/api/skills");
    deprecation.json_response(StatusCode::OK, serde_json::json!({
        "tools": [],
        "deprecation_notice": "Host tools have been unified into the skills system. Use /api/skills instead.",
        "successor": "/api/skills",
    }))
}
