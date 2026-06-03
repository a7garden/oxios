use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::routes::{paginate, PageParams};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// GET /health — Liveness check (no auth required).
///
/// Always returns 200 OK if the process is alive.
pub(crate) async fn handle_health(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /health/ready — Readiness check (no auth required).
///
/// Checks subsystem health: state store, git repository.
/// Returns 200 if healthy, 503 if degraded.
pub(crate) async fn handle_readiness(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut components = serde_json::Map::new();
    let mut all_healthy = true;

    // State store: check workspace path exists
    let ws_path = state.kernel.state.workspace_path();
    let state_ok = ws_path.exists();
    components.insert(
        "state_store".into(),
        serde_json::json!({"healthy": state_ok}),
    );
    all_healthy &= state_ok;

    // Git: verify repository integrity
    let git_ok = state.kernel.infra.git_verify().unwrap_or(false);
    components.insert("git".into(), serde_json::json!({"healthy": git_ok}));
    // Git failure is degraded, not fatal

    // Memory: always healthy (optional subsystem)
    let (index_size, total) = state.kernel.agents.memory_stats().await;
    components.insert(
        "memory".into(),
        serde_json::json!({"healthy": true, "index_size": index_size, "total_entries": total}),
    );

    let status = if all_healthy { "healthy" } else { "degraded" };
    let body = serde_json::json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "components": components,
    });

    if all_healthy {
        Ok(Json(body))
    } else {
        Err((axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(body)))
    }
}

// ---------------------------------------------------------------------------
// Control
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Component Health Types
// ---------------------------------------------------------------------------

/// Health status of an individual component.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct ComponentStatus {
    /// Whether the component is healthy.
    pub healthy: bool,
    /// Optional detail message.
    pub detail: Option<String>,
}

/// Memory subsystem health.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct MemoryHealth {
    /// Whether memory is enabled.
    pub enabled: bool,
    /// Number of entries in the vector index.
    pub index_size: usize,
    /// Total entries across all memory types.
    pub total_entries: usize,
}

/// Agent subsystem health.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct AgentHealth {
    /// Number of currently active agents.
    pub active_count: usize,
    /// Total agents forked (lifetime).
    pub total_forked: u64,
    /// Total agents completed (lifetime).
    pub total_completed: u64,
    /// Total agents failed (lifetime).
    pub total_failed: u64,
}

/// Aggregate health of all system components.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct ComponentHealth {
    /// State store health.
    pub state_store: ComponentStatus,
    /// Event bus health.
    pub event_bus: ComponentStatus,
    /// Memory subsystem health.
    pub memory: MemoryHealth,
    /// Agent subsystem health.
    pub agents: AgentHealth,
    /// Active spaces count.
    pub spaces_active: usize,
}

/// Response body for the status endpoint.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct StatusResponse {
    /// Service name.
    service: String,
    /// Current status.
    status: String,
    /// API version.
    version: String,
    /// Registered channels.
    channels: Vec<String>,
    /// Uptime info.
    uptime: String,
    /// Component-level health details.
    components: Option<ComponentHealth>,
}

