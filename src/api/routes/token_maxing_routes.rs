//! Token-maxing API routes (RFC-031 §9).
//!
//! Control surface for the autonomous drain loop: start/stop/status, session
//! history + report, and live provider eligibility. Backed by the shared
//! [`oxios_kernel::TokenMaxingApi`] on the cached KernelHandle.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::api::error::AppError;
use crate::api::server::AppState;
use oxios_kernel::{MaxingStart, MaxingWindow, TokenMaxingApi};

/// `POST /api/token-maxing/start` body.
#[derive(Debug, Deserialize)]
pub struct StartRequest {
    /// A scheduled window. Omit (or set `manual`) for a manual run.
    pub window: Option<WindowRequest>,
    /// Force a manual run (window ignored). Defaults false.
    #[serde(default)]
    pub manual: bool,
}

#[derive(Debug, Deserialize)]
pub struct WindowRequest {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Resolve the token-maxing facade, or 503 if the subsystem is absent.
fn tm(state: &State<Arc<AppState>>) -> Result<&TokenMaxingApi, AppError> {
    state
        .kernel
        .token_maxing
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("token-maxing subsystem unavailable".into()))
}

/// POST /api/token-maxing/start — launch a session (window or manual).
pub(crate) async fn handle_token_maxing_start(
    state: State<Arc<AppState>>,
    Json(req): Json<StartRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = tm(&state)?;
    if !api.enabled() {
        return Err(AppError::BadRequest(
            "token-maxing is disabled or has no eligible subscription provider".into(),
        ));
    }
    let start = if req.manual {
        MaxingStart::Manual
    } else {
        match req.window {
            Some(w) => MaxingStart::Scheduled(MaxingWindow { start: w.start, end: w.end }),
            None => MaxingStart::Manual,
        }
    };
    let id = api
        .launch(start)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "session_id": id })))
}

/// POST /api/token-maxing/stop — graceful stop after the in-flight task.
pub(crate) async fn handle_token_maxing_stop(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = tm(&state)?;
    api.stop();
    Ok(Json(serde_json::json!({ "stopped": true })))
}

/// GET /api/token-maxing/status — live session state + per-provider verdicts.
pub(crate) async fn handle_token_maxing_status(
    state: State<Arc<AppState>>,
) -> Result<Json<oxios_kernel::MaxerStatus>, AppError> {
    let api = tm(&state)?;
    Ok(Json(api.status()))
}

/// GET /api/token-maxing/sessions — completed session history.
pub(crate) async fn handle_token_maxing_sessions(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<oxios_kernel::TokenMaxingSession>>, AppError> {
    let api = tm(&state)?;
    Ok(Json(api.sessions()))
}

/// GET /api/token-maxing/sessions/{id} — one session report.
pub(crate) async fn handle_token_maxing_session(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<oxios_kernel::TokenMaxingSession>, AppError> {
    let api = tm(&state)?;
    api.session(&id)
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("session {id} not found")))
}

/// GET /api/token-maxing/providers — eligibility + live availability.
pub(crate) async fn handle_token_maxing_providers(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = tm(&state)?;
    Ok(Json(serde_json::json!({
        "enabled": api.enabled(),
        "providers": api.snapshots(),
        "recalibrations": api.recalibration_history(),
        "cooldowns": api.cooldown_history(),
    })))
}
