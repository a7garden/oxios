//! API routes for cron job management.
//!
//! Provides endpoints for creating, listing, updating, deleting,
//! and manually triggering scheduled cron jobs.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use oxios_gateway::message::IncomingMessage;
use oxios_kernel::{CronJob, Priority};

use crate::error::AppError;
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Request body for creating a new cron job.
#[derive(Debug, Deserialize)]
pub struct CreateCronJobRequest {
    /// Human-readable job name.
    pub name: String,
    /// Cron expression (e.g., "0 0 * * * *").
    pub schedule: String,
    /// The goal/prompt to execute.
    pub goal: String,
    /// Optional constraints.
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Optional acceptance criteria.
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    /// Toolchain to use (default: "default").
    #[serde(default = "default_toolchain")]
    pub toolchain: String,
    /// Priority for the spawned agent.
    #[serde(default)]
    pub priority: Priority,
}

fn default_toolchain() -> String {
    "default".into()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/cron-jobs — List all cron jobs.
pub(crate) async fn handle_cron_jobs_list(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let jobs = state.kernel.infra.list_crons();
    Ok(Json(serde_json::json!({ "jobs": jobs })))
}

/// POST /api/cron-jobs — Create a new cron job.
pub(crate) async fn handle_cron_job_create(
    state: State<Arc<AppState>>,
    Json(body): Json<CreateCronJobRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut job = CronJob::new(body.name, body.schedule, body.goal);
    job.constraints = body.constraints;
    job.acceptance_criteria = body.acceptance_criteria;
    job.toolchain = body.toolchain;
    job.priority = body.priority;

    let id = state.kernel.infra.add_cron(job).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "id": id })))
}

/// GET /api/cron-jobs/:id — Get a specific cron job.
pub(crate) async fn handle_cron_job_get(
    state: State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<CronJob>, AppError> {
    state.kernel.infra.get_cron(id)
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("Cron job {} not found", id)))
}

/// DELETE /api/cron-jobs/:id — Delete a cron job.
pub(crate) async fn handle_cron_job_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.kernel.unschedule(&id.to_string()).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": id })))
}

/// PATCH /api/cron-jobs/:id — Update a cron job.
pub(crate) async fn update_cron_job(
    state: State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let update: oxios_kernel::CronJobUpdate = serde_json::from_value(body)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    state.kernel.infra.update_cron(id, update).await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "updated": id })))
}

/// POST /api/cron-jobs/:id/trigger — Manually trigger a cron job.
pub(crate) async fn handle_cron_job_trigger(
    state: State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let job = state.kernel.infra.trigger_cron(id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(job_id = %id, job_name = %job.name, "Triggering cron job");

    // Send the job goal through the gateway channel and wait for response.
    let mut msg = IncomingMessage::new("cron", "system", &job.goal);
    msg.metadata
        .insert("toolchain".to_string(), job.toolchain.clone());

    let response = state
        .channel
        .send_and_wait(msg)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let success = response
        .metadata
        .get("evaluation_passed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(false);

    let summary = response
        .metadata
        .get("output")
        .cloned()
        .unwrap_or_else(|| response.content.clone());

    // Record the result on the job.
    state.kernel.infra.complete_cron(id, success, summary.clone()).await;

    Ok(Json(serde_json::json!({
        "job_id": id,
        "success": success,
        "summary": summary,
    })))
}