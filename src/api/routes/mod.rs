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
mod calendar_routes;
mod chat;
mod cron_jobs;
mod email_routes;
mod engine_routes;
mod events;
mod git_routes;
mod infra;
mod knowledge_routes;
mod marketplace;
mod mount_routes;
mod project_routes;
mod resource_routes;
mod secrets_routes;
mod system;
mod tools;
mod workspace;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use serde::Deserialize;

use crate::api::middleware::{rate_limit_layer, require_auth, require_ready};
use crate::api::persona_routes;
use crate::api::server::AppState;

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
pub(crate) use calendar_routes::{
    handle_calendar_event_create, handle_calendar_event_delete, handle_calendar_event_get,
    handle_calendar_event_update, handle_calendar_events, handle_calendar_freebusy,
    handle_calendar_search,
};
pub(crate) use chat::{
    handle_ask_user_respond, handle_chat, handle_chat_stream, handle_chat_ticket,
    handle_knowledge_saves, handle_remove_knowledge_save, handle_save_to_knowledge,
    handle_session_tool_calls, handle_tool_approval_respond,
};
pub(crate) use cron_jobs::{
    handle_cron_job_create, handle_cron_job_delete, handle_cron_job_get, handle_cron_job_trigger,
    handle_cron_jobs_list, update_cron_job,
};
pub(crate) use email_routes::{
    handle_email_history, handle_email_history_detail, handle_email_setup, handle_email_status,
    handle_email_template_get, handle_email_templates, handle_email_test,
};
pub(crate) use engine_routes::{
    handle_engine_config, handle_engine_models, handle_engine_providers,
    handle_engine_routing_fallbacks, handle_engine_routing_stats, handle_engine_set_api_key,
    handle_engine_set_model, handle_engine_set_provider_options, handle_engine_set_routing,
    handle_engine_validate_key,
};
pub(crate) use events::{
    handle_approval_approve, handle_approval_reject, handle_approvals_list, handle_events,
    handle_session_delete, handle_session_get, handle_session_move, handle_sessions_list,
    handle_sessions_prune,
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
    handle_knowledge_file_or_sub, handle_knowledge_graph, handle_knowledge_habits,
    handle_knowledge_habits_last_week, handle_knowledge_journal_add,
    handle_knowledge_journal_emoji, handle_knowledge_journal_today, handle_knowledge_search,
    handle_knowledge_stats_done_today, handle_knowledge_stats_today, handle_knowledge_tree,
    handle_knowledge_worker_nightly, handle_knowledge_worker_scheduled,
};
pub(crate) use marketplace::{
    handle_marketplace_install, handle_marketplace_search, handle_marketplace_skill_detail,
    handle_marketplace_updates, handle_skills_sh_install, handle_skills_sh_list,
    handle_skills_sh_search, handle_skills_sh_skill_audit, handle_skills_sh_skill_detail,
};
pub(crate) use mount_routes::{
    handle_mount_create, handle_mount_delete, handle_mount_get, handle_mount_rescan,
    handle_mount_update, handle_mounts_list,
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
    handle_agent_get, handle_agent_kill, handle_agent_logs, handle_agent_stats, handle_agent_trace,
    handle_agents_list, handle_audit_verify_api, handle_backup, handle_config_get,
    handle_config_patch, handle_config_put, handle_doctor, handle_health, handle_log,
    handle_readiness, handle_status, handle_update_changelog, handle_update_check,
    handle_update_run,
};
pub(crate) use tools::handle_tools_registry;
pub(crate) use workspace::{
    MemoryMapCache, handle_dream_reports, handle_dream_status, handle_memory_create,
    handle_memory_delete, handle_memory_get, handle_memory_list, handle_memory_map,
    handle_memory_pin, handle_memory_search, handle_memory_semantic_search, handle_memory_stats,
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
        )
        // Marketplace (Skills.sh) — read-only routes, public
        .route(
            "/api/marketplace/skills-sh/search",
            get(handle_skills_sh_search),
        )
        .route(
            "/api/marketplace/skills-sh/list",
            get(handle_skills_sh_list),
        )
        .route(
            "/api/marketplace/skills-sh/skill/{id}",
            get(handle_skills_sh_skill_detail),
        )
        .route(
            "/api/marketplace/skills-sh/skill/{id}/audit",
            get(handle_skills_sh_skill_audit),
        );

    // Protected API routes (auth middleware applied)
    let api = Router::new()
        // Chat
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/ticket", post(handle_chat_ticket))
        .route("/api/chat/stream", get(handle_chat_stream))
        // RFC-017: runtime tool capability escalation
        .route(
            "/api/chat/tool-approval/{id}/respond",
            post(handle_tool_approval_respond),
        )
        // RFC-027: ask_user agent-driven clarification
        .route(
            "/api/chat/ask-user/{id}/respond",
            post(handle_ask_user_respond),
        )
        // RFC-016: Knowledge persistence API
        .route(
            "/api/chat/{session_id}/knowledge-saves",
            get(handle_knowledge_saves),
        )
        .route(
            "/api/chat/{session_id}/messages/{message_index}/save-to-knowledge",
            post(handle_save_to_knowledge),
        )
        .route(
            "/api/chat/{session_id}/messages/{message_index}/knowledge-save",
            delete(handle_remove_knowledge_save),
        )
        // Control
        .route("/api/status", get(handle_status))
        .route("/api/agents", get(handle_agents_list))
        .route("/api/agents/stats", get(handle_agent_stats))
        .route("/api/agents/{id}", get(handle_agent_get))
        .route("/api/agents/{id}/trace", get(handle_agent_trace))
        .route("/api/agents/{id}/logs", get(handle_agent_logs))
        .route("/api/agents/{id}/kill", post(handle_agent_kill))
        // Config
        .route("/api/config", get(handle_config_get))
        .route("/api/config", put(handle_config_put))
        .route("/api/config", patch(handle_config_patch))
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
        // Secrets management (RFC-028 SP-2b)
        .route("/api/secrets", get(secrets_routes::handle_secrets_list))
        .route(
            "/api/secrets/{key}",
            put(secrets_routes::handle_secret_set).delete(secrets_routes::handle_secret_delete),
        )
        .route(
            "/api/secrets/{key}/source",
            get(secrets_routes::handle_secret_source),
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
        .route("/api/memory/map", get(handle_memory_map))
        .route("/api/memory/stats", get(handle_memory_stats))
        .route("/api/memory/dream/status", get(handle_dream_status))
        .route("/api/memory/dream/reports", get(handle_dream_reports))
        .route("/api/memory/{id}/pin", put(handle_memory_pin))
        .route(
            "/api/memory/{name}",
            get(handle_memory_get).delete(handle_memory_delete),
        )
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
        .route("/api/sessions/{id}/project", patch(handle_session_move))
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
        // Calendar
        .route(
            "/api/calendar/events",
            get(handle_calendar_events).post(handle_calendar_event_create),
        )
        .route(
            "/api/calendar/events/{uid}",
            get(handle_calendar_event_get)
                .put(handle_calendar_event_update)
                .delete(handle_calendar_event_delete),
        )
        .route("/api/calendar/search", get(handle_calendar_search))
        .route("/api/calendar/freebusy", get(handle_calendar_freebusy))
        // Email
        .route("/api/email/status", get(handle_email_status))
        .route("/api/email/history", get(handle_email_history))
        .route("/api/email/history/{id}", get(handle_email_history_detail))
        .route("/api/email/templates", get(handle_email_templates))
        .route(
            "/api/email/templates/{name}",
            get(handle_email_template_get),
        )
        .route("/api/email/test", post(handle_email_test))
        .route("/api/email/setup", post(handle_email_setup))
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
        // Mounts (RFC-025)
        .route("/api/mounts", get(handle_mounts_list))
        .route("/api/mounts", post(handle_mount_create))
        .route("/api/mounts/{id}", get(handle_mount_get))
        .route("/api/mounts/{id}", put(handle_mount_update))
        .route("/api/mounts/{id}", delete(handle_mount_delete))
        .route("/api/mounts/{id}/rescan", post(handle_mount_rescan))
        // Tool Registry (for settings UI)
        .route("/api/tools/registry", get(handle_tools_registry))
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
        // Knowledge — file CRUD + git sub-paths unified under one catch-all.
        // axum 0.8: `{*path}` MUST be the last segment, so we dispatch on method/path
        // in a single handler rather than registering separate sub-path routes.
        .route(
            "/api/knowledge/file/{*path}",
            get(handle_knowledge_file_or_sub),
        )
        .route(
            "/api/knowledge/file/{*path}",
            put(handle_knowledge_file_or_sub),
        )
        .route(
            "/api/knowledge/file/{*path}",
            delete(handle_knowledge_file_or_sub),
        )
        .route(
            "/api/knowledge/file/{*path}",
            post(handle_knowledge_file_or_sub),
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
        // Marketplace (ClawHub) — install requires auth
        .route(
            "/api/marketplace/skills/{slug}/install",
            post(handle_marketplace_install),
        )
        // Marketplace (Skills.sh) — install requires auth
        .route(
            "/api/marketplace/skills-sh/skill/{id}/install",
            post(handle_skills_sh_install),
        )
        // System Update
        .route("/api/update/check", get(handle_update_check))
        .route("/api/update/changelog", get(handle_update_changelog))
        .route("/api/update/run", post(handle_update_run))
        // System Tools
        .route("/api/system/doctor", post(handle_doctor))
        .route("/api/system/audit-verify", post(handle_audit_verify_api))
        .route("/api/system/backup", post(handle_backup))
        .route("/api/system/log", get(handle_log))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_ready,
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
