//! A2A (Agent-to-Agent) observation API routes.
//!
//! Read-only endpoints for monitoring the A2A protocol:
//! agent discovery, message log, and communication topology.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use oxios_kernel::a2a::A2AMessageLogEntry;

use crate::api::error::AppError;
use crate::api::server::AppState;

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

/// Convert one [`A2AMessageLogEntry`] to the frontend `A2AMessage`
/// JSON shape (`request_id`, `from_agent`, `to_agent`, `message_type`,
/// `payload_summary`, `accepted`, `timestamp`).
///
/// `from_agent` / `to_agent` are the agent **names** looked up in
/// `name_map`; if an AgentId is not registered, the stringified UUID
/// is emitted as a fallback so the wire format is always a string.
///
/// `request_id` is freshly synthesized (the kernel log entry does not
/// yet persist the original A2A request UUID) and `accepted` is
/// `true` for every entry — presence in the log means acceptance.
fn message_entry_to_json(
    name_map: &HashMap<uuid::Uuid, String>,
    entry: &A2AMessageLogEntry,
) -> serde_json::Value {
    let from_agent = name_map
        .get(&entry.from)
        .cloned()
        .unwrap_or_else(|| entry.from.to_string());
    let to_agent = name_map
        .get(&entry.to)
        .cloned()
        .unwrap_or_else(|| entry.to.to_string());
    serde_json::json!({
        "request_id": uuid::Uuid::new_v4().to_string(),
        "from_agent": from_agent,
        "to_agent": to_agent,
        "message_type": entry.message_type,
        "payload_summary": entry.content,
        "accepted": true,
        "timestamp": entry.timestamp.to_rfc3339(),
    })
}

/// GET /api/a2a/messages — Recent A2A message log.
///
/// Returns the most recent 100 messages from the kernel's A2A message log.
/// Each entry is emitted in the frontend `A2AMessage` shape
/// (`request_id`, `from_agent`, `to_agent`, `message_type`,
/// `payload_summary`, `accepted`, `timestamp`).
///
/// `from_agent` / `to_agent` are the agent **names** (not UUIDs) so the
/// frontend can match them against `TopologyNode.id` for the inspector
/// filter. Unmapped agents fall back to the stringified UUID.
pub(crate) async fn handle_a2a_messages(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state.kernel.a2a.protocol().registry().list_agents().await;

    // Map AgentId -> human-readable name. Mirrors the topology handler.
    let name_map: HashMap<uuid::Uuid, String> = agents
        .iter()
        .map(|a| (a.agent_id, a.name.clone()))
        .collect();

    let entries = state.kernel.a2a.get_message_log(Some(100));
    let messages: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| message_entry_to_json(&name_map, e))
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
                "agent_id": a.agent_id.to_string(),
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
            let from_label = name_map
                .get(from)
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            let to_label = name_map
                .get(to)
                .cloned()
                .unwrap_or_else(|| "unknown".into());
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

#[cfg(test)]
mod tests {
    //! Unit tests for the `/api/a2a/messages` wire format.
    //!
    //! Regression coverage for the P0-1 fix: the route must emit the
    //! frontend `A2AMessage` shape (`request_id`, `from_agent`,
    //! `to_agent`, `message_type`, `payload_summary`, `accepted`,
    //! `timestamp`) instead of the old (`from`, `to`, `content`) shape.
    use super::message_entry_to_json;
    use chrono::{TimeZone, Utc};
    use oxios_kernel::a2a::A2AMessageLogEntry;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn sample_entry(from: Uuid, to: Uuid) -> A2AMessageLogEntry {
        A2AMessageLogEntry {
            from,
            to,
            message_type: "task_delegation".into(),
            timestamp: Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
            content: "review the PR".into(),
        }
    }

    #[test]
    fn emits_frontend_a2a_message_shape() {
        let from = Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap();
        let to = Uuid::parse_str("00000000-0000-0000-0000-00000000000b").unwrap();
        let mut name_map: HashMap<Uuid, String> = HashMap::new();
        name_map.insert(from, "agent-alpha".into());
        name_map.insert(to, "agent-beta".into());

        let entry = sample_entry(from, to);
        let json = message_entry_to_json(&name_map, &entry);

        // P0-1: field names must match the frontend A2AMessage type.
        let obj = json.as_object().expect("json object");
        assert!(obj.contains_key("request_id"), "missing request_id");
        assert!(obj.contains_key("from_agent"), "missing from_agent");
        assert!(obj.contains_key("to_agent"), "missing to_agent");
        assert!(obj.contains_key("message_type"), "missing message_type");
        assert!(
            obj.contains_key("payload_summary"),
            "missing payload_summary"
        );
        assert!(obj.contains_key("accepted"), "missing accepted");
        assert!(obj.contains_key("timestamp"), "missing timestamp");

        // The P0-1 fix replaces `from`/`to`/`content` with the new names.
        assert!(!obj.contains_key("from"), "stale `from` key present");
        assert!(!obj.contains_key("to"), "stale `to` key present");
        assert!(!obj.contains_key("content"), "stale `content` key present");

        // Values.
        assert_eq!(obj["from_agent"], "agent-alpha");
        assert_eq!(obj["to_agent"], "agent-beta");
        assert_eq!(obj["message_type"], "task_delegation");
        assert_eq!(obj["payload_summary"], "review the PR");
        assert_eq!(obj["accepted"], true);
        assert_eq!(obj["timestamp"], "2025-01-01T12:00:00+00:00");

        // request_id must be a valid UUID (synthesized per response).
        let rid = obj["request_id"].as_str().expect("request_id is string");
        Uuid::parse_str(rid).expect("request_id is a valid UUID");
    }

    #[test]
    fn unmapped_agent_falls_back_to_uuid() {
        // If an agent isn't in the registry, emit the stringified UUID
        // rather than a placeholder — keeps the wire format stable.
        let from = Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap();
        let to = Uuid::parse_str("00000000-0000-0000-0000-00000000000b").unwrap();
        let name_map: HashMap<Uuid, String> = HashMap::new(); // empty

        let entry = sample_entry(from, to);
        let json = message_entry_to_json(&name_map, &entry);
        let obj = json.as_object().unwrap();

        assert_eq!(obj["from_agent"], from.to_string());
        assert_eq!(obj["to_agent"], to.to_string());
    }

    #[test]
    fn two_entries_get_distinct_request_ids() {
        // request_id is synthesized per call, so two entries from the
        // same log must each get a unique ID (used as a React key).
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let name_map: HashMap<Uuid, String> = HashMap::new();
        let entry = sample_entry(from, to);

        let a = message_entry_to_json(&name_map, &entry);
        let b = message_entry_to_json(&name_map, &entry);
        assert_ne!(a["request_id"], b["request_id"]);
    }
}