/// GET /api/status — System status with component health.
pub(crate) async fn handle_status(state: State<Arc<AppState>>) -> Json<StatusResponse> {
    let uptime = state.start_time.elapsed();
    let uptime_str = format!(
        "{}h {}m {}s",
        uptime.as_secs() / 3600,
        (uptime.as_secs() % 3600) / 60,
        uptime.as_secs() % 60
    );

    // State store health — check that the base path exists
    let state_store_healthy = state.kernel.state.workspace_path().exists();

    // Event bus — always healthy if we got this far
    let event_bus_healthy = true;

    // Memory health
    let (mem_index_size, mem_total) = state.kernel.agents.memory_stats().await;
    let memory_health = MemoryHealth {
        enabled: true,
        index_size: mem_index_size,
        total_entries: mem_total,
    };

    // Agent health — count active from supervisor, metrics from export
    let active_count = state
        .kernel
        .agents
        .list()
        .await
        .map(|agents| {
            agents
                .iter()
                .filter(|a| {
                    matches!(
                        a.status,
                        oxios_kernel::AgentStatus::Running
                            | oxios_kernel::AgentStatus::Starting
                            | oxios_kernel::AgentStatus::Idle
                    )
                })
                .count()
        })
        .unwrap_or(0);

    let (total_forked, total_completed, total_failed) = parse_agent_metrics();

    let agent_health = AgentHealth {
        active_count,
        total_forked,
        total_completed,
        total_failed,
    };

    let components = Some(ComponentHealth {
        state_store: ComponentStatus {
            healthy: state_store_healthy,
            detail: if state_store_healthy {
                None
            } else {
                Some("base path not found".to_string())
            },
        },
        event_bus: ComponentStatus {
            healthy: event_bus_healthy,
            detail: None,
        },
        memory: memory_health,
        agents: agent_health,
        spaces_active: state
            .kernel
            .projects
            .as_ref()
            .map(|p| p.list_projects().len())
            .unwrap_or(0),
    });

    Json(StatusResponse {
        service: "oxios".into(),
        status: "running".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        channels: vec!["web".into()],
        uptime: uptime_str,
        components,
    })
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

/// Query params for update check.
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateCheckParams {
    /// Check a specific version instead of latest.
    #[serde(default)]
    pub version: Option<String>,
}

/// Response for `GET /api/update/check`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct UpdateCheckResponse {
    /// Currently running version.
    pub current_version: String,
    /// Latest available version on GitHub.
    pub latest_version: String,
    /// Whether an update is available.
    pub update_available: bool,
    /// Release tag name (e.g. "v1.0.0").
    pub tag_name: String,
    /// URL to the release page.
    pub html_url: String,
    /// Short body / release notes excerpt.
    pub release_notes: String,
    /// Publication date.
    pub published_at: String,
    /// Available download assets.
    pub assets: Vec<AssetInfo>,
}

/// Info about a release asset.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct AssetInfo {
    pub name: String,
    pub size: u64,
    pub download_url: String,
}

/// Body for `POST /api/update/run`.
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateRunBody {
    /// Update binary (default: true).
    #[serde(default = "default_true")]
    pub binary: bool,
    /// Update web UI (default: true).
    #[serde(default = "default_true")]
    pub web: bool,
    /// Target version (default: latest).
    pub version: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Response for `POST /api/update/run`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct UpdateRunResponse {
    /// Whether the update succeeded.
    pub success: bool,
    /// Version we updated to.
    pub updated_to: String,
    /// What was updated.
    pub binary_updated: bool,
    pub web_updated: bool,
    /// Human-readable message.
    pub message: String,
}

/// Response for `GET /api/update/changelog`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct ChangelogResponse {
    pub tag_name: String,
    pub version: String,
    pub published_at: String,
    pub body: String,
    pub html_url: String,
}

/// GET /api/update/check — Check for available updates from GitHub Releases.
pub(crate) async fn handle_update_check(
    Query(params): Query<UpdateCheckParams>,
) -> Result<Json<UpdateCheckResponse>, AppError> {
    let current = env!("CARGO_PKG_VERSION");

    let release = fetch_github_release(params.version.as_deref()).await?;

    let tag_name = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let latest_version = tag_name.trim_start_matches('v').to_string();
    let html_url = release["html_url"].as_str().unwrap_or("").to_string();
    let body_text = release["body"]
        .as_str()
        .unwrap_or("No release notes.")
        .to_string();
    let published_at = release["published_at"].as_str().unwrap_or("").to_string();

    let assets: Vec<AssetInfo> = release["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|a| {
            Some(AssetInfo {
                name: a["name"].as_str()?.to_string(),
                size: a["size"].as_u64()?,
                download_url: a["browser_download_url"].as_str()?.to_string(),
            })
        })
        .collect();

    Ok(Json(UpdateCheckResponse {
        current_version: current.to_string(),
        latest_version: latest_version.clone(),
        update_available: latest_version != current,
        tag_name,
        html_url,
        release_notes: body_text,
        published_at,
        assets,
    }))
}

