//! Token-maxing API routes (RFC-031 §9).
//!
//! Control surface for the autonomous drain loop: start/stop/status, session
//! history + report, and live provider eligibility. Backed by the shared
//! [`oxios_kernel::TokenMaxingApi`] on the cached KernelHandle.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
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
            Some(w) => MaxingStart::Scheduled(MaxingWindow {
                start: w.start,
                end: w.end,
            }),
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
///
/// The `providers` array is a richer per-provider DTO built at the
/// route layer. Each entry adds a `billing_model` string derived
/// from the **live quota snapshot** (RFC-031 v2), not the v1
/// `[[token-maxing.providers]]` config:
///
/// - `"subscription"` — the live `QuotaSnapshot` returned
///   `plan_type = Subscription` (e.g. ZAI Coding Plan with
///   `TOKENS_LIMIT`).
/// - `"metered"` — the live response had no `TOKENS_LIMIT`
///   window (pay-per-token key).
/// - `"unknown"` — no live snapshot has been received yet (fetcher
///   hasn't run, or returned an error).
///
/// The `provider` array is also widened to include
/// auto-discovered providers that have a live snapshot but no
/// `[[token-maxing.providers]]` entry.
pub(crate) async fn handle_token_maxing_providers(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = tm(&state)?;
    let snapshots = api.snapshots();
    let providers: Vec<serde_json::Value> = snapshots
        .into_iter()
        .map(|s| {
            // v2: source from the live snapshot's plan_type, not
            // the v1 config block. A subscription key auto-
            // discovered via live fetch (zai Coding Plan) returns
            // "subscription" even with no `[[token-maxing.providers]]`
            // entry.
            let billing_model = match api
                .tracker()
                .live_snapshot(&s.provider)
                .map(|snap| snap.plan_type)
            {
                Some(oxios_kernel::token_maxing::live_quota::PlanType::Subscription) => {
                    "subscription"
                }
                Some(oxios_kernel::token_maxing::live_quota::PlanType::Metered) => "metered",
                // PlanType::Unknown or no live snapshot at all
                _ => "unknown",
            };
            serde_json::json!({
                "provider": s.provider,
                "availability": s.availability,
                "billing_model": billing_model,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "enabled": api.enabled(),
        "providers": providers,
        "recalibrations": api.recalibration_history(),
        "cooldowns": api.cooldown_history(),
    })))
}
