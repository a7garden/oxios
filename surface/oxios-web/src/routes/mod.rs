//! API routes for the web channel.
//!
//! Route groups are split into sub-modules:
//! - **chat**: Chat and WebSocket streaming
//! - **system**: Health, status, agents, config
//! - **workspace**: File tree, seeds, skills, memory
//! - **resource_routes**: System resources
//! - **infra**: Scheduler, audit, permissions, MCP
//! - **events**: Sessions, SSE events, approvals

mod a2a;
mod agent_groups;
mod audit_routes;
mod budget_routes;
mod chat;
mod cron_jobs;
mod engine_routes;
mod events;
mod git_routes;
mod infra;
mod knowledge_routes;
mod marketplace;
mod project_routes;
mod resource_routes;
mod system;
mod workspace;

use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use serde::Deserialize;

use crate::middleware::{rate_limit_layer, require_auth};
use crate::persona_routes;
use crate::server::AppState;

// Re-export all handlers for use in build_routes
pub(crate) use a2a::{
    handle_a2a_agent_detail, handle_a2a_agents, handle_a2a_messages, handle_a2a_topology,
};
pub(crate) use agent_groups::{
    handle_agent_group_get, handle_agent_group_progress, handle_agent_groups_list,
};
pub(crate) use audit_routes::{
    handle_audit_by_agent, handle_audit_entries, handle_audit_export, handle_audit_flush,
    handle_audit_verify,
};
pub(crate) use budget_routes::{
    handle_budget_get, handle_budget_list, handle_budget_remove, handle_budget_reserve,
    handle_budget_reset, handle_budget_set,
};
pub(crate) use chat::{
    handle_chat, handle_chat_stream, handle_chat_ticket, handle_session_tool_calls,
};
pub(crate) use cron_jobs::{
    handle_cron_job_create, handle_cron_job_delete, handle_cron_job_get, handle_cron_job_trigger,
    handle_cron_jobs_list, update_cron_job,
};
pub(crate) use engine_routes::{
    handle_engine_config, handle_engine_models, handle_engine_providers,
    handle_engine_routing_fallbacks, handle_engine_routing_stats, handle_engine_set_api_key,
    handle_engine_set_model, handle_engine_set_provider_options, handle_engine_set_routing,
    handle_engine_validate_key,
};
pub(crate) use events::{
    handle_approval_approve, handle_approval_reject, handle_approvals_list, handle_events,
    handle_session_delete, handle_session_get, handle_sessions_list, handle_sessions_prune,
};
pub(crate) use git_routes::{
    handle_git_log, handle_git_restore, handle_git_tags, handle_git_verify,
};
pub(crate) use infra::{
    handle_audit_log, handle_mcp_server_delete, handle_mcp_server_refresh,
    handle_mcp_server_register, handle_mcp_server_toggle, handle_mcp_servers_list,
    handle_mcp_tool_call, handle_mcp_tools_list, handle_metrics, handle_permissions_get,
    handle_permissions_put, handle_scheduler_stats, handle_scheduler_tasks,
    handle_security_permissions,
};
pub(crate) use knowledge_routes::{
    handle_knowledge_backlinks, handle_knowledge_chat_append, handle_knowledge_chat_delete,
    handle_knowledge_chat_messages, handle_knowledge_chat_move, handle_knowledge_checklist_add,
    handle_knowledge_checklist_complete, handle_knowledge_checklist_items,
    handle_knowledge_checklist_remove, handle_knowledge_config_get, handle_knowledge_config_put,
    handle_knowledge_convert_html, handle_knowledge_copilot, handle_knowledge_emoji,
    handle_knowledge_file_delete, handle_knowledge_file_get, handle_knowledge_file_history,
    handle_knowledge_file_put, handle_knowledge_file_restore, handle_knowledge_graph,
    handle_knowledge_habits, handle_knowledge_habits_last_week, handle_knowledge_journal_add,
    handle_knowledge_journal_emoji, handle_knowledge_journal_today, handle_knowledge_search,
    handle_knowledge_stats_done_today, handle_knowledge_stats_today, handle_knowledge_tree,
    handle_knowledge_worker_nightly, handle_knowledge_worker_scheduled,
};
pub(crate) use marketplace::{
    handle_marketplace_install, handle_marketplace_search, handle_marketplace_skill_detail,
    handle_marketplace_updates,
};
pub(crate) use project_routes::{
    handle_project_create, handle_project_delete, handle_project_get, handle_project_link_memory,
    handle_project_memories, handle_project_unlink_memory, handle_project_update,
    handle_projects_list,
};
pub(crate) use resource_routes::{
    handle_resource_history, handle_resource_overload, handle_resource_snapshot,
};
pub(crate) use system::{
    handle_agent_kill, handle_agents_list, handle_config_get, handle_config_put, handle_health,
    handle_readiness, handle_status,
};
pub(crate) use workspace::{
    handle_memory_create, handle_memory_get, handle_memory_list, handle_memory_search,
    handle_memory_semantic_search, handle_seed_evolution, handle_seed_get, handle_seeds_list,
    handle_skill_content, handle_skill_create, handle_skill_delete, handle_skill_disable,
    handle_skill_enable, handle_skill_get, handle_skills_list, handle_workspace_file_create,
    handle_workspace_file_delete, handle_workspace_file_get, handle_workspace_file_put,
    handle_workspace_tree,
};