/// POST /api/update/run — Execute the update (download + install binary/web).
pub(crate) async fn handle_update_run(
    Json(body): Json<UpdateRunBody>,
) -> Result<Json<UpdateRunResponse>, AppError> {
    let current = env!("CARGO_PKG_VERSION");

    let release = fetch_github_release(body.version.as_deref()).await?;

    let tag_name = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let target_version = tag_name.trim_start_matches('v').to_string();

    let assets: Vec<(String, String, u64)> = release["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|a| {
            Some((
                a["name"].as_str()?.to_string(),
                a["browser_download_url"].as_str()?.to_string(),
                a["size"].as_u64()?,
            ))
        })
        .collect();

    let client = reqwest::Client::builder()
        .user_agent(format!("oxios/{current}"))
        .build()
        .map_err(|e| AppError::Internal(format!("failed to create HTTP client: {e}")))?;

    let mut binary_updated = false;
    let mut web_updated = false;
    let mut messages: Vec<String> = Vec::new();

    // Update web UI
    if body.web {
        if let Some((name, url, size)) = assets.iter().find(|(n, _, _)| n == "web-dist.zip") {
            tracing::info!(name, size, "Downloading web UI for update");
            let bytes = download_bytes(&client, url).await?;

            let dest_dir = dirs::home_dir()
                .ok_or_else(|| AppError::Internal("cannot determine home directory".into()))?
                .join(".oxios")
                .join("web")
                .join("dist");
            std::fs::create_dir_all(&dest_dir)
                .map_err(|e| AppError::Internal(format!("failed to create web dir: {e}")))?;

            let cursor = std::io::Cursor::new(&bytes);
            let mut archive = zip::ZipArchive::new(cursor)
                .map_err(|e| AppError::Internal(format!("invalid zip: {e}")))?;

            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| AppError::Internal(format!("zip read error: {e}")))?;
                let out_path = dest_dir.join(file.name());
                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&out_path).ok();
                } else {
                    if let Some(p) = out_path.parent() {
                        std::fs::create_dir_all(p).ok();
                    }
                    let mut out_file = std::fs::File::create(&out_path)
                        .map_err(|e| AppError::Internal(format!("write error: {e}")))?;
                    std::io::copy(&mut file, &mut out_file)
                        .map_err(|e| AppError::Internal(format!("write error: {e}")))?;
                }
            }
            web_updated = true;
            messages.push(format!("Web UI updated to {target_version}"));
        } else {
            messages.push("web-dist.zip not found in release, skipped".to_string());
        }
    }

    // Update binary via cargo
    if body.binary {
        let mut args = vec!["install", "oxios", "locked"];
        if let Some(ref v) = body.version {
            args.push("--version");
            args.push(v.as_str());
        }

        tracing::info!(?args, "Running cargo install for binary update");

        let output = tokio::process::Command::new("cargo")
            .args(&args)
            .output()
            .await
            .map_err(|e| AppError::Internal(format!("failed to run cargo: {e}")))?;

        if output.status.success() {
            binary_updated = true;
            messages.push(format!("Binary updated to {target_version} via cargo"));
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(%stderr, "cargo install failed");
            messages.push(format!(
                "Binary update failed: {}",
                stderr.lines().take(3).collect::<Vec<_>>().join("; ")
            ));
        }
    }

    tracing::info!(
        binary_updated,
        web_updated,
        target_version,
        "Update completed"
    );

    Ok(Json(UpdateRunResponse {
        success: true,
        updated_to: target_version,
        binary_updated,
        web_updated,
        message: messages.join("; "),
    }))
}

/// GET /api/update/changelog — Show release notes for a version.
pub(crate) async fn handle_update_changelog(
    Query(params): Query<UpdateCheckParams>,
) -> Result<Json<ChangelogResponse>, AppError> {
    let release = fetch_github_release(params.version.as_deref()).await?;

    let tag_name = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let version = tag_name.trim_start_matches('v').to_string();
    let published_at = release["published_at"].as_str().unwrap_or("").to_string();
    let body = release["body"]
        .as_str()
        .unwrap_or("No release notes.")
        .to_string();
    let html_url = release["html_url"].as_str().unwrap_or("").to_string();

    Ok(Json(ChangelogResponse {
        tag_name,
        version,
        published_at,
        body,
        html_url,
    }))
}

// ---------------------------------------------------------------------------
// Update helpers
// ---------------------------------------------------------------------------

const GITHUB_OWNER: &str = "a7garden";
const GITHUB_REPO: &str = "oxios";

