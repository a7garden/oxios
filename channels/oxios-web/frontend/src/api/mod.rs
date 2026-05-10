//! API types and fetch helpers for the Oxios backend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Fetch helpers (gloo-net)
// ---------------------------------------------------------------------------

/// GET JSON from `path`, deserializing into `T`.
pub async fn fetch_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    gloo_net::http::Request::get(path)
        .send()
        .await
        .map_err(|e| format!("GET {path}: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("GET {path} decode: {e}"))
}

/// POST JSON body to `path`, deserializing the response into `T`.
pub async fn post_json<T: serde::de::DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    gloo_net::http::Request::post(path)
        .json(body)
        .map_err(|e| format!("POST {path} encode: {e}"))?
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("POST {path} decode: {e}"))
}

/// POST with no body, deserializing the response into `T`.
#[allow(dead_code)]
pub async fn post_empty<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    gloo_net::http::Request::post(path)
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("POST {path} decode: {e}"))
}

/// PUT JSON body to `path`, deserializing the response into `T`.
#[allow(dead_code)]
pub async fn put_json<T: serde::de::DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    gloo_net::http::Request::put(path)
        .json(body)
        .map_err(|e| format!("PUT {path} encode: {e}"))?
        .send()
        .await
        .map_err(|e| format!("PUT {path}: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("PUT {path} decode: {e}"))
}

/// DELETE `path`, deserializing the response into `T`.
#[allow(dead_code)]
pub async fn delete_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    gloo_net::http::Request::delete(path)
        .send()
        .await
        .map_err(|e| format!("DELETE {path}: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("DELETE {path} decode: {e}"))
}

/// POST with no body, ignoring the response body. Used for action endpoints
/// that return status codes or simple JSON we don't need to parse.
pub async fn post_action(path: &str) -> Result<(), String> {
    gloo_net::http::Request::post(path)
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?;
    Ok(())
}

/// DELETE, ignoring the response body.
pub async fn delete_action(path: &str) -> Result<(), String> {
    gloo_net::http::Request::delete(path)
        .send()
        .await
        .map_err(|e| format!("DELETE {path}: {e}"))?;
    Ok(())
}

/// GET raw text from `path`.
pub async fn fetch_text(path: &str) -> Result<String, String> {
    gloo_net::http::Request::get(path)
        .send()
        .await
        .map_err(|e| format!("GET {path}: {e}"))?
        .text()
        .await
        .map_err(|e| format!("GET {path} text: {e}"))
}

/// PUT raw text body to `path`.
#[allow(dead_code)]
pub async fn put_text(path: &str, body: &str) -> Result<(), String> {
    gloo_net::http::Request::put(path)
        .body(body)
        .map_err(|e| format!("PUT {path} encode: {e}"))?
        .send()
        .await
        .map_err(|e| format!("PUT {path}: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Chat types
// ---------------------------------------------------------------------------

/// Chat request sent to the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Chat response from the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub response: String,
    pub session_id: String,
    pub phase: Option<String>,
    pub evaluation: Option<EvaluationInfo>,
}

/// Evaluation metadata returned with some chat responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EvaluationInfo {
    pub score: f64,
    pub feedback: String,
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// Dashboard / Status types
// ---------------------------------------------------------------------------

/// System status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StatusResponse {
    pub version: String,
    pub uptime_secs: u64,
    pub active_agents: usize,

    pub total_seeds: usize,
    #[serde(default)]
    pub kernel_status: Option<String>,
}

/// Agent information (used by dashboard).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub started_at: Option<String>,
}

/// Scheduler statistics (used by dashboard).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SchedulerStats {
    pub pending_tasks: usize,
    pub running_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    #[serde(default)]
    pub uptime_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// Agent list (backend /api/agents)
// ---------------------------------------------------------------------------

/// Agent summary from backend listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub seed_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Seeds (backend /api/seeds)
// ---------------------------------------------------------------------------

/// Seed summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedSummary {
    pub id: String,
    pub goal: String,
    pub constraints_count: usize,
    pub created_at: String,
}

/// Detailed seed info — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SeedInfo {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub spec: String,
    #[serde(default)]
    pub evaluation_score: Option<f64>,
    #[serde(default)]
    pub iterations: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Skills (backend /api/skills)
// ---------------------------------------------------------------------------

/// Skill information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Memory (backend /api/memory)
// ---------------------------------------------------------------------------

/// Memory / knowledge entry — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Memory list item from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListItem {
    pub name: String,
    pub category: String,
}

