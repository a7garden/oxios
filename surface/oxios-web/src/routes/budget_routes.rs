//! Budget management API routes.

use crate::error::AppError;
use crate::routes::paginate;
use crate::routes::PageParams;
use crate::server::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
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

/// GET /api/budget — List all agent budgets.
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
    let items: Vec<serde_json::Value> = agents
        .iter()
        .filter_map(|a| {
            let info = state.kernel.agents.check_budget(&a.id);
            // Only include agents that have budget limits configured
            if info.tokens_remaining > 0 || info.calls_remaining > 0 {
                Some(serde_json::json!({
                    "agent_id": a.id.to_string(),
                    "name": a.name,
                    "tokens_remaining": info.tokens_remaining,
                    "calls_remaining": info.calls_remaining,
                    "window_remaining_secs": info.window_remaining_secs,
                    "is_exhausted": info.is_exhausted,
                }))
            } else {
                None
            }
        })
        .collect();
    Ok(Json(paginate(&items, &params)))
}

/// GET /api/budget/{agent_id}
pub(crate) async fn handle_budget_get(
    state: State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let aid = parse_agent_id(&agent_id)?;
    let info = state.kernel.agents.check_budget(&aid);
    Ok(Json(serde_json::json!({
        "agent_id": agent_id,
        "tokens_remaining": info.tokens_remaining,
        "calls_remaining": info.calls_remaining,
        "window_remaining_secs": info.window_remaining_secs,
        "is_exhausted": info.is_exhausted,
    })))
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