async fn fetch_github_release(version: Option<&str>) -> Result<serde_json::Value, AppError> {
    let api_url = match version {
        Some(v) => {
            format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/tags/v{v}")
        }
        None => {
            format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest")
        }
    };

    let client = reqwest::Client::builder()
        .user_agent(format!("oxios/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client error: {e}")))?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub API request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "GitHub API error {status}: {body}"
        )));
    }

    resp.json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse GitHub response: {e}")))
}

async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, AppError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Download request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::Internal(format!("Download failed: {status}")));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| AppError::Internal(format!("Failed to read download body: {e}")))
}

/// Parse agent metrics from the Prometheus export text.
/// Returns (forked, completed, failed) counters.
fn parse_agent_metrics() -> (u64, u64, u64) {
    let export = oxios_kernel::metrics::registry().export();
    let mut forked = 0u64;
    let mut completed = 0u64;
    let mut failed = 0u64;
    for line in export.lines() {
        if line.starts_with("oxios_agents_forked_total ") {
            forked = line
                .rsplit(' ')
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("oxios_agents_completed_total ") {
            completed = line
                .rsplit(' ')
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("oxios_agents_failed_total ") {
            failed = line
                .rsplit(' ')
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    (forked, completed, failed)
}

/// Agent summary for listing.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct AgentSummary {
    /// Agent unique ID.
    id: String,
    /// Agent name/goal.
    name: String,
    /// Current status.
    status: String,
    /// Creation timestamp.
    created_at: String,
    /// Seed ID if applicable.
    seed_id: Option<String>,
}

/// GET /api/agents — List agent instances.
pub(crate) async fn handle_agents_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    match state.kernel.agents.list().await {
        Ok(agents) => {
            let items: Vec<AgentSummary> = agents
                .into_iter()
                .map(|a| AgentSummary {
                    id: a.id.to_string(),
                    name: a.name,
                    status: a.status.to_string(),
                    created_at: a.created_at.to_rfc3339(),
                    seed_id: a.seed_id.map(|s| s.to_string()),
                })
                .collect();
            Json(paginate(&items, &params))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list agents");
            Json(paginate(&Vec::<AgentSummary>::new(), &params))
        }
    }
}

/// GET /api/agents/{id} — Agent detail.
#[allow(dead_code)]
pub(crate) async fn handle_agent_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state
        .kernel
        .agents
        .list()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let agent = agents
        .into_iter()
        .find(|a| a.id.to_string() == id)
        .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

    let budget = state.kernel.agents.check_budget(&agent.id);

    Ok(Json(serde_json::json!({
        "id": agent.id.to_string(),
        "name": agent.name,
        "status": agent.status.to_string(),
        "created_at": agent.created_at.to_rfc3339(),
        "seed_id": agent.seed_id.map(|s| s.to_string()),
        "steps_completed": 0,
        "budget": {
            "tokens_remaining": budget.tokens_remaining,
            "calls_remaining": budget.calls_remaining,
            "window_remaining_secs": budget.window_remaining_secs,
            "is_exhausted": budget.is_exhausted,
        },
    })))
}

/// GET /api/agents/{id}/trace — Agent execution trace.
#[allow(dead_code)]
pub(crate) async fn handle_agent_trace(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify the agent exists
    let agents = state
        .kernel
        .agents
        .list()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let _agent = agents
        .into_iter()
        .find(|a| a.id.to_string() == id)
        .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

    // Try to load trace from sessions/{session_id}/trace.json
    // For now, return empty trace
    Ok(Json(serde_json::json!({
        "agent_id": id,
        "steps": [],
        "completed_at": null,
    })))
}

/// GET /api/agents/{id}/logs — Agent execution logs.
#[allow(dead_code)]
pub(crate) async fn handle_agent_logs(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify the agent exists
    let agents = state
        .kernel
        .agents
        .list()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let _agent = agents
        .into_iter()
        .find(|a| a.id.to_string() == id)
        .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

    Ok(Json(serde_json::json!({
        "agent_id": id,
        "entries": [],
    })))
}

/// POST /api/agents/{id}/kill — Kill an agent.
pub(crate) async fn handle_agent_kill(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    tracing::info!(agent_id = %id, "Kill agent requested");
    state.kernel.agents.kill(&id).await.map_err(|e| {
        tracing::warn!(error = %e, "Agent not found");
        AppError::NotFound("agent not found".into())
    })
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// GET /api/config — Get current configuration.
pub(crate) async fn handle_config_get(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Serialize the actual config from AppState (read lock).
    let config = state.config.read();
    match serde_json::to_value(&*config) {
        Ok(mut json) => {
            // Mask API key in response — never expose plaintext
            if let Some(engine) = json.get_mut("engine") {
                if let Some(api_key) = engine.get_mut("api_key") {
                    if api_key.as_str().is_some_and(|k| !k.is_empty()) {
                        *api_key = serde_json::Value::String("***".to_string());
                    }
                }
            }
            // Add api_key_set flag so the frontend knows if a key is currently set
            json["engine"]["api_key_set"] =
                serde_json::Value::Bool(config.engine.api_key.is_some());
            Ok(Json(json))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize config");
            Err(AppError::Internal("failed to serialize config".into()))
        }
    }
}

/// Deep-merge a patch into a base `serde_json::Value` (both must be objects).
///
/// Sections and fields present in `patch` override those in `base`.
/// Sections and fields absent from `patch` are preserved from `base`.
/// This implements PATCH semantics so that a partial config update does not
/// reset fields the caller did not intend to change.
///
/// Conflict policy:
/// - If both sides have a non-null object at the same path, recurse.
/// - Otherwise `patch` wins.
fn deep_merge_json(base: &mut serde_json::Value, patch: serde_json::Value) {
    use serde_json::Value;
    if let Value::Object(patch_map) = patch {
        if !base.is_object() {
            *base = Value::Object(serde_json::Map::new());
        }
        let base_map = base.as_object_mut().expect("just ensured object");
        for (key, patch_val) in patch_map {
            match base_map.get_mut(&key) {
                Some(existing) if existing.is_object() && patch_val.is_object() => {
                    deep_merge_json(existing, patch_val);
                }
                _ => {
                    base_map.insert(key, patch_val);
                }
            }
        }
    }
}

/// PUT /api/config — Update configuration.
///
/// PATCH semantics: the request body is deep-merged into the current
/// in-memory config so that sections and fields the caller omits are
/// preserved rather than reset to defaults. The merged result is
/// validated against the `OxiosConfig` schema, persisted to disk, and
/// hot-reloaded in memory.
pub(crate) async fn handle_config_put(
    state: State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    tracing::info!("Config update requested");

    // Deep-merge the patch into the current config so omitted fields are preserved.
    let mut current_value = {
        let cfg = state.config.read();
        serde_json::to_value(&*cfg).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize current config");
            AppError::Internal("failed to serialize current config".into())
        })?
    };
    deep_merge_json(&mut current_value, body.clone());

    // Validate the merged result by parsing as OxiosConfig.
    let updated: oxios_kernel::OxiosConfig = match serde_json::from_value(current_value.clone()) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid config shape");
            return Err(AppError::BadRequest(format!("Invalid config: {e}")));
        }
    };

    // Run the kernel validator too (catches semantic errors like
    // default_timeout > max_timeout) before we touch disk.
    let (errors, warnings) = updated.validate();
    for w in &warnings {
        tracing::warn!(config_warning = %w, "Config validation warning");
    }
    if !errors.is_empty() {
        let msg = errors.join("; ");
        tracing::warn!(error = %msg, "Config validation failed");
        return Err(AppError::BadRequest(format!("Invalid config: {msg}")));
    }

    // Persist the merged config to disk.
    let content = toml::to_string_pretty(&updated)
        .map_err(|e: toml::ser::Error| AppError::Internal(e.to_string()))?;
    if let Err(e) = tokio::fs::write(&state.config_path, content).await {
        tracing::error!(error = %e, "Failed to persist config");
        return Err(AppError::Internal(e.to_string()));
    }
    tracing::info!(path = %state.config_path.display(), "Config persisted");

    // Hot-reload: update in-memory config.
    let updated_config = updated;
    *state.config.write() = updated_config.clone();

    // Propagate hot-reloadable config to kernel subsystems.
    // Each subsystem gets its relevant slice of the config.

    // ExecApi — allowlist, shell mode, timeouts
    *state.kernel.exec.shared_config().write() = updated_config.exec.clone();

    // AgentScheduler — concurrency, rate limit, zombie timeout
    state.kernel.infra.scheduler().update_config(
        updated_config.scheduler.max_concurrent,
        updated_config.scheduler.rate_limit_per_minute,
        updated_config.scheduler.zombie_timeout_secs,
    );

    // ResourceMonitor — CPU/memory/load thresholds
    use oxios_kernel::resource_monitor::OverloadThreshold;
    state.kernel.infra.resource_monitor().set_overload_threshold(OverloadThreshold {
        cpu_percent: updated_config.resource_monitor.cpu_threshold,
        memory_percent: updated_config.resource_monitor.memory_threshold,
        load_avg: updated_config.resource_monitor.load_threshold,
    });

    tracing::info!(
        "Config hot-reloaded (web + kernel subsystems) from {}",
        state.config_path.display()
    );
    Ok(Json(body))
}