// ---------------------------------------------------------------------------
// Shared pagination types
// ---------------------------------------------------------------------------

/// Pagination query parameters.
#[derive(Debug, Deserialize, Default)]
pub struct PageParams {
    /// Page number (1-indexed).
    #[serde(default = "default_page")]
    pub page: usize,
    /// Items per page.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_page() -> usize {
    1
}
fn default_limit() -> usize {
    50
}

/// Apply pagination to a slice of items.
/// Returns a JSON value with `{items, total, page, limit}`.
pub fn paginate<T: Clone + serde::Serialize>(
    items: &[T],
    params: &PageParams,
) -> serde_json::Value {
    let total = items.len();
    let limit = params.limit.min(500);
    let offset = (params.page.saturating_sub(1)) * limit;
    serde_json::json!({
        "items": items.iter().skip(offset).take(limit).cloned().collect::<Vec<_>>(),
        "total": total,
        "page": params.page,
        "limit": limit,
    })
}

/// Builds the axum router with all API routes.
///
/// Auth middleware is applied to all `/api/*` routes.
/// `/health` and static assets are excluded from auth.
pub fn build_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    // Public routes (no auth)
    let public = Router::new()
        .route("/health", get(handle_health))
        .route("/health/ready", get(handle_readiness))
        .route("/metrics", get(handle_metrics))
        // Marketplace (ClawHub) — read-only routes, public
        .route("/api/marketplace/search", get(handle_marketplace_search))
        .route("/api/marketplace/updates", get(handle_marketplace_updates))
        .route(
            "/api/marketplace/skills/{slug}",
            get(handle_marketplace_skill_detail),
        );

    // Protected API routes (auth middleware applied)
    let api = Router::new()
        // Chat
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/ticket", post(handle_chat_ticket))
        .route("/api/chat/stream", get(handle_chat_stream))
        // Control
        .route("/api/status", get(handle_status))
        .route("/api/agents", get(handle_agents_list))
        .route("/api/agents/{id}/kill", post(handle_agent_kill))
        // Config
        .route("/api/config", get(handle_config_get))
        .route("/api/config", put(handle_config_put))
        // Engine
        .route("/api/engine/providers", get(handle_engine_providers))
        .route("/api/engine/models", get(handle_engine_models))
        .route("/api/engine/config", get(handle_engine_config))
        .route("/api/engine/model", put(handle_engine_set_model))
        .route("/api/engine/api-key", put(handle_engine_set_api_key))
        .route(
            "/api/engine/provider-options",
            put(handle_engine_set_provider_options),
        )
        .route("/api/engine/validate-key", post(handle_engine_validate_key))
        .route("/api/engine/routing", put(handle_engine_set_routing))
        .route(
            "/api/engine/routing/stats",
            get(handle_engine_routing_stats),
        )
        .route(
            "/api/engine/routing/fallbacks",
            get(handle_engine_routing_fallbacks),
        )
        // Workspace
        .route("/api/workspace/tree", get(handle_workspace_tree))
        .route(
            "/api/workspace/file/{*path}",
            get(handle_workspace_file_get),
        )
        .route(
            "/api/workspace/file/{*path}",
            put(handle_workspace_file_put),
        )
        .route(
            "/api/workspace/file/{*path}",
            post(handle_workspace_file_create),
        )
        .route(
            "/api/workspace/file/{*path}",
            delete(handle_workspace_file_delete),
        )
        // Seeds
        .route("/api/seeds", get(handle_seeds_list))
        .route("/api/seeds/{id}", get(handle_seed_get))
        .route("/api/seeds/{id}/evolution", get(handle_seed_evolution))
        // Skills
        .route("/api/skills", get(handle_skills_list))
        .route("/api/skills/{name}", get(handle_skill_get))
        .route("/api/skills", post(handle_skill_create))
        .route("/api/skills/{name}", delete(handle_skill_delete))
        .route("/api/skills/{name}/enable", post(handle_skill_enable))
        .route("/api/skills/{name}/disable", post(handle_skill_disable))
        .route("/api/skills/{name}/content", get(handle_skill_content))
        // Memory
        .route("/api/memory", get(handle_memory_list))
        .route("/api/memory", post(handle_memory_create))
        .route("/api/memory/search", post(handle_memory_search))
        .route("/api/memory/semantic", post(handle_memory_semantic_search))
        .route("/api/memory/{name}", get(handle_memory_get))
        // Scheduler stats & tasks
        .route("/api/scheduler/stats", get(handle_scheduler_stats))
        .route("/api/scheduler/tasks", get(handle_scheduler_tasks))
        // Audit log
        .route("/api/audit/entries", get(handle_audit_entries))
        .route("/api/audit/verify", get(handle_audit_verify))
        .route("/api/audit/agent/{agent_id}", get(handle_audit_by_agent))
        .route("/api/audit/export", post(handle_audit_export))
        .route("/api/audit/flush", post(handle_audit_flush))
        // Permissions
        .route("/api/audit", get(handle_audit_log))
        .route(
            "/api/security/permissions",
            get(handle_security_permissions),
        )
        .route("/api/permissions/{agent}", get(handle_permissions_get))
        .route("/api/permissions/{agent}", put(handle_permissions_put))
        // MCP
        .route(
            "/api/mcp/servers",
            get(handle_mcp_servers_list).post(handle_mcp_server_register),
        )
        .route("/api/mcp/servers/{name}", delete(handle_mcp_server_delete))
        .route(
            "/api/mcp/servers/{name}/toggle",
            post(handle_mcp_server_toggle),
        )
        .route(
            "/api/mcp/servers/{name}/refresh",
            post(handle_mcp_server_refresh),
        )
        .route(
            "/api/mcp/tools",
            get(handle_mcp_tools_list).post(handle_mcp_tool_call),
        )
        // Prometheus metrics
        .route("/api/metrics", get(handle_metrics))
        // Resources
        .route("/api/resources", get(handle_resource_snapshot))
        .route("/api/resources/history", get(handle_resource_history))
        .route("/api/resources/overload", get(handle_resource_overload))
        // Agent Groups
        .route("/api/agent-groups", get(handle_agent_groups_list))
        .route("/api/agent-groups/{id}", get(handle_agent_group_get))
        .route(
            "/api/agent-groups/{id}/progress",
            get(handle_agent_group_progress),
        )
        // A2A Monitor
        .route("/api/a2a/agents", get(handle_a2a_agents))
        .route("/api/a2a/agents/{id}", get(handle_a2a_agent_detail))
        .route("/api/a2a/messages", get(handle_a2a_messages))
        .route("/api/a2a/topology", get(handle_a2a_topology))
        // Events
        .route("/api/events", get(handle_events))
        // Personas (delegated to persona_routes)
        .route("/api/personas", get(persona_routes::handle_personas_list))
        .route("/api/personas", post(persona_routes::handle_persona_create))
        .route(
            "/api/personas/{id}",
            get(persona_routes::handle_persona_get),
        )
        .route(
            "/api/personas/{id}",
            put(persona_routes::handle_persona_update),
        )
        .route(
            "/api/personas/{id}",
            delete(persona_routes::handle_persona_delete),
        )
        .route(
            "/api/personas/active",
            get(persona_routes::handle_persona_active_get),
        )
        .route(
            "/api/personas/active",
            put(persona_routes::handle_persona_active_set),
        )
        // Sessions
        .route("/api/sessions", get(handle_sessions_list))
        .route("/api/sessions/prune", post(handle_sessions_prune))
        .route("/api/sessions/{id}", get(handle_session_get))
        .route("/api/sessions/{id}", delete(handle_session_delete))
        .route(
            "/api/sessions/{id}/tool-calls",
            get(handle_session_tool_calls),
        )
        // Cron Jobs
        .route("/api/cron-jobs", get(handle_cron_jobs_list))
        .route("/api/cron-jobs", post(handle_cron_job_create))
        .route("/api/cron-jobs/{id}", get(handle_cron_job_get))
        .route("/api/cron-jobs/{id}", delete(handle_cron_job_delete))
        .route("/api/cron-jobs/{id}/edit", post(update_cron_job))
        .route("/api/cron-jobs/{id}/trigger", post(handle_cron_job_trigger))
        // Approvals (HitL)
        .route("/api/approvals", get(handle_approvals_list))
        .route("/api/approvals/{id}/approve", post(handle_approval_approve))
        .route("/api/approvals/{id}/reject", post(handle_approval_reject))
        // Git
        .route("/api/git/log", get(handle_git_log))
        .route("/api/git/tags", get(handle_git_tags))
        .route("/api/git/verify", post(handle_git_verify))
        .route("/api/git/restore", post(handle_git_restore))
        // Projects
        .route("/api/projects", get(handle_projects_list))
        .route("/api/projects", post(handle_project_create))
        .route("/api/projects/{id}", get(handle_project_get))
        .route("/api/projects/{id}", put(handle_project_update))
        .route("/api/projects/{id}", delete(handle_project_delete))
        .route("/api/projects/{id}/memories", get(handle_project_memories))
        .route(
            "/api/projects/{id}/memories",
            post(handle_project_link_memory),
        )
        .route(
            "/api/projects/{id}/memories/{memoryId}",
            delete(handle_project_unlink_memory),
        )
        // Budget
        .route("/api/budget", get(handle_budget_list))
        .route("/api/budget/{agent_id}", get(handle_budget_get))
        .route("/api/budget/{agent_id}", post(handle_budget_set))
        .route("/api/budget/{agent_id}", delete(handle_budget_remove))
        .route(
            "/api/budget/{agent_id}/reserve",
            post(handle_budget_reserve),
        )
        .route("/api/budget/{agent_id}/reset", post(handle_budget_reset))
        // Knowledge
        .route("/api/knowledge/tree", get(handle_knowledge_tree))
        .route(
            "/api/knowledge/file/{*path}",
            get(handle_knowledge_file_get),
        )
        .route(
            "/api/knowledge/file/{*path}",
            put(handle_knowledge_file_put),
        )
        .route(
            "/api/knowledge/file/{*path}",
            delete(handle_knowledge_file_delete),
        )
        .route("/api/knowledge/search", post(handle_knowledge_search))
        .route("/api/knowledge/backlinks", get(handle_knowledge_backlinks))
        .route("/api/knowledge/graph", get(handle_knowledge_graph))
        .route("/api/knowledge/copilot", post(handle_knowledge_copilot))
        // Knowledge — Checklist
        .route(
            "/api/knowledge/checklist/items",
            post(handle_knowledge_checklist_items),
        )
        .route(
            "/api/knowledge/checklist/add",
            post(handle_knowledge_checklist_add),
        )
        .route(
            "/api/knowledge/checklist/complete",
            post(handle_knowledge_checklist_complete),
        )
        .route(
            "/api/knowledge/checklist/remove",
            post(handle_knowledge_checklist_remove),
        )
        // Knowledge — Chat
        .route(
            "/api/knowledge/chat/append",
            post(handle_knowledge_chat_append),
        )
        .route(
            "/api/knowledge/chat/messages",
            get(handle_knowledge_chat_messages),
        )
        .route(
            "/api/knowledge/chat/delete",
            post(handle_knowledge_chat_delete),
        )
        .route("/api/knowledge/chat/move", post(handle_knowledge_chat_move))
        // Knowledge — Journal
        .route(
            "/api/knowledge/journal/add",
            post(handle_knowledge_journal_add),
        )
        .route(
            "/api/knowledge/journal/emoji",
            post(handle_knowledge_journal_emoji),
        )
        .route(
            "/api/knowledge/journal/today",
            get(handle_knowledge_journal_today),
        )
        // Knowledge — Habits
        .route("/api/knowledge/habits", get(handle_knowledge_habits))
        .route(
            "/api/knowledge/habits/last-week",
            get(handle_knowledge_habits_last_week),
        )
        // Knowledge — Stats
        .route(
            "/api/knowledge/stats/today",
            get(handle_knowledge_stats_today),
        )
        .route(
            "/api/knowledge/stats/done-today",
            get(handle_knowledge_stats_done_today),
        )
        // Knowledge — Config
        .route("/api/knowledge/config", get(handle_knowledge_config_get))
        .route("/api/knowledge/config", put(handle_knowledge_config_put))
        // Knowledge — Worker
        .route(
            "/api/knowledge/worker/nightly",
            post(handle_knowledge_worker_nightly),
        )
        .route(
            "/api/knowledge/worker/scheduled",
            post(handle_knowledge_worker_scheduled),
        )
        // Knowledge — Convert & Emoji
        .route(
            "/api/knowledge/convert/html",
            post(handle_knowledge_convert_html),
        )
        .route("/api/knowledge/emoji", get(handle_knowledge_emoji))
        // Knowledge — Git version history
        .route(
            "/api/knowledge/file/{*path}/history",
            get(handle_knowledge_file_history),
        )
        .route(
            "/api/knowledge/file/{*path}/restore",
            post(handle_knowledge_file_restore),
        )
        // Marketplace (ClawHub) — install requires auth
        .route(
            "/api/marketplace/skills/{slug}/install",
            post(handle_marketplace_install),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone().rate_limiter.clone(),
            rate_limit_layer,
        ))
        .layer(axum::extract::DefaultBodyLimit::max(API_BODY_LIMIT))
        .with_state(state.clone());

    public.merge(api).with_state(state)
}

/// Body size limit for API requests (10 MB).
const API_BODY_LIMIT: usize = 10 * 1024 * 1024;