/// Memory detail from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDetail {
    pub name: String,
    pub category: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Scheduler (backend /api/scheduler)
// ---------------------------------------------------------------------------

/// Scheduler stats from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStatsResponse {
    pub queued: usize,
    pub running: usize,
    pub max_concurrent: usize,
    pub rate_limit_per_minute: u32,
    pub rate_remaining: u32,
}

/// Task summary for scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub description: String,
    pub priority: String,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// Scheduler tasks response from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerTasks {
    pub queued: Vec<TaskSummary>,
    pub running: Vec<TaskSummary>,
}

// ---------------------------------------------------------------------------
// Security / Audit (backend /api/audit)
// ---------------------------------------------------------------------------

/// Audit log entry — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub agent: String,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
}

/// Audit entry from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: String,
    pub agent_name: String,
    pub action: String,
    pub resource: String,
    pub allowed: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Approvals (backend /api/approvals)
// ---------------------------------------------------------------------------

/// Approval request information — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ApprovalInfo {
    pub id: String,
    pub agent: String,
    pub action: String,
    pub status: String,
    #[serde(default)]
    pub requested_at: Option<String>,
    #[serde(default)]
    pub resolved_at: Option<String>,
}

/// Approval response from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    pub id: String,
    pub subject: String,
    pub action: String,
    pub resource: String,
    pub reason: String,
    pub created_at: String,
    pub status: String,
}

// ---------------------------------------------------------------------------
// Programs (backend /api/programs)
// ---------------------------------------------------------------------------

/// Program information — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProgramInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
}

/// Program summary from backend listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub enabled: bool,
    pub tools_count: usize,
    pub has_skill_content: bool,
}

// ---------------------------------------------------------------------------
// Host Tools (backend /api/host-tools)
// ---------------------------------------------------------------------------

/// Host tool status — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct HostToolStatus {
    pub all_required_present: bool,
    #[serde(default)]
    pub missing_required: Vec<String>,
    #[serde(default)]
    pub optional_available: Vec<String>,
}

/// Host tools status from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostToolsStatusResponse {
    pub all_required_present: bool,
    pub missing_required: Vec<String>,
    pub optional_available: HashMap<String, bool>,
}

// ---------------------------------------------------------------------------
// Personas (backend /api/personas)
// ---------------------------------------------------------------------------

/// Persona information — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PersonaInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub active: Option<bool>,
}

/// Persona summary from backend listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSummary {
    pub id: String,
    pub name: String,
    pub role: String,
    pub description: String,
    pub enabled: bool,
    #[serde(default)]
    pub personality_traits: Vec<String>,
}

// ---------------------------------------------------------------------------
// MCP (stub)
// ---------------------------------------------------------------------------

/// MCP server information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct McpServerInfo {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub tools_count: Option<usize>,
}

// ---------------------------------------------------------------------------
// Config (backend /api/config)
// ---------------------------------------------------------------------------

/// Configuration response — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ConfigResponse {
    pub toml: String,
    #[serde(default)]
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Workspace (backend /api/workspace)
// ---------------------------------------------------------------------------

/// Workspace tree entry — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TreeEntryOld {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub children: Vec<TreeEntryOld>,
}

/// Workspace tree entry from backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

// ---------------------------------------------------------------------------
// SSE events
// ---------------------------------------------------------------------------

/// SSE event from the event bus — kept for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SseEvent {
    pub event_type: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// SSE event entry stored for display.
#[derive(Debug, Clone)]
pub struct EventEntry {
    pub time: String,
    pub event_type: String,
    pub data: String,
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// Session information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SessionInfo {
    pub id: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub message_count: Option<usize>,
}