#[cfg(test)]
mod deep_merge_tests {
    use super::deep_merge_json;
    use serde_json::json;

    #[test]
    fn preserves_omitted_top_level_sections() {
        let mut base = json!({
            "kernel": {"workspace": "~/.oxios/workspace", "max_agents": 10},
            "exec": {"allowed_commands": ["ls", "cat"], "allowlist_mode": "enforced"},
        });
        let patch = json!({
            "kernel": {"max_agents": 20},
        });
        deep_merge_json(&mut base, patch);
        assert_eq!(base["kernel"]["workspace"], "~/.oxios/workspace");
        assert_eq!(base["kernel"]["max_agents"], 20);
        assert_eq!(base["exec"]["allowed_commands"][0], "ls");
        assert_eq!(base["exec"]["allowlist_mode"], "enforced");
    }

    #[test]
    fn patch_value_replaces_scalar() {
        let mut base = json!({"engine": {"default_model": "old/model"}});
        deep_merge_json(
            &mut base,
            json!({"engine": {"default_model": "new/model"}}),
        );
        assert_eq!(base["engine"]["default_model"], "new/model");
    }

    #[test]
    fn patch_object_replaces_object() {
        let mut base = json!({"security": {"auth_enabled": false, "cors_origins": ["http://a"]}});
        deep_merge_json(&mut base, json!({"security": {"auth_enabled": true}}));
        assert_eq!(base["security"]["auth_enabled"], true);
        assert_eq!(base["security"]["cors_origins"][0], "http://a");
    }

