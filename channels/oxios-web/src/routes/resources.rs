use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::routes::{PageParams, paginate};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Gardens
// ---------------------------------------------------------------------------

/// Garden summary for listing.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct GardenSummary {
    /// Garden name.
    name: String,
    /// Image tag used.
    image_tag: String,
    /// Whether the garden is currently running.
    running: bool,
    /// Creation timestamp.
    created_at: String,
}

/// Request body for creating a garden.
#[derive(Debug, Deserialize)]
pub(crate) struct GardenCreateRequest {
    /// Name for the new garden.
    name: String,
}

/// Request body for executing a command in a garden.
#[derive(Debug, Deserialize)]
pub(crate) struct GardenExecRequest {
    /// Command to execute.
    command: Vec<String>,
    /// Working directory (optional, defaults to /workspace).
    #[serde(default)]
    workdir: Option<String>,
}

/// Response body for a garden exec command.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct GardenExecResponse {
    /// Standard output.
    stdout: String,
    /// Standard error.
    stderr: String,
    /// Exit code.
    exit_code: i32,
    /// Duration in milliseconds.
    duration_ms: u64,
}

/// GET /api/gardens — List gardens.
pub(crate) async fn handle_gardens_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let manager = state.container_manager.clone();
    match manager.list_containers().await {
        Ok(gardens) => {
            let summaries: Vec<GardenSummary> = gardens
                .into_iter()
                .map(|g| GardenSummary {
                    name: g.name,
                    image_tag: g.image_tag,
                    running: g.running,
                    created_at: g.created_at,
                })
                .collect();
            Ok(Json(paginate(&summaries, &params)))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list gardens");
            Err(AppError::Internal("failed to list gardens".into()))
        }
    }
}

/// POST /api/gardens — Create a new garden.
pub(crate) async fn handle_garden_create(
    state: State<Arc<AppState>>,
    Json(body): Json<GardenCreateRequest>,
) -> Result<Json<GardenSummary>, AppError> {
    let manager = state.container_manager.clone();
    match manager.new_container(&body.name).await {
        Ok(()) => {
            tracing::info!(garden = %body.name, "Garden created via API");
            Ok(Json(GardenSummary {
                name: body.name,
                image_tag: "oxios:latest".into(),
                running: false,
                created_at: chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %body.name, "Failed to create garden");
            Err(AppError::BadRequest(e.to_string()))
        }
    }
}

/// POST /api/gardens/:name/start — Start a garden container.
pub(crate) async fn handle_garden_start(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let manager = state.container_manager.clone();
    match manager.start_container(&name).await {
        Ok(()) => {
            tracing::info!(garden = %name, "Garden started via API");
            Ok(Json(serde_json::json!({"status": "started", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to start garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// POST /api/gardens/:name/stop — Stop a garden container.
pub(crate) async fn handle_garden_stop(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let manager = state.container_manager.clone();
    match manager.stop_container(&name).await {
        Ok(()) => {
            tracing::info!(garden = %name, "Garden stopped via API");
            Ok(Json(serde_json::json!({"status": "stopped", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to stop garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// DELETE /api/gardens/:name — Remove a garden.
pub(crate) async fn handle_garden_remove(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let manager = state.container_manager.clone();
    match manager.remove_container(&name).await {
        Ok(()) => {
            tracing::info!(garden = %name, "Garden removed via API");
            Ok(Json(serde_json::json!({"status": "removed", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to remove garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// POST /api/gardens/:name/exec — Execute a command in a garden.
pub(crate) async fn handle_garden_exec(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<GardenExecRequest>,
) -> Result<Json<GardenExecResponse>, (StatusCode, String)> {
    let manager = state.container_manager.clone();
    match manager
        .exec_in_container(&name, &body.command, body.workdir.as_deref())
        .await
    {
        Ok(result) => Ok(Json(GardenExecResponse {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            duration_ms: result.duration_ms,
        })),
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to exec in garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

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
    let programs = state.program_manager.list_programs().await;
    let summaries: Vec<ProgramSummary> = programs
        .into_iter()
        .map(|p| ProgramSummary {
            name: p.meta.name,
            version: p.meta.version,
            description: p.meta.description,
            author: p.meta.author,
            enabled: p.enabled,
            tools_count: p.meta.tools.len(),
            has_skill_content: !p.skill_content.is_empty(),
        })
        .collect();
    Json(paginate(&summaries, &params))
}

/// GET /api/programs/:name — Get program details.
pub(crate) async fn handle_program_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.program_manager.get_program(&name).await {
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

    match state.program_manager.install_from(source).await {
        Ok(program) => {
            tracing::info!(program = %program.meta.name, "Program installed via API");
            Ok(Json(serde_json::json!({
                "status": "installed",
                "name": program.meta.name,
                "version": program.meta.version,
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, path = %body.path, "Failed to install program");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
}

/// DELETE /api/programs/:name — Uninstall a program.
pub(crate) async fn handle_program_uninstall(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.uninstall(&name).await {
        Ok(()) => {
            tracing::info!(program = %name, "Program uninstalled via API");
            Ok(Json(serde_json::json!({"status": "uninstalled", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, program = %name, "Failed to uninstall program");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
}

/// POST /api/programs/:name/enable — Enable a program.
pub(crate) async fn handle_program_enable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.set_enabled(&name, true).await {
        Ok(()) => Ok(Json(serde_json::json!({"status": "enabled", "name": name}))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

/// POST /api/programs/:name/disable — Disable a program.
pub(crate) async fn handle_program_disable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.set_enabled(&name, false).await {
        Ok(()) => Ok(Json(serde_json::json!({"status": "disabled", "name": name}))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

/// GET /api/programs/:name/host-requirements — Check host requirements for a program.
pub(crate) async fn handle_program_host_requirements(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.check_host_requirements(&name).await {
        Ok(check) => serde_json::to_value(&check)
            .map(Json)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
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
    let status = state.host_tool_validator.full_check();
    Json(HostToolsStatusResponse {
        all_required_present: status.all_required_present,
        missing_required: status.missing_required,
        optional_available: status.optional_available,
    })
}
