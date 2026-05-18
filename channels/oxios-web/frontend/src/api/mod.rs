//! API types and fetch helpers for the Oxios backend.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Global signals
// ---------------------------------------------------------------------------

/// Global signal for the last API error (consumed by AppLayout toast).
static LAST_ERROR: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Global signal for the API authentication token.
static AUTH_TOKEN: GlobalSignal<Option<String>> = Signal::global(|| None);

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

/// Read the current auth token.
pub fn auth_token() -> Option<String> {
    AUTH_TOKEN().clone()
}

/// Set the auth token (called from login UI or localStorage init).
pub fn set_auth_token(token: Option<String>) {
    *AUTH_TOKEN.write() = token;
}

/// Build the Authorization header value if a token is set.
fn auth_header() -> Option<String> {
    AUTH_TOKEN().as_ref().map(|t| format!("Bearer {t}"))
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
    let mut req = gloo_net::http::Request::get(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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
    let mut req = gloo_net::http::Request::post(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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
    let mut req = gloo_net::http::Request::post(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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
    let mut req = gloo_net::http::Request::put(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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
    let mut req = gloo_net::http::Request::delete(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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
    let mut req = gloo_net::http::Request::post(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?;
    check_status(&resp, "POST", path)
}

/// DELETE, checking for HTTP errors.
pub async fn delete_action(path: &str) -> Result<(), String> {
    let mut req = gloo_net::http::Request::delete(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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
    let mut req = gloo_net::http::Request::put(path);
    if let Some(hdr) = auth_header() {
        req = req.header("Authorization", &hdr);
    }
    let resp = req
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

/// Chat request sent to the backend (matches backend `routes::chat::ChatRequest`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// The user's message content.
    pub content: String,
    /// User identifier (defaults to "default").
    #[serde(default = "default_user_id")]
    pub user_id: String,
    /// Session ID for multi-turn conversations.
    #[serde(default)]
    pub session_id: String,
}

fn default_user_id() -> String {
    "default".to_string()
}

/// Chat response from the backend (matches backend `routes::chat::ChatResponse`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The message ID.
    pub id: String,
    /// Echo of the user's message.
    #[allow(dead_code)]
    pub echo: String,
    /// The response from the orchestrator.
    pub reply: String,
    /// Session ID for multi-turn conversations.
    pub session_id: Option<String>,
    /// Phase reached during orchestration.
    pub phase: Option<String>,
    /// Evaluation metadata (optional).
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

// ---------------------------------------------------------------------------
// Cron Jobs (backend /api/cron-jobs)
// ---------------------------------------------------------------------------

/// Cron job summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobSummary {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub last_run: Option<String>,
    #[serde(default)]
    pub next_run: Option<String>,
}

/// Create/edit cron job request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCronJobRequest {
    pub name: String,
    pub schedule: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default = "default_toolchain")]
    pub toolchain: String,
}

fn default_toolchain() -> String {
    "default".to_string()
}

// ---------------------------------------------------------------------------
// Sessions (backend /api/sessions)
// ---------------------------------------------------------------------------

/// Session summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListItem {
    pub id: String,
    pub user_id: String,
    pub message_count: usize,
    #[serde(default)]
    pub active_seed_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Git (backend /api/git)
// ---------------------------------------------------------------------------

/// Git commit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogEntry {
    pub hash: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// Git tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitTag {
    pub name: String,
    #[serde(default)]
    pub hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Budget (backend /api/budget)
// ---------------------------------------------------------------------------

/// Budget info for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetInfo {
    pub agent_id: String,
    pub tokens_remaining: u64,
    pub calls_remaining: u64,
    pub window_remaining_secs: u64,
    pub is_exhausted: bool,
}

/// Set budget request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBudgetRequest {
    pub token_budget: u64,
    pub calls_budget: u64,
    pub window_secs: u64,
}

// ---------------------------------------------------------------------------
// Spaces (backend /api/spaces)
// ---------------------------------------------------------------------------

/// Space summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub id: String,
    pub name: String,
    pub source: String,
    pub active: bool,
    pub paths: Vec<String>,
    pub interaction_count: u64,
    pub knowledge_visible: bool,
    pub last_active: String,
}

/// Knowledge flow entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFlowInfo {
    pub from: String,
    pub to: String,
    pub flow_type: String,
    pub entry_count: usize,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Resources (backend /api/resources)
// ---------------------------------------------------------------------------

/// System overload status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverloadStatus {
    pub overloaded: bool,
    pub threshold: ThresholdInfo,
}

/// Resource threshold configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdInfo {
    #[serde(default)]
    pub cpu_percent: f64,
    #[serde(default)]
    pub memory_percent: f64,
    #[serde(default)]
    pub load_avg: f64,
}
