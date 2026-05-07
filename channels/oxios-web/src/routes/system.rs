use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use oxios_kernel::{AgentId};
use uuid::Uuid;

use crate::error::AppError;
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// GET /health — Health check endpoint (no auth required).
pub(crate) async fn handle_health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "backend": {
            "container": state.container_manager.is_backend_available(),
        }
    }))
}

// ---------------------------------------------------------------------------
// Control
// ---------------------------------------------------------------------------

/// Response body for the status endpoint.
#[derive(Debug, Serialize)]
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
}

/// GET /api/status — System status.
pub(crate) async fn handle_status(
    _state: State<Arc<AppState>>,
) -> Json<StatusResponse> {
    Json(StatusResponse {
        service: "oxios".into(),
        status: "running".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        channels: vec!["web".into()],
        uptime: "n/a".into(),
    })
}

/// Agent summary for listing.
#[derive(Debug, Serialize)]
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
) -> Json<Vec<AgentSummary>> {
    match state.supervisor.list().await {
        Ok(agents) => Json(
            agents
                .into_iter()
                .map(|a| AgentSummary {
                    id: a.id.to_string(),
                    name: a.name,
                    status: format!("{:?}", a.status),
                    created_at: a.created_at.to_rfc3339(),
                    seed_id: a.seed_id.map(|s| s.to_string()),
                })
                .collect(),
        ),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list agents");
            Json(Vec::new())
        }
    }
}

/// POST /api/agents/:id/kill — Kill an agent.
pub(crate) async fn handle_agent_kill(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    tracing::info!(agent_id = %id, "Kill agent requested");
    let agent_id = match Uuid::parse_str(&id) {
        Ok(uuid) => AgentId::from(uuid),
        Err(e) => {
            tracing::warn!(error = %e, "Invalid agent ID");
            return Err(AppError::BadRequest("invalid agent ID".into()));
        }
    };
    match state.supervisor.kill(agent_id).await {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::warn!(error = %e, "Agent not found");
            Err(AppError::NotFound("agent not found".into()))
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// GET /api/config — Get current configuration.
pub(crate) async fn handle_config_get(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Serialize the actual config from AppState.
    let config = &*state.config;
    match serde_json::to_value(config) {
        Ok(json) => Ok(Json(json)),
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize config");
            Err(AppError::Internal("failed to serialize config".into()))
        }
    }
}

/// PUT /api/config — Update configuration.
///
/// Validates the incoming JSON against the config schema and persists
/// changes to the config file on disk.
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
    if let Some(config_path) = &state.config_path {
        let content = toml::to_string_pretty(&updated)
            .map_err(|e: toml::ser::Error| AppError::Internal(e.to_string()))?;
        if let Err(e) = tokio::fs::write(config_path, content).await {
            tracing::error!(error = %e, "Failed to persist config");
            return Err(AppError::Internal(e.to_string()));
        }
        tracing::info!("Config persisted to {:?}", config_path);
    }

    Ok(Json(body))
}
