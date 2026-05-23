use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use oxios_kernel::access_manager::AuditEntry;
use oxios_kernel::metrics::registry;
use oxios_kernel::ArgumentDef;

use crate::routes::PageParams;
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
    let stats = state.kernel.infra.scheduler_stats();
    Json(SchedulerStatsResponse {
        queued: stats.queued,
        running: stats.running,
        max_concurrent: stats.max_concurrent,
        rate_limit_per_minute: stats.rate_limit_per_minute,
        rate_remaining: stats.rate_remaining,
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

/// GET /api/scheduler/tasks — List queued and running tasks.
pub(crate) async fn handle_scheduler_tasks(
    state: State<Arc<AppState>>,
    _params: Query<PageParams>,
) -> Json<serde_json::Value> {
    let queued: Vec<TaskSummary> = state
        .kernel
        .infra
        .queued_tasks()
        .into_iter()
        .map(|t| TaskSummary {
            id: t.id.to_string(),
            description: t.description,
            priority: format!("{:?}", t.priority).to_lowercase(),
            status: format!("{:?}", t.status).to_lowercase(),
            created_at: t.created_at.to_rfc3339(),
            error: t.error,
        })
        .collect();

    let running: Vec<TaskSummary> = state
        .kernel
        .infra
        .running_tasks()
        .into_iter()
        .map(|t| TaskSummary {
            id: t.id.to_string(),
            description: t.description,
            priority: format!("{:?}", t.priority).to_lowercase(),
            status: format!("{:?}", t.status).to_lowercase(),
            created_at: t.created_at.to_rfc3339(),
            error: t.error,
        })
        .collect();

    Json(serde_json::json!({
        "queued": queued,
        "running": running,
    }))
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

fn default_page() -> usize {
    1
}
fn default_limit() -> usize {
    50
}

/// GET /api/audit — Get security audit log (paginated).
pub(crate) async fn handle_audit_log(
    state: State<Arc<AppState>>,
    Query(params): Query<AuditLogParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let entries = state.kernel.security.get_audit_log();
    let total = entries.len();
    let limit = params.limit.min(500);
    let offset = (params.page.saturating_sub(1)) * limit;
    let page_entries: Vec<_> = entries
        .iter()
        .rev()
        .skip(offset)
        .take(limit)
        .map(AuditEntryResponse::from)
        .collect();
    Ok(Json(serde_json::json!({
        "items": page_entries,
        "total": total,
        "page": params.page,
        "limit": limit,
    })))
}

/// Permission update request from JSON API.
/// We avoid derive(Deserialize) to prevent conflicts with kernel's PermissionUpdate.
pub(crate) struct PermissionsUpdate {
    allowed_tools: Option<Vec<String>>,
    allowed_paths: Option<Vec<String>>,
    denied_paths: Option<Vec<String>>,
    network_access: Option<bool>,
    max_execution_time_secs: Option<u64>,
    max_memory_mb: Option<u64>,
    can_fork: Option<bool>,
}

impl PermissionsUpdate {
    /// Parse from JSON value.
    pub fn from_json(value: serde_json::Value) -> Self {
        Self {
            allowed_tools: value
                .get("allowed_tools")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            allowed_paths: value
                .get("allowed_paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            denied_paths: value
                .get("denied_paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            network_access: value.get("network_access").and_then(|v| v.as_bool()),
            max_execution_time_secs: value
                .get("max_execution_time_secs")
                .and_then(|v| v.as_u64()),
            max_memory_mb: value.get("max_memory_mb").and_then(|v| v.as_u64()),
            can_fork: value.get("can_fork").and_then(|v| v.as_bool()),
        }
    }

    /// Convert to kernel PermissionUpdate.
    pub fn into_kernel(self) -> oxios_kernel::access_manager::PermissionUpdate {
        oxios_kernel::access_manager::PermissionUpdate {
            allowed_tools: self
                .allowed_tools
                .map(|t| t.into_iter().collect::<std::collections::HashSet<String>>()),
            allowed_paths: self.allowed_paths,
            denied_paths: self.denied_paths,
            network_access: self.network_access,
            max_execution_time_secs: self.max_execution_time_secs,
            max_memory_mb: self.max_memory_mb,
            can_fork: self.can_fork,
        }
    }
}

/// GET /api/permissions/:agent — Get permissions for an agent.
pub(crate) async fn handle_permissions_get(
    state: State<Arc<AppState>>,
    Path(agent): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.kernel.security.get_permissions(&agent) {
        Some(perms) => Ok(Json(serde_json::json!({
            "agent_name": perms.agent_name,
            "allowed_tools": perms.allowed_tools.iter().cloned().collect::<Vec<_>>(),
            "allowed_paths": perms.allowed_paths,
            "denied_paths": perms.denied_paths,
            "network_access": perms.network_access,
            "max_execution_time_secs": perms.max_execution_time_secs,
            "max_memory_mb": perms.max_memory_mb,
            "can_fork": perms.can_fork,
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// PUT /api/permissions/:agent — Set permissions for an agent.
pub(crate) async fn handle_permissions_put(
    state: State<Arc<AppState>>,
    Path(agent): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let update = PermissionsUpdate::from_json(body).into_kernel();
    state
        .kernel
        .security
        .update_permissions(&agent, update)
        .map_err(|e: anyhow::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
    let servers = state.kernel.mcp.list_servers();
    let mut results = Vec::new();
    for name in servers {
        let (command, args, enabled) = state
            .kernel
            .mcp
            .get_server(&name)
            .map(|s| (s.command.clone(), s.args.clone(), s.enabled))
            .unwrap_or_else(|| ("unknown".to_string(), Vec::new(), false));
        let initialized = state.kernel.mcp.client_status(&name).await.unwrap_or(false);
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
    let mut server = oxios_kernel::McpServer::new(&name, &command);
    server.args = body.args;
    server.enabled = true;
    state.kernel.mcp.register_server(server);
    state.kernel.mcp.init_server(&name).await.map_err(|e| {
        tracing::error!(server = %name, error = %e, "Failed to start MCP server");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
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
    let tools = match state.kernel.mcp.list_tools().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list MCP tools");
            Vec::new()
        }
    };

    // Reconstruct server attribution from cached tools.
    let mut results = Vec::new();
    for name in state.kernel.mcp.list_servers() {
        if let Some(cached) = state.kernel.mcp.cached_tools(&name).await {
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
    let result = state.kernel.mcp.call_tool(&body.server, &body.tool, body.arguments)
        .await
        .map_err(|e| {
            tracing::error!(server = %body.server, tool = %body.tool, error = %e, "MCP tool call failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Serialize the content blocks.
    let content: Vec<serde_json::Value> = result
        .content
        .iter()
        .map(|block| serde_json::to_value(block).unwrap_or_default())
        .collect();

    Ok(Json(serde_json::json!({
        "content": content,
        "is_error": result.is_error,
    })))
}

// ---------------------------------------------------------------------------
// Security / Permissions overview
// ---------------------------------------------------------------------------

/// GET /api/security/permissions — List roles and policies.
pub(crate) async fn handle_security_permissions(
    state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    use oxios_kernel::access_manager::Role;
    let roles: Vec<String> = [
        format!("{:?}", Role::User).to_lowercase(),
        format!("{:?}", Role::Superuser).to_lowercase(),
        format!("{:?}", Role::Admin).to_lowercase(),
    ]
    .into_iter()
    .map(|r| r.to_lowercase())
    .collect();

    // Build policy summaries from each role's default policy
    let mut policies = Vec::new();
    for role in [Role::User, Role::Superuser, Role::Admin] {
        let policy = role.default_policy();
        // Serialize the policy to get its allowed actions
        let val = serde_json::to_value(&policy).unwrap_or_default();
        policies.push(serde_json::json!({
            "name": format!("{}-default", format!("{:?}", role).to_lowercase()),
            "effect": "allow",
            "resources": val.get("allowed_actions").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                .unwrap_or_default(),
        }));
    }

    Json(serde_json::json!({
        "roles": roles,
        "policies": policies,
    }))
}
