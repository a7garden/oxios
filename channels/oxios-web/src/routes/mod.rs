//! API routes for the web channel.
//!
//! Route groups are split into sub-modules:
//! - **chat**: Chat and WebSocket streaming
//! - **system**: Health, status, agents, config
//! - **workspace**: File tree, seeds, skills, memory
//! - **resources**: Programs, host-tools, system resources
//! - **infra**: Scheduler, audit, permissions, MCP
//! - **events**: Sessions, SSE events, approvals

mod agent_groups;
mod audit_routes;
mod budget_routes;
mod chat;
mod cron_jobs;
mod events;
mod git_routes;
mod infra;
mod resource_routes;
mod resources;
mod space_routes;
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
pub(crate) use agent_groups::{handle_agent_group_get, handle_agent_groups_list};
pub(crate) use audit_routes::{
    handle_audit_by_agent, handle_audit_entries, handle_audit_export, handle_audit_flush,
    handle_audit_verify,
};
pub(crate) use budget_routes::{
    handle_budget_get, handle_budget_remove, handle_budget_reserve, handle_budget_reset,
    handle_budget_set,
};
pub(crate) use chat::{handle_chat, handle_chat_stream};
pub(crate) use cron_jobs::{
    handle_cron_job_create, handle_cron_job_delete, handle_cron_job_get, handle_cron_job_trigger,
    handle_cron_jobs_list, update_cron_job,
};
pub(crate) use events::{
    handle_approval_approve, handle_approval_reject, handle_approvals_list, handle_events,
    handle_session_delete, handle_session_get, handle_sessions_list,
};
pub(crate) use git_routes::{
    handle_git_log, handle_git_restore, handle_git_tags, handle_git_verify,
};
pub(crate) use infra::{
    handle_audit_log, handle_metrics, handle_permissions_get, handle_permissions_put,
    handle_scheduler_stats, handle_scheduler_tasks,
};
pub(crate) use resource_routes::{
    handle_resource_history, handle_resource_overload, handle_resource_snapshot,
};
pub(crate) use resources::{
    handle_host_tools_check, handle_program_disable, handle_program_enable, handle_program_get,
    handle_program_host_requirements, handle_program_install, handle_program_uninstall,
    handle_programs_list,
};
pub(crate) use space_routes::{
    handle_knowledge_flow, handle_knowledge_flow_for, handle_space_activate, handle_space_archive,
    handle_space_current, handle_space_get, handle_space_merge, handle_space_restore,
    handle_spaces_list,
};
pub(crate) use system::{
    handle_agent_kill, handle_agents_list, handle_config_get, handle_config_put, handle_health,
    handle_status,
};
pub(crate) use workspace::{
    handle_memory_create, handle_memory_get, handle_memory_list, handle_memory_search,
    handle_memory_semantic_search, handle_seed_evolution, handle_seed_get, handle_seeds_list,
    handle_skill_create, handle_skill_delete, handle_skill_get, handle_skills_list,
    handle_workspace_file_get, handle_workspace_file_put, handle_workspace_tree,
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
    let public = Router::new().route("/health", get(handle_health));

    // Protected API routes (auth middleware applied)
    let api = Router::new()
        // Chat
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/stream", get(handle_chat_stream))
        // Control
        .route("/api/status", get(handle_status))
        .route("/api/agents", get(handle_agents_list))
        .route("/api/agents/{id}/kill", post(handle_agent_kill))
        // Config
        .route("/api/config", get(handle_config_get))
        .route("/api/config", put(handle_config_put))
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
        // Seeds
        .route("/api/seeds", get(handle_seeds_list))
        .route("/api/seeds/{id}", get(handle_seed_get))
        .route("/api/seeds/{id}/evolution", get(handle_seed_evolution))
        // Skills
        .route("/api/skills", get(handle_skills_list))
        .route("/api/skills/{name}", get(handle_skill_get))
        .route("/api/skills", post(handle_skill_create))
        .route("/api/skills/{name}", delete(handle_skill_delete))
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
        .route("/api/permissions/{agent}", get(handle_permissions_get))
        .route("/api/permissions/{agent}", put(handle_permissions_put))
        // Prometheus metrics
        .route("/api/metrics", get(handle_metrics))
        // Programs
        .route("/api/programs", get(handle_programs_list))
        .route("/api/programs", post(handle_program_install))
        .route("/api/programs/{name}", get(handle_program_get))
        .route("/api/programs/{name}", delete(handle_program_uninstall))
        .route("/api/programs/{name}/enable", post(handle_program_enable))
        .route("/api/programs/{name}/disable", post(handle_program_disable))
        .route(
            "/api/programs/{name}/host-requirements",
            get(handle_program_host_requirements),
        )
        // Host tools
        .route("/api/host-tools", get(handle_host_tools_check))
        // Resources
        .route("/api/resources", get(handle_resource_snapshot))
        .route("/api/resources/history", get(handle_resource_history))
        .route("/api/resources/overload", get(handle_resource_overload))
        // Agent Groups
        .route("/api/agent-groups", get(handle_agent_groups_list))
        .route("/api/agent-groups/{id}", get(handle_agent_group_get))
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
        .route("/api/sessions/{id}", get(handle_session_get))
        .route("/api/sessions/{id}", delete(handle_session_delete))
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
        // Spaces
        .route("/api/spaces", get(handle_spaces_list))
        .route("/api/spaces/current", get(handle_space_current))
        .route("/api/spaces/{id}", get(handle_space_get))
        .route("/api/spaces/{id}/activate", post(handle_space_activate))
        .route("/api/spaces/{id}/archive", post(handle_space_archive))
        .route("/api/spaces/{id}/restore", post(handle_space_restore))
        .route("/api/spaces/merge", post(handle_space_merge))
        .route("/api/spaces/knowledge-flow", get(handle_knowledge_flow))
        .route("/api/spaces/{id}/knowledge-flow", get(handle_knowledge_flow_for))
        // Budget
        .route("/api/budget/{agent_id}", get(handle_budget_get))
        .route("/api/budget/{agent_id}", post(handle_budget_set))
        .route("/api/budget/{agent_id}", delete(handle_budget_remove))
        .route(
            "/api/budget/{agent_id}/reserve",
            post(handle_budget_reserve),
        )
        .route("/api/budget/{agent_id}/reset", post(handle_budget_reset))
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
