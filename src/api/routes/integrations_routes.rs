//! Integrations API — `/api/integrations` (RFC-041 Phase 2).
//!
//! Lists registry entries with live detect + credential status. The credential
//! status calls the matching resolver (H6) — never pokes one env var. Static
//! `Secret` values can be set/removed; `OAuth`/provisioning land in Phase 3/4.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::api::error::AppError;
use crate::api::server::AppState;
use oxios_kernel::CredentialStatus;

/// One integration's full status row (registry + detect + credential).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationRow {
    pub id: String,
    pub label: String,
    pub cli: Option<String>,
    /// `none` | `secret` | `oauth` — drives the frontend credential UI.
    pub resolver_kind: String,
    /// `package_manager` | `cli_tool` | `credential_only` — UI grouping (S1).
    pub kind: String,
    /// `null` when the integration has no CLI to detect.
    pub detected: Option<DetectedRow>,
    pub credential: CredentialStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedRow {
    pub installed: bool,
    pub version: Option<String>,
    pub source: String,
    pub path: String,
}

/// `GET /api/integrations` — all integrations with live status.
pub(crate) async fn handle_integrations_list(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<IntegrationRow>>, AppError> {
    let mut rows = Vec::new();
    for it in state.kernel.host_tools.integrations() {
        let detected = if let Some(cli) = &it.cli {
            match state.kernel.host_tools.detect(cli).await {
                Some(t) => Some(DetectedRow {
                    installed: true,
                    version: t.version,
                    source: format!("{:?}", t.source).to_lowercase(),
                    path: t.path,
                }),
                None => Some(DetectedRow {
                    installed: false,
                    version: None,
                    source: "none".into(),
                    path: String::new(),
                }),
            }
        } else {
            None
        };
        let credential =
            state
                .kernel
                .host_tools
                .credential_status(&it.id)
                .unwrap_or(CredentialStatus {
                    configured: false,
                    source: "none".into(),
                });
        rows.push(IntegrationRow {
            id: it.id.clone(),
            label: it.label.clone(),
            cli: it.cli.clone(),
            resolver_kind: resolver_kind_label(&it.credential),
            kind: format!("{:?}", it.kind).to_lowercase(),
            detected,
            credential,
        });
    }
    Ok(Json(rows))
}

/// `GET /api/integrations/{id}/credential` — credential status for one.
pub(crate) async fn handle_integration_credential_status(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<CredentialStatus>, AppError> {
    let status = state
        .kernel
        .host_tools
        .credential_status(&id)
        .ok_or_else(|| AppError::NotFound(format!("integration '{id}' not found")))?;
    Ok(Json(status))
}

/// Response body for `POST /api/integrations/{id}/install` (RFC-041 M3).
/// The route returns this immediately; the actual install runs in a background
/// task whose progress and outcome arrive as `integration_install_*` events
/// on the existing SSE channel (`/api/events`), keyed by `job_id`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallJob {
    pub job_id: String,
    pub integration_id: String,
}

/// `POST /api/integrations/{id}/install` — start a privileged install.
///
/// Returns `{ job_id }` immediately (M3). Output streams via SSE events
/// `integration_install_started` / `_progress` / `_completed` / `_failed`,
/// all keyed by `job_id`. The background task:
/// 1. Publishes `Started` (audit subscriber records it automatically).
/// 2. Runs the first applicable install spec via the kernel's privileged op.
/// 3. On success: invalidates the scanner cache (B1 — the next `detect` call
///    sees the freshly installed binary instead of the stale 60s TTL `None`)
///    and publishes `Completed`. On failure: publishes `Failed`.
pub(crate) async fn handle_integration_install(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<InstallJob>, AppError> {
    use oxios_kernel::event_bus::KernelEvent;
    use uuid::Uuid;

    let kernel = state.kernel.clone();
    // Validate the integration exists and has install specs up front so a
    let it = kernel
        .host_tools
        .integration(&id)
        .ok_or_else(|| AppError::NotFound(format!("integration '{id}' not found")))?;
    if it.install.is_empty() {
        return Err(AppError::BadRequest(format!(
            "integration '{id}' has no install specs"
        )));
    }

    let job_id = Uuid::new_v4().to_string();
    let label = it.label.clone();
    let integration_id = id.clone();
    let job_id_for_task = job_id.clone();
    // Build the preview command line up front — extracting it here lets us
    // drop the `&Integration` borrow on `kernel.host_tools` before the
    // spawned task moves `kernel` (the borrow checker would otherwise reject
    // the move). All kernel access inside the task re-resolves by id.
    let preview_cmd = it
        .install
        .first()
        .and_then(|s| {
            oxios_kernel::host_tools::provisioner::build_command(s)
                .map(|(bin, args)| format!("{bin} {}", args.join(" ")))
        })
        .unwrap_or_else(|| format!("install {integration_id}"));
    // Publish Started before spawn so the SSE subscriber sees a deterministic
    // ordering: Started → (Progress?) → Completed|Failed.
    let _ = kernel
        .infra
        .publish(KernelEvent::IntegrationInstallStarted {
            job_id: job_id.clone(),
            integration_id: integration_id.clone(),
            label: label.clone(),
        });

    tokio::spawn(async move {
        let _ = kernel
            .infra
            .publish(KernelEvent::IntegrationInstallProgress {
                job_id: job_id_for_task.clone(),
                integration_id: integration_id.clone(),
                line: preview_cmd,
            });

        let outcome = kernel.host_tools.install(&integration_id).await;
        match outcome {
            Ok(out) => {
                // B1: invalidate the scanner cache so the success is visible
                // immediately — without this the 60s TTL hides the freshly
                // installed binary and the UI keeps showing ✗ not-installed.
                kernel.host_tools.invalidate();
                let _ = kernel
                    .infra
                    .publish(KernelEvent::IntegrationInstallCompleted {
                        job_id: job_id_for_task,
                        integration_id,
                        command: out.command,
                        output: out.output,
                        exit_code: out.exit_code,
                    });
            }
            Err(e) => {
                let _ = kernel.infra.publish(KernelEvent::IntegrationInstallFailed {
                    job_id: job_id_for_task,
                    integration_id,
                    error: e.to_string(),
                });
            }
        }
    });

    Ok(Json(InstallJob {
        job_id,
        integration_id: id,
    }))
}

/// `POST /api/integrations/{id}/oauth/start` — begin a device-code flow.
///
/// Returns `{ handle, user_code, verification_url, expires_in }`. The
pub(crate) async fn handle_integration_oauth_start(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<oxios_kernel::host_tools::DeviceCodeResponse>, AppError> {
    let resp = state
        .kernel
        .host_tools
        .oauth_start(&id)
        .await
        .map_err(|e| AppError::Internal(format!("oauth start failed: {e}")))?;
    Ok(Json(resp))
}

/// `GET /api/integrations/{id}/oauth/poll?handle=…` — poll a device-code flow.
pub(crate) async fn handle_integration_oauth_poll(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<OAuthPollQuery>,
) -> Result<Json<oxios_kernel::host_tools::PollResponse>, AppError> {
    let _ = id; // handle is the lookup key; id is in the path for REST consistency
    let resp = state
        .kernel
        .host_tools
        .oauth_poll(&q.handle)
        .await
        .map_err(|e| AppError::BadRequest(format!("oauth poll failed: {e}")))?;
    Ok(Json(resp))
}

/// Query params for `/oauth/poll`.
#[derive(Debug, Deserialize)]
pub struct OAuthPollQuery {
    pub handle: String,
}

/// `PUT /api/integrations/{id}/credential` body — set a static `Secret` value.
#[derive(Debug, Deserialize)]
pub struct SetCredentialBody {
    pub value: String,
}

/// `PUT /api/integrations/{id}/credential` — store a `Secret`-class value.
///
/// Only valid for `resolver = "secret"` integrations (D7: providers are
/// configured via engine_api, not here). The store key comes from the
/// descriptor — callers never name it, so there is no key-confusion surface.
pub(crate) async fn handle_integration_credential_set(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SetCredentialBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    use oxios_kernel::host_tools::CredentialResolver;
    let it = state
        .kernel
        .host_tools
        .integration(&id)
        .ok_or_else(|| AppError::NotFound(format!("integration '{id}' not found")))?;
    let store_key = match &it.credential {
        CredentialResolver::Secret { store_key, .. } => store_key.clone(),
        other => {
            return Err(AppError::BadRequest(format!(
                "integration '{id}' uses {:?} resolver; only 'secret' is settable here",
                other
            )));
        }
    };
    oxios_kernel::CredentialStore::store(&store_key, &body.value)
        .map_err(|e| AppError::Internal(format!("failed to store credential: {e}")))?;
    Ok(Json(serde_json::json!({ "status": "ok", "id": id })))
}

/// `DELETE /api/integrations/{id}/credential` — remove a `Secret` value.
pub(crate) async fn handle_integration_credential_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use oxios_kernel::host_tools::CredentialResolver;
    let it = state
        .kernel
        .host_tools
        .integration(&id)
        .ok_or_else(|| AppError::NotFound(format!("integration '{id}' not found")))?;
    let store_key = match &it.credential {
        CredentialResolver::Secret { store_key, .. } => store_key.clone(),
        CredentialResolver::OAuth { store_key, .. } => {
            // Phase 3 will revoke at the provider first; for now just delete.
            store_key.clone()
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "integration '{id}' uses {:?} resolver; nothing to delete",
                other
            )));
        }
    };
    oxios_kernel::CredentialStore::delete(&store_key)
        .map_err(|e| AppError::Internal(format!("failed to delete credential: {e}")))?;
    Ok(Json(serde_json::json!({ "status": "ok", "id": id })))
}

/// Map a `CredentialResolver` to a stable frontend label.
fn resolver_kind_label(c: &oxios_kernel::host_tools::CredentialResolver) -> String {
    use oxios_kernel::host_tools::CredentialResolver;
    match c {
        CredentialResolver::None => "none",
        CredentialResolver::Secret { .. } => "secret",
        CredentialResolver::OAuth { .. } => "oauth",
    }
    .into()
}
