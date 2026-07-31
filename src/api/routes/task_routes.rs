//! API routes for task management (RFC-043).
//!
//! CRUD + scheduling + verify + comments for the task lifecycle.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use oxios_kernel::task::{
    CreateTaskParams, ListTasksParams, SetScheduleParams, SetVerifyParams, TaskRunTrigger,
    TaskStatus,
};

/// Ceiling for a synchronous manual task run (`POST /api/tasks/:id/run`).
/// Longer-running work belongs on a schedule (cron/heartbeat), whose jobs use
/// the CronScheduler's longer `job_timeout_secs`.
const TASK_RUN_TIMEOUT: u64 = 300;

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
    // Persist automation fields + compute next_run + set status, all in one
    // store call (replaces the previous buggy handler that only set
    // next_run_at and dropped the automation fields).
    let result = {
        let store = state.task_store.lock().await;
        store.set_automation(&id, params).await
    };
    match result {
        Ok(()) => {
            // Re-read to return the computed next_run/status.
            let task = {
                let store = state.task_store.lock().await;
                store.get_task_by_id(&id).await
            };
            match task {
                Ok(t) => Ok(Json(serde_json::json!({
                    "id": t.id,
                    "automation_mode": t.automation_mode,
                    "schedule_pattern": t.schedule_pattern,
                    "schedule_timezone": t.schedule_timezone,
                    "heartbeat_interval_secs": t.heartbeat_interval_secs,
                    "max_executions": t.max_executions,
                    "next_run_at": t.next_run_at,
                    "status": t.status,
                }))),
                Err(e) => Err(AppError::Internal(format!(
                    "Schedule set, reload failed: {e}"
                ))),
            }
        }
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

/// POST /api/tasks/:id/run — trigger manual (synchronous) execution.
///
/// Executes the task's `instruction` through the shared `run_goal` primitive
/// (direct orchestrator path), bounded by `TASK_RUN_TIMEOUT` so a hung agent
/// can't hold the HTTP connection forever. Records the run in `task_runs`
/// and updates the task lifecycle (status, execution_count, etc.).
pub(crate) async fn handle_task_run(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. Load + validate the task exists and isn't already running.
    let task = {
        let store = state.task_store.lock().await;
        store.get_task_by_id(&id).await
    }
    .map_err(|e| AppError::NotFound(format!("Task not found: {id} ({e})")))?;
    if task.status == TaskStatus::Running {
        return Err(AppError::Conflict(format!(
            "Task '{id}' is already running"
        )));
    }

    // Execute + record the full lifecycle via the shared helper (also used by
    // the auto-run tick loop). Bounded by TASK_RUN_TIMEOUT for the HTTP path.
    let (run_id, success, summary) = execute_task_run(
        state.task_store.clone(),
        state.kernel.clone(),
        &id,
        &task.instruction,
        TaskRunTrigger::Manual,
        TASK_RUN_TIMEOUT,
    )
    .await;

    Ok(Json(serde_json::json!({
        "id": id,
        "run_id": run_id,
        "success": success,
        "summary": summary,
    })))
}

// ── Run history ───────────────────────────────────────────────────

/// GET /api/tasks/:id/runs — execution history (newest first).
pub(crate) async fn handle_task_runs(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let runs = {
        let store = state.task_store.lock().await;
        store
            .list_runs(&id)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to list runs: {e}")))?
    };
    Ok(Json(
        serde_json::json!({ "runs": runs, "count": runs.len() }),
    ))
}

/// Shared task execution: mark running → run goal (bounded timeout) → finalize.
///
/// The single execution path used by both the manual run endpoint
/// ([`handle_task_run`]) and the auto-run tick loop (`spawn_task_auto_run` in
/// `plugin.rs`). Returns `(run_id, success, summary)`; on a `mark_running`
/// failure the run_id is empty.
///
/// Success signal: no provider failure AND evaluation passed (default true —
/// a goal with no acceptance criteria that ran cleanly is a success). This is
/// the opposite of the metrics code's `unwrap_or(false)`, which is a latent
/// bug there and must NOT be copied here.
pub(crate) async fn execute_task_run(
    task_store: Arc<tokio::sync::Mutex<oxios_kernel::task::TaskStore>>,
    kernel: Arc<oxios_kernel::KernelHandle>,
    id: &str,
    instruction: &str,
    trigger: oxios_kernel::task::TaskRunTrigger,
    timeout_secs: u64,
) -> (String, bool, String) {
    // 1. Mark running + open a task_runs row.
    let run_id = match task_store.lock().await.mark_running(id, trigger).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(%id, error = %e, "mark_running failed");
            return (String::new(), false, format!("Failed to start run: {e}"));
        }
    };

    // 2. Execute with a bounded timeout (a hung agent can't run forever).
    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        kernel.run_goal(instruction, None),
    )
    .await;

    let (success, summary, error) = match result {
        Ok(Ok(r)) => {
            let success = r.failure_class.is_none() && r.evaluation_passed.unwrap_or(true);
            let summary = r.output.clone().unwrap_or_else(|| r.response.clone());
            (success, summary, None)
        }
        Ok(Err(e)) => {
            tracing::error!(%id, error = %e, "task run failed");
            (false, String::new(), Some(e.to_string()))
        }
        Err(_) => {
            tracing::error!(%id, timeout = timeout_secs, "task run timed out");
            (
                false,
                String::new(),
                Some(format!("Timed out after {timeout_secs}s")),
            )
        }
    };

    // 3. Finalize: task_runs row + task terminal state.
    if let Err(e) = task_store
        .lock()
        .await
        .mark_finished(id, &run_id, success, summary.clone(), error)
        .await
    {
        tracing::error!(%id, error = %e, "mark_finished failed");
    }

    (run_id, success, summary)
}
