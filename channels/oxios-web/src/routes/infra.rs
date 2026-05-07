use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use oxios_kernel::access_manager::AuditEntry;
use oxios_kernel::ArgumentDef;
use oxios_kernel::metrics::registry;

use crate::routes::{PageParams, paginate};
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Prometheus Metrics
// ---------------------------------------------------------------------------

/// GET /api/metrics — Prometheus-compatible metrics endpoint.
pub(crate) async fn handle_metrics() -> Result<String, StatusCode> {
    Ok(registry().export())
}

// ---------------------------------------------------------------------------
// Scheduler (AIOS-inspired task scheduling)
// ---------------------------------------------------------------------------

/// Scheduler statistics response.
#[derive(Debug, Serialize)]
pub(crate) struct SchedulerStatsResponse {
    queued: usize,
    running: usize,
    max_concurrent: usize,
    rate_limit_per_minute: u32,
    rate_remaining: u32,
}

/// GET /api/scheduler/stats — Get scheduler statistics.
pub(crate) async fn handle_scheduler_stats(
    state: State<Arc<AppState>>,
) -> Json<SchedulerStatsResponse> {
    let stats = state.scheduler.stats();
    let rate_remaining = state.scheduler.rate_limit_remaining();
    Json(SchedulerStatsResponse {
        queued: stats.queued,
        running: stats.running,
        max_concurrent: stats.max_concurrent,
        rate_limit_per_minute: stats.rate_limit_per_minute,
        rate_remaining,
    })
}

/// Task summary for listing.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct TaskSummary {
    id: String,
    description: String,
    priority: String,
    status: String,
    created_at: String,
    error: Option<String>,
}

/// GET /api/scheduler/tasks — List queued and running tasks (paginated).
pub(crate) async fn handle_scheduler_tasks(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    let queued: Vec<TaskSummary> = state
        .scheduler
        .queued_tasks()
        .into_iter()
        .map(|t| TaskSummary {
            id: t.id.to_string(),
            description: t.description,
            priority: format!("{:?}", t.priority),
            status: format!("{:?}", t.status),
            created_at: t.created_at.to_rfc3339(),
            error: t.error,
        })
        .collect();

    let running: Vec<TaskSummary> = state
        .scheduler
        .running_tasks()
        .into_iter()
        .map(|t| TaskSummary {
            id: t.id.to_string(),
            description: t.description,
            priority: format!("{:?}", t.priority),
            status: format!("{:?}", t.status),
            created_at: t.created_at.to_rfc3339(),
            error: t.error,
        })
        .collect();

    // Combine and paginate
    let mut all_tasks = Vec::new();
    all_tasks.extend(queued);
    all_tasks.extend(running);
    Json(paginate(&all_tasks, &params))
}

// ---------------------------------------------------------------------------
// Audit & Permissions
// ---------------------------------------------------------------------------

/// Audit log entry response.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct AuditEntryResponse {
    timestamp: String,
    agent_name: String,
    action: String,
    resource: String,
    allowed: bool,
    reason: Option<String>,
}

impl From<&AuditEntry> for AuditEntryResponse {
    fn from(entry: &AuditEntry) -> Self {
        Self {
            timestamp: entry.timestamp.to_rfc3339(),
            agent_name: entry.agent_name.clone(),
            action: entry.action.clone(),
            resource: entry.resource.clone(),
            allowed: entry.allowed,
            reason: entry.reason.clone(),
        }
    }
}

