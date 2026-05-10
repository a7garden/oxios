use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::routes::{PageParams, paginate};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Programs (OS-level installable applications)
// ---------------------------------------------------------------------------

/// Program summary for listing.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct ProgramSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    enabled: bool,
    tools_count: usize,
    has_skill_content: bool,
}

/// GET /api/programs — List all installed programs.
pub(crate) async fn handle_programs_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    let programs = state.kernel.list_programs().await;
    let summaries: Vec<ProgramSummary> = programs
        .into_iter()
        .map(|p| ProgramSummary {
            name: p.name,
            version: p.version,
            description: p.description,
            author: p.author,
            enabled: false, // ProgramMeta doesn't have enabled; check original
            tools_count: p.tools.len(),
            has_skill_content: false, // ProgramMeta doesn't have skill_content
        })
        .collect();
    Json(paginate(&summaries, &params))
}

/// GET /api/programs/:name — Get program details.
pub(crate) async fn handle_program_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.kernel.get_program(&name).await {
        Some(program) => Ok(Json(serde_json::json!({
            "name": program.meta.name,
            "version": program.meta.version,
            "description": program.meta.description,
            "author": program.meta.author,
            "enabled": program.enabled,
            "tools": program.meta.tools,
            "skill_content": program.skill_content,
            "path": program.path.to_string_lossy(),
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Request body for program installation.
#[derive(Debug, Deserialize)]
pub(crate) struct ProgramInstallRequest {
    /// URL (git or tarball) to install from.
    path: String,
}

/// POST /api/programs — Install a program from a URL.
pub(crate) async fn handle_program_install(
    state: State<Arc<AppState>>,
    Json(body): Json<ProgramInstallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Validate install source URL length (proxy for sanity check)
    const MAX_SOURCE_LENGTH: usize = 8192;
    if body.path.len() > MAX_SOURCE_LENGTH {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, format!(
            "Source URL too long: {} bytes exceeds limit of {} bytes",
            body.path.len(),
            MAX_SOURCE_LENGTH,
        )));
    }

    use oxios_kernel::InstallSource;

    // Only allow remote sources via API (no local path traversal)
    let source = if body.path.ends_with(".git") || body.path.starts_with("git@") {
        InstallSource::Git { url: body.path.clone(), branch: None }
    } else if body.path.starts_with("http://") || body.path.starts_with("https://") {
        InstallSource::Tarball { url: body.path.clone() }
    } else {
        return Err((StatusCode::BAD_REQUEST, "Local path installation not allowed via API. Use git URL or tarball URL.".into()));
    };

    state.kernel.install_program(source).await
        .map(|program| {
            tracing::info!(program = %program.meta.name, "Program installed via API");
            Json(serde_json::json!({
                "status": "installed",
                "name": program.meta.name,
                "version": program.meta.version,
            }))
        })
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

/// DELETE /api/programs/:name — Uninstall a program.
pub(crate) async fn handle_program_uninstall(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.kernel.uninstall_program(&name).await
        .map(|_| {
            tracing::info!(program = %name, "Program uninstalled via API");
            Json(serde_json::json!({"status": "uninstalled", "name": name}))
        })
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

/// POST /api/programs/:name/enable — Enable a program.
pub(crate) async fn handle_program_enable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.kernel.enable_program(&name).await
        .map(|_| Json(serde_json::json!({"status": "enabled", "name": name})))
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

/// POST /api/programs/:name/disable — Disable a program.
pub(crate) async fn handle_program_disable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.kernel.disable_program(&name).await
        .map(|_| Json(serde_json::json!({"status": "disabled", "name": name})))
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

/// GET /api/programs/:name/host-requirements — Check host requirements for a program.
pub(crate) async fn handle_program_host_requirements(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.kernel.check_program_host_requirements(&name).await
        .map(|check| serde_json::to_value(&check).map(Json))
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ---------------------------------------------------------------------------
// Host Tools
// ---------------------------------------------------------------------------

/// Host tools status response.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct HostToolsStatusResponse {
    all_required_present: bool,
    missing_required: Vec<String>,
    optional_available: std::collections::HashMap<String, bool>,
}

/// GET /api/host-tools — Check host tool availability.
pub(crate) async fn handle_host_tools_check(
    state: State<Arc<AppState>>,
) -> Json<HostToolsStatusResponse> {
    let status = state.kernel.check_host_tools();
    Json(HostToolsStatusResponse {
        all_required_present: status.all_required_present,
        missing_required: status.missing_required,
        optional_available: status.optional_available,
    })
}