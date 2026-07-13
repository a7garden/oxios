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

/// `POST /api/integrations/{id}/install` — provision an integration.
///
/// Runs the first applicable install spec as a privileged kernel op (D8).
/// User-triggered only (the UI shows a confirm gate). Returns the install
/// output synchronously; SSE progress streaming (RFC M3) is a refinement.
pub(crate) async fn handle_integration_install(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<oxios_kernel::host_tools::InstallOutput>, AppError> {
    let out = state
        .kernel
        .host_tools
        .install(&id)
        .await
        .map_err(|e| AppError::Internal(format!("install failed: {e}")))?;
    Ok(Json(out))
}

/// `POST /api/integrations/{id}/oauth/start` — begin a device-code flow.
///
/// Returns `{ handle, user_code, verification_url, expires_in }`. The
/// `device_code` stays daemon-side (H1) — it is never in the response.
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
