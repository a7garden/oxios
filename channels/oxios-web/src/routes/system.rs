use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Serialize;

use crate::error::AppError;
use crate::routes::{paginate, PageParams};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// GET /health — Health check endpoint (no auth required).
pub(crate) async fn handle_health(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
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
                    status: format!("{:?}", a.status),
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

/// POST /api/agents/:id/kill — Kill an agent.
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
            // Add api_key_set flag so the frontend knows if a key is currently set
            json["engine"]["api_key_set"] = serde_json::Value::Bool(config.engine.api_key.is_some());
            Ok(Json(json))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize config");
            Err(AppError::Internal("failed to serialize config".into()))
        }
    }
}

/// PUT /api/config — Update configuration.
///
/// Validates the incoming JSON against the config schema, persists
/// changes to the config file on disk, and hot-reloads the in-memory config.
pub(crate) async fn handle_config_put(
    state: State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    tracing::info!("Config update requested");

    // Validate: parse as OxiosConfig to ensure the shape is correct.
    let updated: oxios_kernel::OxiosConfig = match serde_json::from_value(body.clone()) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid config shape");
            return Err(AppError::BadRequest(format!("Invalid config: {e}")));
        }
    };

    // Persist to the config file.
    let content = toml::to_string_pretty(&updated)
        .map_err(|e: toml::ser::Error| AppError::Internal(e.to_string()))?;
    if let Err(e) = tokio::fs::write(&state.config_path, content).await {
        tracing::error!(error = %e, "Failed to persist config");
        return Err(AppError::Internal(e.to_string()));
    }
    tracing::info!(path = %state.config_path.display(), "Config persisted");

    // Hot-reload: update in-memory config.
    *state.config.write() = updated;

    tracing::info!("Config hot-reloaded from {}", state.config_path.display());
    Ok(Json(body))
}
