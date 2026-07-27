//! RFC-035 approval configuration HTTP API.
use crate::api::{error::AppError, server::AppState};
use axum::{
    Json,
    extract::{Path, State},
};
use oxios_kernel::approval::{ApprovalConfig, ApprovalMode, ToolPolicy};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Debug, Default, Deserialize)]
pub(crate) struct ApprovalConfigPatch {
    pub mode: Option<ApprovalMode>,
    pub allow_list: Option<Vec<String>>,
    pub tool_overrides: Option<std::collections::HashMap<String, ToolPolicy>>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct GrantBody {
    pub key: String,
}
fn apply_patch(mut c: ApprovalConfig, p: ApprovalConfigPatch) -> ApprovalConfig {
    if let Some(v) = p.mode {
        c.mode = v;
    }
    if let Some(v) = p.allow_list {
        c.allow_list = v;
    }
    if let Some(v) = p.tool_overrides {
        c.tool_overrides = v;
    }
    c
}
async fn update(state: &AppState, c: ApprovalConfig) -> Result<ApprovalConfig, AppError> {
    state
        .kernel
        .infra
        .set_approval_config(c)
        .await
        .map_err(AppError::from)
}
pub(crate) async fn handle_approval_config_get(
    State(s): State<Arc<AppState>>,
) -> Json<ApprovalConfig> {
    Json(s.kernel.infra.approval_config())
}
pub(crate) async fn handle_approval_config_patch(
    State(s): State<Arc<AppState>>,
    Json(p): Json<ApprovalConfigPatch>,
) -> Result<Json<ApprovalConfig>, AppError> {
    Ok(Json(
        update(&s, apply_patch(s.kernel.infra.approval_config(), p)).await?,
    ))
}
pub(crate) async fn handle_approval_grant_add(
    State(s): State<Arc<AppState>>,
    Json(b): Json<GrantBody>,
) -> Result<Json<ApprovalConfig>, AppError> {
    if b.key.trim().is_empty() {
        return Err(AppError::BadRequest("grant key must not be empty".into()));
    }
    Ok(Json(
        s.kernel
            .infra
            .add_grant(b.key)
            .await
            .map_err(AppError::from)?,
    ))
}
pub(crate) async fn handle_approval_grant_remove(
    State(s): State<Arc<AppState>>,
    Path(k): Path<String>,
) -> Result<Json<ApprovalConfig>, AppError> {
    Ok(Json(
        s.kernel
            .infra
            .remove_grant(&k)
            .await
            .map_err(AppError::from)?,
    ))
}
pub(crate) async fn remember_grant(s: &AppState, key: String) -> Result<ApprovalConfig, AppError> {
    s.kernel
        .infra
        .add_grant(key)
        .await
        .map_err(AppError::from)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_preserves_fields() {
        let c = ApprovalConfig::default();
        assert_eq!(
            apply_patch(
                c,
                ApprovalConfigPatch {
                    mode: Some(ApprovalMode::AutoRun),
                    ..Default::default()
                }
            )
            .mode,
            ApprovalMode::AutoRun
        );
    }
}
