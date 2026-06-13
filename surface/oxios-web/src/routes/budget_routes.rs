//! Budget management API routes.

use crate::error::AppError;
use crate::routes::PageParams;
use crate::routes::paginate;
use crate::server::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use oxios_kernel::budget::BudgetLimit;
use oxios_kernel::types::AgentId;
use serde::Deserialize;
use serde_json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SetBudgetRequest {
    pub token_budget: u64,
    pub calls_budget: u64,
    pub window_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReserveRequest {
    pub tokens: u64,
}

fn parse_agent_id(id: &str) -> Result<AgentId, AppError> {
    AgentId::parse_str(id).map_err(|e| AppError::Internal(format!("Invalid agent ID: {e}")))
}

/// GET /api/budget — List all agent budgets with full info.
pub(crate) async fn handle_budget_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state
        .kernel
        .agents
        .list()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let all_budgets = state.kernel.agents.all_budget_info();

    let mut total_tokens_used = 0u64;
    let mut total_tokens_limit = 0u64;
    let mut exhausted_count = 0usize;

    let items: Vec<serde_json::Value> = all_budgets
        .into_iter()
        .map(|b| {
            total_tokens_used += b.tokens_used;
            total_tokens_limit += b.token_limit;
            if b.is_exhausted {
                exhausted_count += 1;
            }

            let agent_name = agents
                .iter()
                .find(|a| a.id == b.agent_id)
                .map(|a| a.name.clone())
                .unwrap_or_default();

            serde_json::json!({
                "agent_id": b.agent_id.to_string(),
                "name": agent_name,
                "budget": {
                    "token_limit": b.token_limit,
                    "tokens_used": b.tokens_used,
                    "tokens_remaining": b.tokens_remaining,
                    "calls_limit": b.calls_limit,
                    "calls_used": b.calls_used,
                    "calls_remaining": b.calls_remaining,
                    "window_secs": b.window_secs,
                    "window_remaining_secs": b.window_remaining_secs,
                    "is_exhausted": b.is_exhausted,
                }
            })
        })
        .collect();

    let summary = serde_json::json!({
        "total_agents": items.len(),
        "total_tokens_used": total_tokens_used,
        "total_tokens_limit": total_tokens_limit,
        "exhausted_agents": exhausted_count,
    });

    let paginated = paginate(&items, &params);
    let mut response = serde_json::json!({});
    response["agents"] = paginated["items"].clone();
    response["summary"] = summary;
    if let Some(p) = paginated.get("total") {
        response["total"] = p.clone();
    }
    if let Some(p) = paginated.get("page") {
        response["page"] = p.clone();
    }
    if let Some(p) = paginated.get("limit") {
        response["limit"] = p.clone();
    }

    Ok(Json(response))
}

/// GET /api/budget/{agent_id}
pub(crate) async fn handle_budget_get(
    state: State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let aid = parse_agent_id(&agent_id)?;
    let info = state.kernel.agents.full_budget_info(&aid);

    match info {
        Some(b) => Ok(Json(serde_json::json!({
            "agent_id": agent_id,
            "budget": {
                "token_limit": b.token_limit,
                "tokens_used": b.tokens_used,
                "tokens_remaining": b.tokens_remaining,
                "calls_limit": b.calls_limit,
                "calls_used": b.calls_used,
                "calls_remaining": b.calls_remaining,
                "window_secs": b.window_secs,
                "window_remaining_secs": b.window_remaining_secs,
                "is_exhausted": b.is_exhausted,
            }
        }))),
        None => Err(AppError::NotFound(format!(
            "No budget configured for agent '{agent_id}'"
        ))),
    }
}

/// POST /api/budget/{agent_id}
pub(crate) async fn handle_budget_set(
    state: State<Arc<AppState>>,
    Path(agent_id): Path<String>,
    Json(body): Json<SetBudgetRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let aid = parse_agent_id(&agent_id)?;
    state.kernel.agents.set_budget(BudgetLimit {
        agent_id: aid,
        token_budget: body.token_budget,
        calls_budget: body.calls_budget,
        window_secs: body.window_secs,
    });
    Ok(Json(
        serde_json::json!({ "set": true, "agent_id": agent_id }),
    ))
}

/// DELETE /api/budget/{agent_id}
pub(crate) async fn handle_budget_remove(
    state: State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let aid = parse_agent_id(&agent_id)?;
    state.kernel.agents.remove_budget(&aid);
    Ok(Json(
        serde_json::json!({ "removed": true, "agent_id": agent_id }),
    ))
}

/// POST /api/budget/{agent_id}/reserve
pub(crate) async fn handle_budget_reserve(
    state: State<Arc<AppState>>,
    Path(agent_id): Path<String>,
    Json(body): Json<ReserveRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let aid = parse_agent_id(&agent_id)?;
    state
        .kernel
        .agents
        .reserve_budget(&aid, body.tokens)
        .map_err(|e| AppError::Internal(format!("Budget exceeded: {e}")))?;
    Ok(Json(serde_json::json!({ "reserved": true })))
}

/// POST /api/budget/{agent_id}/reset
pub(crate) async fn handle_budget_reset(
    state: State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let aid = parse_agent_id(&agent_id)?;
    state.kernel.agents.reset_budget(&aid);
    Ok(Json(
        serde_json::json!({ "reset": true, "agent_id": agent_id }),
    ))
}
