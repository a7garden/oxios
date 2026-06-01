//! A2A (Agent-to-Agent) observation API routes.
//!
//! Read-only endpoints for monitoring the A2A protocol:
//! agent discovery, message log, and communication topology.

use std::collections::{HashMap, HashSet};
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
/// Returns the most recent 100 messages from the kernel's A2A message log.
pub(crate) async fn handle_a2a_messages(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entries = state.kernel.a2a.get_message_log(Some(100));
    let messages: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "from": e.from.to_string(),
                "to": e.to.to_string(),
                "message_type": e.message_type,
                "timestamp": e.timestamp.to_rfc3339(),
                "content": e.content,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "messages": messages })))
}

/// GET /api/a2a/topology — Agent communication topology.
///
/// Returns nodes from the agent card registry. Edges are derived
/// from the message log — each unique (from, to) pair becomes an edge.
pub(crate) async fn handle_a2a_topology(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state.kernel.a2a.protocol().registry().list_agents().await;
    let name_map: HashMap<uuid::Uuid, &str> = agents
        .iter()
        .map(|a| (a.agent_id, a.name.as_str()))
        .collect();

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

    // Derive edges from message log: unique (from, to) pairs.
    let log_entries = state.kernel.a2a.get_message_log(None);
    let mut edge_set = HashSet::new();
    for entry in &log_entries {
        edge_set.insert((entry.from, entry.to));
    }
    let edges: Vec<serde_json::Value> = edge_set
        .iter()
        .map(|(from, to)| {
            let from_label = name_map.get(from).copied().unwrap_or("unknown");
            let to_label = name_map.get(to).copied().unwrap_or("unknown");
            serde_json::json!({
                "source": from_label,
                "target": to_label,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "nodes": nodes,
        "edges": edges,
    })))
}
