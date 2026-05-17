//! API types and fetch helpers for the Oxios backend.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Global error toast
// ---------------------------------------------------------------------------

/// Global signal for the last API error (consumed by AppLayout toast).
static LAST_ERROR: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Read the current global API error.
pub fn last_api_error() -> Option<String> {
    LAST_ERROR().clone()
}

/// Set the global API error (shown as toast).
fn set_api_error(msg: String) {
    *LAST_ERROR.write() = Some(msg);
}

/// Clear the global API error (dismiss toast).
pub fn clear_api_error() {
    *LAST_ERROR.write() = None;
}

// ---------------------------------------------------------------------------
// HTTP response helpers
// ---------------------------------------------------------------------------

/// Check HTTP status and return an error for 4xx/5xx responses.
fn check_status(response: &gloo_net::http::Response, method: &str, path: &str) -> Result<(), String> {
    let status = response.status();
    if status >= 400 {
        let msg = format!("{method} {path}: HTTP {status}");
        set_api_error(msg.clone());
        Err(msg)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Fetch helpers (gloo-net)
// ---------------------------------------------------------------------------

/// GET JSON from `path`, deserializing into `T`.
pub async fn fetch_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::get(path)
        .send()
        .await
        .map_err(|e| format!("GET {path}: {e}"))?;
    check_status(&resp, "GET", path)?;
    resp.json::<T>()
        .await
        .map_err(|e| format!("GET {path} decode: {e}"))
}

/// POST JSON body to `path`, deserializing the response into `T`.
pub async fn post_json<T: serde::de::DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    let resp = gloo_net::http::Request::post(path)
        .json(body)
        .map_err(|e| format!("POST {path} encode: {e}"))?
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?;
    check_status(&resp, "POST", path)?;
    resp.json::<T>()
        .await
        .map_err(|e| format!("POST {path} decode: {e}"))
}

/// POST with no body, deserializing the response into `T`.
#[allow(dead_code)]
pub async fn post_empty<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::post(path)
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?;
    check_status(&resp, "POST", path)?;
    resp.json::<T>()
        .await
        .map_err(|e| format!("POST {path} decode: {e}"))
}

/// PUT JSON body to `path`, deserializing the response into `T`.
#[allow(dead_code)]
pub async fn put_json<T: serde::de::DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    let resp = gloo_net::http::Request::put(path)
        .json(body)
        .map_err(|e| format!("PUT {path} encode: {e}"))?
        .send()
        .await
        .map_err(|e| format!("PUT {path}: {e}"))?;
    check_status(&resp, "PUT", path)?;
    resp.json::<T>()
        .await
        .map_err(|e| format!("PUT {path} decode: {e}"))
}

/// DELETE `path`, deserializing the response into `T`.
#[allow(dead_code)]
pub async fn delete_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::delete(path)
        .send()
        .await
        .map_err(|e| format!("DELETE {path}: {e}"))?;
    check_status(&resp, "DELETE", path)?;
    resp.json::<T>()
        .await
        .map_err(|e| format!("DELETE {path} decode: {e}"))
}

/// POST with no body, checking for HTTP errors. Used for action endpoints
/// that return status codes or simple JSON we don't need to parse.
pub async fn post_action(path: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::post(path)
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?;
    check_status(&resp, "POST", path)
}

/// DELETE, checking for HTTP errors.
pub async fn delete_action(path: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::delete(path)
        .send()
        .await
        .map_err(|e| format!("DELETE {path}: {e}"))?;
    check_status(&resp, "DELETE", path)
}

/// GET paginated JSON from `path`, returning just the items vector.
pub async fn fetch_paginated<T: serde::de::DeserializeOwned>(path: &str) -> Result<Vec<T>, String> {
    fetch_json::<PaginatedResponse<T>>(path)
        .await
        .map(|r| r.items)
}

/// GET raw text from `path`.
pub async fn fetch_text(path: &str) -> Result<String, String> {
    let resp = gloo_net::http::Request::get(path)
        .send()
        .await
        .map_err(|e| format!("GET {path}: {e}"))?;
    check_status(&resp, "GET", path)?;
    resp.text()
        .await
        .map_err(|e| format!("GET {path} text: {e}"))
}

/// PUT raw text body to `path`.
#[allow(dead_code)]
pub async fn put_text(path: &str, body: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::put(path)
        .body(body)
        .map_err(|e| format!("PUT {path} encode: {e}"))?
        .send()
        .await
        .map_err(|e| format!("PUT {path}: {e}"))?;
    check_status(&resp, "PUT", path)
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

/// System status response (matches backend `routes::system::StatusResponse`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StatusResponse {
    pub service: String,
    pub status: String,
    pub version: String,
    #[serde(default)]
    pub channels: Vec<String>,
    pub uptime: String,
    #[serde(default)]
    pub components: Option<ComponentHealth>,
}

/// Component-level health (matches backend `routes::system::ComponentHealth`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ComponentHealth {
    pub state_store: ComponentStatus,
    pub event_bus: ComponentStatus,
    pub memory: MemoryHealth,
    pub agents: AgentHealth,
}

/// Individual component status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ComponentStatus {
    pub healthy: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Memory subsystem health.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MemoryHealth {
    pub enabled: bool,
    pub index_size: usize,
    pub total_entries: usize,
}

/// Agent subsystem health.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentHealth {
    pub active_count: usize,
    pub total_forked: u64,
    pub total_completed: u64,
    pub total_failed: u64,
}

/// Paginated response wrapper from the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub limit: usize,
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

// ---------------------------------------------------------------------------
// Skills (backend /api/skills)
// ---------------------------------------------------------------------------

/// Skill information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Memory (backend /api/memory)
// ---------------------------------------------------------------------------

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
// Workspace (backend /api/workspace)
// ---------------------------------------------------------------------------

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

/// SSE event entry stored for display.
#[derive(Debug, Clone)]
pub struct EventEntry {
    pub time: String,
    pub event_type: String,
    pub data: String,
}
