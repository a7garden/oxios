//! A2A (Agent-to-Agent) observation API routes.
//!
//! Read-only endpoints for monitoring the A2A protocol:
//! agent discovery, message log, and communication topology.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use crate::error::AppError;
use crate::server::AppState;

/// GET /api/a2a/agents — List all registered agent cards.
pub(crate) async fn handle_a2a_agents(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state.kernel.a2a.protocol().registry().list_agents().await;
    Ok(Json(serde_json::json!({
        "agents": agents.iter().map(|a| serde_json::json!({
            "agent_id": a.agent_id.to_string(),
            "name": a.name,
            "description": a.description,
            "capabilities": a.capabilities,
            "skills": a.skills,
            "status": format!("{:?}", a.status).to_lowercase(),
            "endpoint": a.endpoint,
        })).collect::<Vec<_>>()
    })))
}

/// GET /api/a2a/agents/{id} — Get agent card detail.
pub(crate) async fn handle_a2a_agent_detail(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agent_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| AppError::Internal(format!("Invalid agent ID: {e}")))?;

    let card = state
        .kernel
        .a2a
        .protocol()
        .registry()
        .get_agent(agent_id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Agent '{id}' not found in A2A registry")))?;

    Ok(Json(serde_json::json!({
        "agent_id": card.agent_id.to_string(),
        "name": card.name,
        "description": card.description,
        "capabilities": card.capabilities,
        "skills": card.skills,
        "status": format!("{:?}", card.status).to_lowercase(),
        "endpoint": card.endpoint,
    })))
}

/// GET /api/a2a/messages — Recent A2A message log.
///
/// Returns an empty list until kernel message logging is implemented.
pub(crate) async fn handle_a2a_messages(
    _state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Implement message logging in kernel A2AProtocol
    Ok(Json(serde_json::json!({
        "messages": Vec::<serde_json::Value>::new()
    })))
}

/// GET /api/a2a/topology — Agent communication topology.
///
/// Returns nodes from the agent card registry. Edges are derived
/// from the message log when available.
pub(crate) async fn handle_a2a_topology(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state.kernel.a2a.protocol().registry().list_agents().await;
    let nodes: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.name,
                "label": a.name,
                "status": format!("{:?}", a.status).to_lowercase(),
            })
        })
        .collect();

    // Edges will be populated from message log when available
    Ok(Json(serde_json::json!({
        "nodes": nodes,
        "edges": Vec::<serde_json::Value>::new(),
    })))
}
