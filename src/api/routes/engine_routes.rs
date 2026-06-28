//! Engine API routes — LLM providers, models, and engine config.
//!
//! All routes are protected by auth middleware (applied in `build_routes`).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::api::error::AppError;
use crate::api::server::AppState;

// ── Request types ───────────────────────────────────────────────────────────

/// Query params for GET /api/engine/models
#[derive(Debug, Deserialize, Default)]
pub struct ModelsQuery {
    /// Filter by provider name.
    pub provider: Option<String>,
    /// Search query to filter models.
    pub q: Option<String>,
}

/// Request body for PUT /api/engine/model
#[derive(Debug, Deserialize)]
pub struct SetModelRequest {
    /// Model ID in "provider/model" format.
    pub model_id: String,
}

/// Request body for PUT /api/engine/api-key
#[derive(Debug, Deserialize)]
pub struct SetApiKeyRequest {
    /// Provider name (e.g. "anthropic", "openai").
    pub provider: String,
    /// API key to store.
    pub api_key: String,
}

/// Request body for PUT /api/engine/provider-options
#[derive(Debug, Deserialize)]
pub struct SetProviderOptionsRequest {
    /// Provider-specific options.
    pub options: oxi_sdk::ProviderOptions,
}

/// Request body for POST /api/engine/validate-key
#[derive(Debug, Deserialize)]
pub struct ValidateKeyRequest {
    /// Provider name.
    pub provider: String,
    /// API key to validate. When empty, validates the stored key.
    #[serde(default)]
    pub api_key: String,
}

/// Query params for DELETE /api/engine/api-key
#[derive(Debug, Deserialize)]
pub struct DeleteKeyQuery {
    /// Provider name whose key should be deleted.
    pub provider: String,
}

/// Request body for PUT /api/engine/routing
pub type RoutingUpdateRequest = oxios_kernel::RoutingUpdate;

// ── Response types ──────────────────────────────────────────────────────────

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/engine/providers — List all available LLM providers.
pub(crate) async fn handle_engine_providers(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let providers = state.kernel.engine.providers();
    Ok(Json(serde_json::json!({
        "providers": providers,
    })))
}

/// GET /api/engine/models — List models, optionally filtered by provider and/or query.
///
/// Query params:
/// - `provider` — filter by provider name
/// - `q` — search query
pub(crate) async fn handle_engine_models(
    state: State<Arc<AppState>>,
    Query(params): Query<ModelsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let models = match (params.provider.as_deref(), params.q.as_deref()) {
        (Some(provider), Some(q)) => state.kernel.engine.models(provider, Some(q)),
        (Some(provider), None) => state.kernel.engine.models(provider, None),
        (None, Some(q)) => state.kernel.engine.search_models(q),
        (None, None) => {
            // RFC-032: return models from ALL providers that have credentials
            // configured, not just the default provider. This lets the frontend
            // show a complete model picker regardless of which provider is active.
            let connected: Vec<_> = state
                .kernel
                .engine
                .providers()
                .into_iter()
                .filter(|p| p.has_key)
                .collect();
            let providers = if connected.is_empty() {
                state.kernel.engine.providers()
            } else {
                connected
            };
            let mut all_models = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for p in providers {
                for m in state.kernel.engine.models(&p.id, None) {
                    if seen.insert(m.id.clone()) {
                        all_models.push(m);
                    }
                }
            }
            all_models
        }
    };
    Ok(Json(serde_json::json!({
        "models": models,
        "count": models.len(),
    })))
}

/// GET /api/engine/config — Get current engine configuration.
pub(crate) async fn handle_engine_config(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.kernel.engine.config();
    Ok(Json(serde_json::json!(config)))
}

