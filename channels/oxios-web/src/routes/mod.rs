//! API routes for the web channel.
//!
//! Route groups are split into sub-modules:
//! - **chat**: Chat and WebSocket streaming
//! - **system**: Health, status, agents, config
//! - **workspace**: File tree, seeds, skills, memory
//! - **resources**: Gardens, programs, host-tools
//! - **infra**: Scheduler, audit, permissions, MCP
//! - **events**: Sessions, SSE events, approvals

mod agent_groups;
mod chat;
mod cron_jobs;
mod events;
mod git_routes;
mod infra;
mod resources;
mod system;
mod workspace;

use std::sync::Arc;

use axum::{routing::{delete, get, post, put}, Router};
use serde::Deserialize;

use crate::middleware::{require_auth, rate_limit_layer};
use crate::server::AppState;
use crate::persona_routes;

// Re-export all handlers for use in build_routes
pub(crate) use chat::{handle_chat, handle_chat_stream};
pub(crate) use cron_jobs::{handle_cron_jobs_list, handle_cron_job_create, handle_cron_job_get, handle_cron_job_delete, update_cron_job, handle_cron_job_trigger};
pub(crate) use events::{handle_events, handle_sessions_list, handle_session_get, handle_session_delete, handle_approvals_list, handle_approval_approve, handle_approval_reject};
pub(crate) use infra::{handle_audit_log, handle_metrics, handle_permissions_get, handle_permissions_put, handle_scheduler_stats, handle_scheduler_tasks};
pub(crate) use resources::{handle_gardens_list, handle_garden_create, handle_garden_start, handle_garden_stop, handle_garden_remove, handle_garden_exec, handle_programs_list, handle_program_get, handle_program_install, handle_program_uninstall, handle_program_enable, handle_program_disable, handle_program_host_requirements, handle_host_tools_check};
pub(crate) use system::{handle_health, handle_status, handle_agents_list, handle_agent_kill, handle_config_get, handle_config_put, handle_container_tools, handle_container_create, handle_toolchains_list};
pub(crate) use agent_groups::{handle_agent_groups_list, handle_agent_group_get};
pub(crate) use git_routes::{handle_git_log, handle_git_tags, handle_git_verify, handle_git_restore};
pub(crate) use workspace::{handle_workspace_tree, handle_workspace_file_get, handle_workspace_file_put, handle_seeds_list, handle_seed_get, handle_seed_evolution, handle_skills_list, handle_skill_get, handle_skill_create, handle_skill_delete, handle_memory_list, handle_memory_get, handle_memory_create, handle_memory_search};

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

fn default_page() -> usize { 1 }
fn default_limit() -> usize { 50 }

/// Apply pagination to a slice of items.
/// Returns a JSON value with `{items, total, page, limit}`.
pub fn paginate<T: Clone + serde::Serialize>(items: &[T], params: &PageParams) -> serde_json::Value {
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
        .route("/dioxus", get(|| async { axum::response::Redirect::permanent("/dioxus/") }));

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
        .route("/api/workspace/file/{*path}", get(handle_workspace_file_get))
        .route("/api/workspace/file/{*path}", put(handle_workspace_file_put))
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
        .route("/api/memory/{name}", get(handle_memory_get))
        // Gardens
        .route("/api/gardens", get(handle_gardens_list))
        .route("/api/gardens", post(handle_garden_create))
        .route("/api/gardens/{name}/start", post(handle_garden_start))
        .route("/api/gardens/{name}/stop", post(handle_garden_stop))
        .route("/api/gardens/{name}", delete(handle_garden_remove))
        .route("/api/gardens/{name}/exec", post(handle_garden_exec))
        // Scheduler stats & tasks
        .route("/api/scheduler/stats", get(handle_scheduler_stats))
        .route("/api/scheduler/tasks", get(handle_scheduler_tasks))
        // Audit log & permissions
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
        .route("/api/programs/{name}/host-requirements", get(handle_program_host_requirements))
        // Host tools
        .route("/api/host-tools", get(handle_host_tools_check))
        // Agent Groups
        .route("/api/agent-groups", get(handle_agent_groups_list))
        .route("/api/agent-groups/{id}", get(handle_agent_group_get))
        // Container Tools
        .route("/api/containers/{name}/tools", get(handle_container_tools))
        // Container Create
        .route("/api/containers", post(handle_container_create))
        // Toolchains
        .route("/api/toolchains", get(handle_toolchains_list))
        // Events
        .route("/api/events", get(handle_events))
        // Personas (delegated to persona_routes)
        .route("/api/personas", get(persona_routes::handle_personas_list))
        .route("/api/personas", post(persona_routes::handle_persona_create))
        .route("/api/personas/{id}", get(persona_routes::handle_persona_get))
        .route("/api/personas/{id}", put(persona_routes::handle_persona_update))
        .route("/api/personas/{id}", delete(persona_routes::handle_persona_delete))
        .route("/api/personas/active", get(persona_routes::handle_persona_active_get))
        .route("/api/personas/active", put(persona_routes::handle_persona_active_set))
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
        .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth))
        .layer(axum::middleware::from_fn_with_state(state.clone().rate_limiter.clone(), rate_limit_layer))
        .layer(axum::extract::DefaultBodyLimit::max(API_BODY_LIMIT))
        .with_state(state.clone());

    public.merge(api).with_state(state)
}

/// Body size limit for API requests (10 MB).
const API_BODY_LIMIT: usize = 10 * 1024 * 1024;