    #[test]
    fn empty_patch_is_noop() {
        let mut base = json!({"exec": {"allowed_commands": ["ls"]}});
        let original = base.clone();
        deep_merge_json(&mut base, json!({}));
        assert_eq!(base, original);
    }
}

// ---------------------------------------------------------------------------
// System Tools (Doctor, Audit Verify, Backup, Log)
// ---------------------------------------------------------------------------

/// A single diagnostic check result.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct DoctorCheck {
    /// Check name.
    pub name: String,
    /// Status: pass, warn, fail.
    pub status: String,
    /// Human-readable detail.
    pub message: String,
}

/// Response for `POST /api/system/doctor`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct DoctorResponse {
    /// Total checks performed.
    pub checks: u32,
    /// Number of issues found.
    pub issues: u32,
    /// Per-check results.
    pub results: Vec<DoctorCheck>,
    /// List of actionable issues.
    pub action_items: Vec<String>,
}

/// POST /api/system/doctor — Run system diagnostics.
pub(crate) async fn handle_doctor(state: State<Arc<AppState>>) -> Json<DoctorResponse> {
    // Clone what we need from config, don't hold the lock across await points
    let (default_model, api_key, workspace, daemon_log_dir) = {
        let config = state.config.read();
        (
            config.engine.default_model.clone(),
            config.api_key(),
            config.kernel.workspace.clone(),
            config.daemon.log_dir.clone(),
        )
    };
    let mut results = Vec::new();
    let mut action_items = Vec::new();

    // 1. Config file
    if state.config_path.exists() {
        results.push(DoctorCheck {
            name: "config_file".into(),
            status: "pass".into(),
            message: format!("Config file present ({})", state.config_path.display()),
        });
    } else {
        results.push(DoctorCheck {
            name: "config_file".into(),
            status: "fail".into(),
            message: "Config file missing".into(),
        });
        action_items.push("Config file not found. Run `oxios onboard` to create it.".into());
    }

    // 2. Credentials
    let provider = oxios_kernel::CredentialStore::provider_from_model(&default_model);
    match provider {
        Some(p) => match oxios_kernel::CredentialStore::resolve(p, api_key.as_deref()) {
            Some((key, source)) => {
                let preview = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    "(set)".to_string()
                };
                results.push(DoctorCheck {
                    name: "credentials".into(),
                    status: "pass".into(),
                    message: format!("Credentials found ({}, via {:?})", preview, source),
                });
            }
            None => {
                results.push(DoctorCheck {
                    name: "credentials".into(),
                    status: "fail".into(),
                    message: format!("No credentials for provider '{p}'"),
                });
                action_items.push(format!(
                    "No API key for '{p}'. Configure in Settings → Engine."
                ));
            }
        },
        None => {
            results.push(DoctorCheck {
                name: "credentials".into(),
                status: "fail".into(),
                message: "No model configured".into(),
            });
            action_items.push("No model set. Configure in Settings → Engine.".into());
        }
    }

    // 3. Workspace directory
    let workspace = oxios_kernel::config::expand_home(&workspace);
    if workspace.exists() {
        results.push(DoctorCheck {
            name: "workspace".into(),
            status: "pass".into(),
            message: format!("Workspace directory ({})", workspace.display()),
        });
    } else {
        results.push(DoctorCheck {
            name: "workspace".into(),
            status: "warn".into(),
            message: format!("Workspace directory missing ({})", workspace.display()),
        });
        action_items.push("Workspace directory not found. It will be created on first run.".into());
    }

    // 4. Default model
    if !default_model.is_empty() {
        results.push(DoctorCheck {
            name: "model".into(),
            status: "pass".into(),
            message: format!("Default model: {default_model}"),
        });
    } else {
        results.push(DoctorCheck {
            name: "model".into(),
            status: "fail".into(),
            message: "No default model set".into(),
        });
        action_items.push("No default model configured.".into());
    }

    // 5. MCP servers
    let mcp_count = state.kernel.mcp.server_count();
    if mcp_count > 0 {
        results.push(DoctorCheck {
            name: "mcp_servers".into(),
            status: "pass".into(),
            message: format!("{mcp_count} MCP server(s) connected"),
        });
    } else {
        results.push(DoctorCheck {
            name: "mcp_servers".into(),
            status: "warn".into(),
            message: "No MCP servers configured".into(),
        });
    }

    // 6. Git repository
    let git_ok = state.kernel.infra.git_verify().unwrap_or(false);
    if git_ok {
        results.push(DoctorCheck {
            name: "git".into(),
            status: "pass".into(),
            message: "Git repository intact".into(),
        });
    } else {
        results.push(DoctorCheck {
            name: "git".into(),
            status: "warn".into(),
            message: "Git repository verification failed".into(),
        });
    }

    // 7. State store
    let ws_path = state.kernel.state.workspace_path();
    if ws_path.exists() {
        results.push(DoctorCheck {
            name: "state_store".into(),
            status: "pass".into(),
            message: format!("State store path exists ({})", ws_path.display()),
        });
    } else {
        results.push(DoctorCheck {
            name: "state_store".into(),
            status: "warn".into(),
            message: "State store path not found".into(),
        });
    }

    // 8. Memory subsystem
    let (index_size, total) = state.kernel.agents.memory_stats().await;
    results.push(DoctorCheck {
        name: "memory".into(),
        status: "pass".into(),
        message: format!("Memory: {index_size} indexed, {total} total entries"),
    });

    // 9. Web dist directory
    if let Some(ref web_dist) = state.web_dist {
        if web_dist.exists() {
            results.push(DoctorCheck {
                name: "web_dist".into(),
                status: "pass".into(),
                message: format!("Web UI dist ({})", web_dist.display()),
            });
        } else {
            results.push(DoctorCheck {
                name: "web_dist".into(),
                status: "warn".into(),
                message: "Web UI dist directory not found".into(),
            });
        }
    } else {
        results.push(DoctorCheck {
            name: "web_dist".into(),
            status: "pass".into(),
            message: "Web UI served from embedded assets".into(),
        });
    }

    let checks = results.len() as u32;
    let issues = action_items.len() as u32;

    Json(DoctorResponse {
        checks,
        issues,
        results,
        action_items,
    })
}

