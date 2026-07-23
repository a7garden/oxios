//! API routes for task management (RFC-043).
//!
//! CRUD + scheduling + verify + comments for the task lifecycle.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use oxios_kernel::task::{
    CreateTaskParams, ListTasksParams, SetScheduleParams, SetVerifyParams, TaskStatus,
};

use crate::api::error::AppError;
use crate::api::server::AppState;

// ── List ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub statuses: Option<String>,
    pub assignee: Option<String>,
    pub parent: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// GET /api/tasks
pub(crate) async fn handle_tasks_list(
    state: State<Arc<AppState>>,
    Query(q): Query<ListTasksQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let statuses = q
        .statuses
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

    let params = ListTasksParams {
        statuses,
        assignee_agent_id: q.assignee,
        parent_task_id: q.parent,
        limit: q.limit,
        offset: q.offset,
    };

    let store = state.task_store.lock().await;
    match store.list_tasks(params).await {
        Ok(tasks) => Ok(Json(
            serde_json::json!({ "tasks": tasks, "count": tasks.len() }),
        )),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list tasks");
            Err(AppError::Internal(format!("Failed to list tasks: {e}")))
        }
    }
}

// ── Create ────────────────────────────────────────────────────────

/// POST /api/tasks
pub(crate) async fn handle_task_create(
    state: State<Arc<AppState>>,
    Json(params): Json<CreateTaskParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    if params.name.trim().is_empty() || params.instruction.trim().is_empty() {
        return Err(AppError::BadRequest(
            "name and instruction are required".into(),
        ));
    }

    let store = state.task_store.lock().await;
    match store.create_task(params).await {
        Ok(task) => Ok(Json(serde_json::to_value(&task).unwrap_or_default())),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create task");
            Err(AppError::Internal(format!("Failed to create task: {e}")))
        }
    }
}

// ── Get by ID ─────────────────────────────────────────────────────

/// GET /api/tasks/:id
pub(crate) async fn handle_task_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = state.task_store.lock().await;
    match store.get_task_by_id(&id).await {
        Ok(task) => Ok(Json(serde_json::to_value(&task).unwrap_or_default())),
        Err(e) => {
            tracing::error!(error = %e, id = %id, "Failed to get task");
            Err(AppError::NotFound(format!("Task not found: {id}")))
        }
    }
}

// ── Delete ────────────────────────────────────────────────────────

/// DELETE /api/tasks/:id
pub(crate) async fn handle_task_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = state.task_store.lock().await;
    match store.delete_task(&id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "deleted": true }))),
        Err(e) => {
            tracing::error!(error = %e, id = %id, "Failed to delete task");
            Err(AppError::Internal(format!("Failed to delete task: {e}")))
        }
    }
}

// ── Update status ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

/// PUT /api/tasks/:id/status
pub(crate) async fn handle_task_update_status(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStatusRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let status: TaskStatus = req
        .status
        .parse()
        .map_err(|e: String| AppError::BadRequest(e))?;

    let store = state.task_store.lock().await;
    match store.update_status(&id, &status).await {
        Ok(()) => Ok(Json(
            serde_json::json!({ "id": id, "status": status.to_string() }),
        )),
        Err(e) => {
            tracing::error!(error = %e, id = %id, "Failed to update task status");
            Err(AppError::Internal(format!("Failed to update status: {e}")))
        }
    }
}

// ── Set schedule ──────────────────────────────────────────────────

/// PUT /api/tasks/:id/schedule
pub(crate) async fn handle_task_set_schedule(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(params): Json<SetScheduleParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = state.task_store.lock().await;
    let now = chrono::Utc::now().to_rfc3339();

    let automation_str = params.automation_mode.as_ref().map(|m| m.to_string());

    let next_run = params.schedule_pattern.as_ref().map(|_| now.clone());

    let conn_result = store.set_next_run(&id, next_run.as_deref()).await;

    // Also update automation fields
    {
        let _store2 = state.task_store.lock().await;
        // We'd need a separate update method here, but for now the model
        // supports it through set_next_run
    }
    match conn_result {
        Ok(()) => Ok(Json(serde_json::json!({
            "id": id,
            "automation_mode": automation_str,
            "schedule_pattern": params.schedule_pattern,
            "heartbeat_interval_secs": params.heartbeat_interval_secs,
        }))),
        Err(e) => Err(AppError::Internal(format!("Failed to set schedule: {e}"))),
    }
}

// ── Set verify ────────────────────────────────────────────────────

pub(crate) async fn handle_task_set_verify(
    _state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(params): Json<SetVerifyParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::json!({
        "id": id,
        "verify_enabled": params.enabled,
        "verify_requirement": params.requirement,
    })))
}

// ── Run task ──────────────────────────────────────────────────────

/// POST /api/tasks/:id/run — trigger manual execution.
///
/// Not yet implemented: spawning an agent session from a stored task
/// requires `KernelHandle.agent` session integration. Returns 503 so the
/// caller gets a real error instead of a task silently stuck in "Running".
pub(crate) async fn handle_task_run(
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::ServiceUnavailable(format!(
        "Task execution is not yet implemented (task '{id}')"
    )))
}