/// PUT /api/engine/model — Set the default model.
pub(crate) async fn handle_engine_set_model(
    state: State<Arc<AppState>>,
    Json(body): Json<SetModelRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if body.model_id.is_empty() {
        return Err(AppError::BadRequest("model_id is required".into()));
    }

    state
        .kernel
        .engine
        .set_model(&body.model_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Also update the shared AppState config
    {
        let mut cfg = state.config.write();
        cfg.engine.default_model = body.model_id.clone();
    }

    tracing::info!(model = %body.model_id, "Engine model updated");
    Ok(Json(serde_json::json!({
        "ok": true,
        "model": body.model_id,
    })))
}

/// PUT /api/engine/api-key — Set an API key for a provider.
pub(crate) async fn handle_engine_set_api_key(
    state: State<Arc<AppState>>,
    Json(body): Json<SetApiKeyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if body.provider.is_empty() {
        return Err(AppError::BadRequest("provider is required".into()));
    }
    if body.api_key.is_empty() {
        return Err(AppError::BadRequest("api_key is required".into()));
    }

    state
        .kernel
        .engine
        .set_api_key(&body.provider, &body.api_key)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(provider = %body.provider, "API key set");
    Ok(Json(serde_json::json!({
        "ok": true,
        "provider": body.provider,
    })))
}

/// DELETE /api/engine/api-key — Delete a provider's API key entirely.
///
/// Removes the key from the credential store (`~/.oxi/auth.json`) and
/// config.toml. Keys sourced from environment variables cannot be
/// removed this way — the caller should check the credential source first.
pub(crate) async fn handle_engine_delete_api_key(
    state: State<Arc<AppState>>,
    Query(params): Query<DeleteKeyQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    if params.provider.is_empty() {
        return Err(AppError::BadRequest("provider is required".into()));
    }

    state
        .kernel
        .engine
        .delete_api_key(&params.provider)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(provider = %params.provider, "API key deleted");
    Ok(Json(serde_json::json!({
        "ok": true,
        "provider": params.provider,
    })))
}

/// PUT /api/engine/provider-options — Update provider-specific options.
pub(crate) async fn handle_engine_set_provider_options(
    state: State<Arc<AppState>>,
    Json(body): Json<SetProviderOptionsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .engine
        .set_provider_options(&body.options)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "ok": true,
    })))
}

/// POST /api/engine/validate-key — Validate an API key.
pub(crate) async fn handle_engine_validate_key(
    state: State<Arc<AppState>>,
    Json(body): Json<ValidateKeyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if body.provider.is_empty() {
        return Err(AppError::BadRequest("provider is required".into()));
    }

    let result = if body.api_key.is_empty() {
        state
            .kernel
            .engine
            .validate_stored_key(&body.provider)
            .await
    } else {
        state
            .kernel
            .engine
            .validate_key(&body.provider, &body.api_key)
            .await
    };
    Ok(Json(serde_json::json!(result)))
}

/// GET /api/engine/roles — Get the current role routing config (RFC-032).
pub(crate) async fn handle_engine_roles(
    state: State<Arc<crate::api::server::AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let role_routing = state.kernel.engine.role_routing();
    Ok(Json(serde_json::json!({
        "roles": role_routing.roles,
        "count": role_routing.roles.len(),
    })))
}

/// PUT /api/engine/roles — Update role routing config and persist (RFC-032).
pub(crate) async fn handle_engine_set_roles(
    state: State<Arc<crate::api::server::AppState>>,
    Json(body): Json<RoleRoutingUpdateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let role_routing = oxios_kernel::config::RoleRoutingConfig { roles: body.roles };
    state
        .kernel
        .engine
        .set_role_routing(role_routing)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "roles": state.kernel.engine.role_routing().roles,
    })))
}

/// Request body for PUT /api/engine/roles.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct RoleRoutingUpdateRequest {
    pub roles: std::collections::HashMap<String, String>,
}

/// PUT /api/engine/routing — Update routing configuration.
pub(crate) async fn handle_engine_set_routing(
    state: State<Arc<AppState>>,
    Json(body): Json<RoutingUpdateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .engine
        .set_routing(body)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!("Routing configuration updated via API");
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/engine/routing/stats — Get model usage statistics.
pub(crate) async fn handle_engine_routing_stats(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let stats = state.kernel.engine.routing_stats_snapshot();
    Ok(Json(serde_json::json!(stats)))
}

/// GET /api/engine/routing/fallbacks — Get recent fallback history.
#[derive(Debug, Deserialize, Default)]
pub struct FallbacksQuery {
    pub limit: Option<usize>,
}

pub(crate) async fn handle_engine_routing_fallbacks(
    state: State<Arc<AppState>>,
    Query(params): Query<FallbacksQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = params.limit.unwrap_or(20);
    let events = state.kernel.engine.fallback_history(limit);
    Ok(Json(serde_json::json!({
        "events": events,
        "total_count": events.len(),
    })))
}