/// Response for `POST /api/system/audit-verify`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct AuditVerifyResponse {
    pub valid: bool,
    pub entries_checked: u64,
    pub message: String,
}

/// POST /api/system/audit-verify — Verify audit trail integrity.
pub(crate) async fn handle_audit_verify_api(
    state: State<Arc<AppState>>,
) -> Json<AuditVerifyResponse> {
    let audit = &state.kernel.security;
    match audit.verify_chain() {
        Ok(valid) => Json(AuditVerifyResponse {
            valid,
            entries_checked: 0,
            message: if valid {
                "Audit trail verified successfully.".into()
            } else {
                "Audit trail verification failed.".into()
            },
        }),
        Err(e) => Json(AuditVerifyResponse {
            valid: false,
            entries_checked: 0,
            message: format!("Audit trail verification failed: {e}"),
        }),
    }
}

/// Response for `POST /api/system/backup`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct BackupResponse {
    pub success: bool,
    pub path: String,
    pub size_bytes: u64,
    pub message: String,
}

/// POST /api/system/backup — Create a backup of Oxios state.
pub(crate) async fn handle_backup(state: State<Arc<AppState>>) -> Json<BackupResponse> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => {
            return Json(BackupResponse {
                success: false,
                path: String::new(),
                size_bytes: 0,
                message: "Cannot determine home directory.".into(),
            })
        }
    };
    let oxios_home = home.join(".oxios");

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!("oxios-backup-{timestamp}.tar.gz");
    let backup_path = oxios_home.join(&backup_name);

    tracing::info!(path = %backup_path.display(), "Creating backup");

    // Use tar command for simplicity
    let output = match tokio::process::Command::new("tar")
        .args([
            "-czf",
            match backup_path.to_str() {
                Some(s) => s,
                None => {
                    return Json(BackupResponse {
                        success: false,
                        path: String::new(),
                        size_bytes: 0,
                        message: "Invalid backup path.".into(),
                    })
                }
            },
            "-C",
            oxios_home.to_str().unwrap_or("."),
            "config.toml",
            "workspace",
            "knowledge",
        ])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            return Json(BackupResponse {
                success: false,
                path: String::new(),
                size_bytes: 0,
                message: format!("tar failed: {e}"),
            })
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Json(BackupResponse {
            success: false,
            path: String::new(),
            size_bytes: 0,
            message: format!("Backup failed: {stderr}"),
        });
    }

    let size = std::fs::metadata(&backup_path)
        .map(|m| m.len())
        .unwrap_or(0);

    tracing::info!(
        path = %backup_path.display(),
        size,
        "Backup created"
    );

    Json(BackupResponse {
        success: true,
        path: backup_path.display().to_string(),
        size_bytes: size,
        message: format!(
            "Backup created: {backup_name} ({})",
            format_size_helper(size)
        ),
    })
}

/// Response for `GET /api/system/log`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct LogResponse {
    pub lines: Vec<String>,
    pub total: usize,
}

/// GET /api/system/log — Read recent daemon log entries.
pub(crate) async fn handle_log(state: State<Arc<AppState>>) -> Json<LogResponse> {
    let log_dir = {
        let config = state.config.read();
        oxios_kernel::config::expand_home(&config.daemon.log_dir)
    };
    let log_file = log_dir.join("oxios.log");

    if !log_file.exists() {
        return Json(LogResponse {
            lines: vec!["No log file found.".into()],
            total: 1,
        });
    }

    // Read last N lines efficiently
    let content = tokio::fs::read_to_string(&log_file)
        .await
        .unwrap_or_default();

    let all_lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let total = all_lines.len();
    let lines: Vec<String> = all_lines
        .into_iter()
        .rev()
        .take(50)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Json(LogResponse { lines, total })
}

fn format_size_helper(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