/// Audit log with pagination.
#[derive(Debug, Deserialize)]
pub struct AuditLogParams {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_page() -> usize { 1 }
fn default_limit() -> usize { 50 }

/// GET /api/audit — Get security audit log (paginated).
pub(crate) async fn handle_audit_log(
    state: State<Arc<AppState>>,
    Query(params): Query<AuditLogParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let access = state.access_manager.lock();
    let entries = access.audit_log();
    let total = entries.len();
    let limit = params.limit.min(500);
    let offset = (params.page.saturating_sub(1)) * limit;
    let page_entries: Vec<_> = entries.iter().rev().skip(offset).take(limit).map(AuditEntryResponse::from).collect();
    Ok(Json(serde_json::json!({
        "items": page_entries,
        "total": total,
        "page": params.page,
        "limit": limit,
    })))
}

/// Permissions response.
#[derive(Debug, Serialize)]
pub(crate) struct PermissionsResponse {
    agent_name: String,
    allowed_tools: Vec<String>,
    allowed_paths: Vec<String>,
    denied_paths: Vec<String>,
    network_access: bool,
    max_execution_time_secs: u64,
    max_memory_mb: u64,
    can_fork: bool,
}

/// GET /api/permissions/:agent — Get permissions for an agent.
pub(crate) async fn handle_permissions_get(
    state: State<Arc<AppState>>,
    Path(agent): Path<String>,
) -> Result<Json<PermissionsResponse>, StatusCode> {
    let access = state.access_manager.lock();
    match access.get_permissions(&agent) {
        Some(perms) => Ok(Json(PermissionsResponse {
            agent_name: perms.agent_name.clone(),
            allowed_tools: perms.allowed_tools.iter().cloned().collect(),
            allowed_paths: perms.allowed_paths.clone(),
            denied_paths: perms.denied_paths.clone(),
            network_access: perms.network_access,
            max_execution_time_secs: perms.max_execution_time_secs,
            max_memory_mb: perms.max_memory_mb,
            can_fork: perms.can_fork,
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// PUT /api/permissions/:agent — Set permissions for an agent.
#[derive(Debug, Deserialize)]
pub(crate) struct PermissionsUpdate {
    allowed_tools: Option<Vec<String>>,
    allowed_paths: Option<Vec<String>>,
    denied_paths: Option<Vec<String>>,
    network_access: Option<bool>,
    max_execution_time_secs: Option<u64>,
    max_memory_mb: Option<u64>,
    can_fork: Option<bool>,
}

pub(crate) async fn handle_permissions_put(
    state: State<Arc<AppState>>,
    Path(agent): Path<String>,
    Json(body): Json<PermissionsUpdate>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut access = state.access_manager.lock();
    let perms = access.get_or_create_permissions(&agent);

    if let Some(tools) = body.allowed_tools {
        perms.allowed_tools = tools.into_iter().collect();
    }
    if let Some(paths) = body.allowed_paths {
        perms.allowed_paths = paths;
    }
    if let Some(paths) = body.denied_paths {
        perms.denied_paths = paths;
    }
    if let Some(v) = body.network_access {
        perms.network_access = v;
    }
    if let Some(v) = body.max_execution_time_secs {
        perms.max_execution_time_secs = v;
    }
    if let Some(v) = body.max_memory_mb {
        perms.max_memory_mb = v;
    }
    if let Some(v) = body.can_fork {
        perms.can_fork = v;
    }

    tracing::info!(agent = %agent, "Permissions updated");
    Ok(Json(serde_json::json!({
        "status": "updated",
        "agent": agent,
    })))
}

// ---------------------------------------------------------------------------
// MCP (Model Context Protocol)
// ---------------------------------------------------------------------------

/// MCP server configuration response.
#[allow(dead_code)] // Reserved for future MCP management API
#[derive(Debug, Serialize)]
pub(crate) struct McpServerResponse {
    name: String,
    command: String,
    args: Vec<String>,
    enabled: bool,
    initialized: bool,
}

/// GET /api/mcp/servers — List registered MCP servers.
#[allow(dead_code)] // Reserved for future MCP management API
pub(crate) async fn handle_mcp_servers_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<McpServerResponse>> {
    let bridge = &*state.mcp_bridge;
    let servers = bridge.servers();
    let mut results = Vec::new();
    for name in servers {
        let (command, args, enabled) = bridge
            .get_server(&name)
            .map(|s| (s.command.clone(), s.args.clone(), s.enabled))
            .unwrap_or_else(|| ("unknown".to_string(), Vec::new(), false));
        let initialized = if let Some(ref c) = bridge.client(&name).await {
            c.is_initialized().await
        } else {
            false
        };
        results.push(McpServerResponse {
            name: name.to_string(),
            command,
            args,
            enabled,
            initialized,
        });
    }
    Json(results)
}

/// MCP server registration request.
#[allow(dead_code)] // Reserved for future MCP management API
#[derive(Debug, Deserialize)]
pub(crate) struct McpServerRegisterRequest {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

/// POST /api/mcp/servers — Register a new MCP server and start it.
#[allow(dead_code)] // Reserved for future MCP management API
pub(crate) async fn handle_mcp_server_register(
    state: State<Arc<AppState>>,
    Json(body): Json<McpServerRegisterRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = body.name.clone();
    let command = body.command.clone();
    {
        let bridge = &*state.mcp_bridge;
        let mut server = oxios_kernel::McpServer::new(&name, &command);
        server.args = body.args;
        server.enabled = true;
        bridge.register_server(server);
    }
    if let Err(e) = state.mcp_bridge.initialize_server(&name).await {
        tracing::error!(server = %name, error = %e, "Failed to start MCP server");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
    }
    tracing::info!(server = %name, command = %command, "MCP server registered and started");
    Ok(Json(serde_json::json!({
        "status": "registered",
        "name": name,
        "command": command,
    })))
}

/// MCP tool summary exposed to agents.
#[allow(dead_code)] // Reserved for future MCP management API
#[derive(Debug, Serialize)]
pub(crate) struct McpToolResponse {
    name: String,
    description: String,
    server: String,
    arguments: Vec<ArgumentDef>,
}

/// GET /api/mcp/tools — List all available MCP tools.
#[allow(dead_code)] // Reserved for future MCP management API
pub(crate) async fn handle_mcp_tools_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<McpToolResponse>> {
    let bridge = &*state.mcp_bridge;
    let tools = match bridge.list_tools().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list MCP tools");
            Vec::new()
        }
    };

    // Reconstruct server attribution from cached tools.
    let mut results = Vec::new();
    for name in bridge.servers() {
        if let Some(cached) = bridge.cached_tools(&name).await {
            for tool in cached {
                results.push(McpToolResponse {
                    name: tool.name,
                    description: tool.description,
                    server: name.to_string(),
                    arguments: tool.arguments,
                });
            }
        }
    }
    // Fallback: if no cached tools, list from list_tools() with unknown server.
    if results.is_empty() {
        for tool in &tools {
            results.push(McpToolResponse {
                name: tool.name.clone(),
                description: tool.description.clone(),
                server: "<unknown>".to_string(),
                arguments: tool.arguments.clone(),
            });
        }
    }
    Json(results)
}

/// Request body for calling an MCP tool.
#[allow(dead_code)] // Reserved for future MCP management API
#[derive(Debug, Deserialize)]
pub(crate) struct McpToolCallRequest {
    server: String,
    tool: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

/// POST /api/mcp/tools — Call an MCP tool.
#[allow(dead_code)] // Reserved for future MCP management API
pub(crate) async fn handle_mcp_tool_call(
    state: State<Arc<AppState>>,
    Json(body): Json<McpToolCallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let result = state.mcp_bridge
        .call_tool(&body.server, &body.tool, body.arguments)
        .await
        .map_err(|e| {
            tracing::error!(server = %body.server, tool = %body.tool, error = %e, "MCP tool call failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Serialize the content blocks.
    let content: Vec<serde_json::Value> = result.content
        .iter()
        .map(|block| serde_json::to_value(block).unwrap_or_default())
        .collect();


    Ok(Json(serde_json::json!({
        "content": content,
        "is_error": result.is_error,
    })))
}
