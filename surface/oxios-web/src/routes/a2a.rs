//! A2A (Agent-to-Agent) observation API routes.
//!
//! Read-only endpoints for monitoring the A2A protocol:
//! agent discovery, message log, and communication topology.

use std::collections::HashMap;
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
/// Returns nodes (from the agent card registry) and edges (aggregated
/// from the last 5 minutes of A2A message log). Each edge carries a
/// `message_count_5m` and the type of the most recent message.
pub(crate) async fn handle_a2a_topology(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state.kernel.a2a.protocol().registry().list_agents().await;

    // Map AgentId -> human-readable name (used for both nodes and edges).
    let name_map: HashMap<uuid::Uuid, String> = agents
        .iter()
        .map(|a| (a.agent_id, a.name.clone()))
        .collect();

    // Recent (5 minute) window for edge aggregation.
    let recent = state.kernel.a2a.recent_messages(300);

    // Build a per-(from,to) aggregate.
    // Use ordered (from, to) tuples so directionality is preserved.
    let mut edge_aggregates: HashMap<(uuid::Uuid, uuid::Uuid), (u32, String)> = HashMap::new();
    for entry in &recent {
        let key = (entry.from, entry.to);
        let aggregate = edge_aggregates.entry(key).or_insert((0, String::new()));
        aggregate.0 = aggregate.0.saturating_add(1);
        // Most recent message_type wins (entries are in chronological order
        // in the log, so the last update is the most recent).
        aggregate.1 = entry.message_type.clone();
    }

    // Build node-level last_seen timestamps from the recent window.
    let mut last_seen: HashMap<uuid::Uuid, chrono::DateTime<chrono::Utc>> = HashMap::new();
    for entry in &recent {
        let prev = last_seen.get(&entry.from).copied();
        if prev.is_none_or(|p| entry.timestamp > p) {
            last_seen.insert(entry.from, entry.timestamp);
        }
        let prev = last_seen.get(&entry.to).copied();
        if prev.is_none_or(|p| entry.timestamp > p) {
            last_seen.insert(entry.to, entry.timestamp);
        }
    }

    let nodes: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            let last = last_seen.get(&a.agent_id).map(|t| t.to_rfc3339());
            serde_json::json!({
                "id": a.name,
                "label": a.name,
                "status": format!("{:?}", a.status).to_lowercase(),
                "capabilities": a.capabilities,
                "skills": a.skills,
                "last_seen": last,
            })
        })
        .collect();

    let edges: Vec<serde_json::Value> = edge_aggregates
        .iter()
        .map(|((from, to), (count, last_kind))| {
            let from_label = name_map.get(from).cloned().unwrap_or_else(|| "unknown".into());
            let to_label = name_map.get(to).cloned().unwrap_or_else(|| "unknown".into());
            serde_json::json!({
                "from": from_label,
                "to": to_label,
                "message_count_5m": *count,
                "last_kind": last_kind,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "nodes": nodes,
        "edges": edges,
    })))
}
