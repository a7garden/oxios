//! A2A (Agent-to-Agent) protocol observation API.
//!
//! Read-only endpoints for observing A2A state:
//! - Registered agent cards (capabilities, skills, status)
//! - Message log (placeholder — requires kernel message logging)
//! - Communication topology (nodes from registry, edges from message log)

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use crate::error::AppError;
use crate::server::AppState;

/// GET /api/a2a/agents — List all registered agent cards.
pub(crate) async fn handle_a2a_agents(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state
        .kernel
        .a2a
        .protocol()
        .registry()
        .list_agents()
        .await;

    let cards: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "agent_id": a.agent_id.to_string(),
                "name": a.name,
                "description": a.description,
                "capabilities": a.capabilities,
                "skills": a.skills,
                "status": a.status.to_string().to_lowercase(),
                "endpoint": a.endpoint,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "agents": cards })))
}

/// GET /api/a2a/agents/{id} — Get a single agent card.
pub(crate) async fn handle_a2a_agent_detail(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agent_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid agent ID: {e}")))?;

    let card = state
        .kernel
        .a2a
        .protocol()
        .registry()
        .get_agent(agent_id)
        .await
        .ok_or_else(|| {
            AppError::NotFound(format!("Agent '{}' not found in A2A registry", id))
        })?;

    Ok(Json(serde_json::json!({
        "agent_id": card.agent_id.to_string(),
        "name": card.name,
        "description": card.description,
        "capabilities": card.capabilities,
        "skills": card.skills,
        "status": card.status.to_string().to_lowercase(),
        "endpoint": card.endpoint,
    })))
}

/// GET /api/a2a/messages — A2A message log.
///
/// Currently returns an empty array. Full message logging requires
/// adding an A2AMessageLog to the kernel that records each send/receive.
pub(crate) async fn handle_a2a_messages(
    _state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Integrate with kernel A2A message log when available.
    // The kernel's A2AProtocol publishes KernelEvent::MessageReceived on every send.
    // A future implementation could subscribe to that event bus and accumulate
    // a ring buffer of recent messages for this endpoint.
    Ok(Json(serde_json::json!({
        "messages": [] as Vec<serde_json::Value>,
        "note": "Message logging requires kernel A2AMessageLog (planned)"
    })))
}

/// GET /api/a2a/topology — Agent communication topology.
///
/// Nodes are derived from the agent registry. Edges require message logging
/// (planned for future kernel iteration).
pub(crate) async fn handle_a2a_topology(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state
        .kernel
        .a2a
        .protocol()
        .registry()
        .list_agents()
        .await;

    let nodes: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.agent_id.to_string(),
                "label": a.name,
                "status": a.status.to_string().to_lowercase(),
                "capabilities": a.capabilities,
            })
        })
        .collect();

    // Edges will be populated once kernel adds A2A message log.
    Ok(Json(serde_json::json!({
        "nodes": nodes,
        "edges": [] as Vec<serde_json::Value>,
        "note": "Edge discovery requires kernel A2AMessageLog (planned)"
    })))
}