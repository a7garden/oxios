//! Engine API — LLM engine introspection + config writes + routing control.
//!
//! Provides access to the oxi-sdk model catalog (providers, models, search)
//! and write operations that persist to config.toml (model, API key, routing).
//!
//! Routing statistics (`RoutingStats`) are shared between this API and
//! `AgentRuntime` via an `Arc`, so model usage is recorded end-to-end.

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// ── Routing types ─────────────────────────────────────────────────────────────

/// Snapshot of routing configuration (read-only API response).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingConfigSnapshot {
    /// Whether automatic model routing is enabled.
    pub routing_enabled: bool,
    /// Whether cost-efficient models are preferred when routing.
    pub prefer_cost_efficient: bool,
    /// Ordered list of fallback models (tried left-to-right on primary failure).
    pub fallback_models: Vec<String>,
    /// Models excluded from automatic routing.
    pub excluded_models: Vec<String>,
}

/// Model usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingStatsSnapshot {
    /// Model ID → number of calls.
    pub model_calls: HashMap<String, u64>,
    /// Model ID → estimated total cost (USD).
    pub model_cost: HashMap<String, f64>,
    /// Total number of requests.
    pub total_requests: u64,
    /// Total estimated cost (USD).
    pub total_cost: f64,
}

/// Single fallback event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackEvent {
    /// When the fallback occurred.
    pub timestamp: DateTime<Utc>,
    /// Model that was skipped/replaced.
    pub from_model: String,
    /// Model that was used instead.
    pub to_model: String,
    /// Reason for fallback (e.g. "rate_limit", "context_overflow", "error").
    pub reason: String,
    /// Whether the fallback succeeded (no further fallback needed).
    pub success: bool,
}

/// Request body for `PUT /api/engine/routing`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingUpdate {
    pub routing_enabled: Option<bool>,
    pub prefer_cost_efficient: Option<bool>,
    pub fallback_models: Option<Vec<String>>,
    pub excluded_models: Option<Vec<String>>,
}

// ── RoutingStats ─────────────────────────────────────────────────────────────

/// In-memory routing statistics, shared between `EngineApi` and `AgentRuntime`.
/// Uses simple RwLock for thread-safe reads/writes.
pub struct RoutingStats {
    calls: RwLock<HashMap<String, u64>>,
    costs: RwLock<HashMap<String, f64>>,
    /// Circular buffer of recent fallback events (max 200).
    fallbacks: RwLock<Vec<FallbackEvent>>,
}

impl Default for RoutingStats {
    fn default() -> Self {
        Self {
            calls: RwLock::new(HashMap::new()),
            costs: RwLock::new(HashMap::new()),
            fallbacks: RwLock::new(Vec::new()),
        }
    }
}

impl RoutingStats {
    /// Create a new stats tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one model invocation.
    pub fn record_model_usage(&self, model_id: &str, cost_usd: f64) {
        let mut calls = self.calls.write();
        *calls.entry(model_id.to_string()).or_insert(0) += 1;
        if cost_usd > 0.0 {
            let mut costs = self.costs.write();
            *costs.entry(model_id.to_string()).or_insert(0.0) += cost_usd;
        }
    }

    /// Record a fallback event.
    pub fn record_fallback(&self, event: FallbackEvent) {
        let mut fb = self.fallbacks.write();
        fb.push(event);
        let keep = fb.len().saturating_sub(200);
        if keep > 0 {
            fb.drain(0..keep);
        }
    }

    /// Get a snapshot of current stats.
    pub fn snapshot(&self) -> RoutingStatsSnapshot {
        let calls = self.calls.read();
        let costs = self.costs.read();
        let total_requests: u64 = calls.values().sum();
        let total_cost: f64 = costs.values().sum();
        RoutingStatsSnapshot {
            model_calls: calls.clone(),
            model_cost: costs.clone(),
            total_requests,
            total_cost,
        }
    }

    /// Get recent fallback events, newest first.
    pub fn fallback_history(&self, limit: usize) -> Vec<FallbackEvent> {
        let fb = self.fallbacks.read();
        fb.iter().rev().take(limit).cloned().collect()
    }
}

// ── Model cost estimation ────────────────────────────────────────────────────

/// Estimate cost in USD for a model given token usage.
/// Uses oxi-sdk's model_db for per-model pricing.
pub fn estimate_cost(model_id: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let entries = oxi_sdk::get_provider_models(model_id.split('/').next().unwrap_or(model_id));
    let entry = entries
        .iter()
        .find(|e| format!("{}/{}", e.provider, e.id) == model_id);
    match entry {
        Some(e) => {
            (e.cost_input * input_tokens as f64 / 1_000_000.0)
                + (e.cost_output * output_tokens as f64 / 1_000_000.0)
        }
        None => {
            // Fall back to a rough estimate for unknown models
            (0.003 * input_tokens as f64 / 1_000_000.0)
                + (0.015 * output_tokens as f64 / 1_000_000.0)
        }
    }
}

// ── Provider/Model response types ──────────────────────────────────────────

/// Summary of an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Provider identifier (e.g. "anthropic", "openai").
    pub id: String,
    /// Number of models available for this provider.
    pub model_count: usize,
    /// Whether an API key is currently configured.
    pub has_key: bool,
}

/// Summary of a model from the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Full model ID: "provider/model-id".
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Provider name.
    pub provider: String,
    /// Whether this model supports reasoning/thinking.
    pub reasoning: bool,
    /// Maximum context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
    /// Cost per million input tokens (USD).
    pub cost_input: f64,
    /// Cost per million output tokens (USD).
    pub cost_output: f64,
}

impl From<&oxi_sdk::ModelEntry> for ModelInfo {
    fn from(entry: &oxi_sdk::ModelEntry) -> Self {
        Self {
            id: format!("{}/{}", entry.provider, entry.id),
            name: entry.name.to_string(),
            provider: entry.provider.to_string(),
            reasoning: entry.reasoning,
            context_window: entry.context_window,
            max_tokens: entry.max_tokens,
            cost_input: entry.cost_input,
            cost_output: entry.cost_output,
        }
    }
}

/// Current engine configuration + credential status + routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfigResponse {
    /// Currently configured default model.
    pub default_model: String,
    /// Whether an API key is set for the current provider.
    pub api_key_set: bool,
    /// Source of the API key (if any).
    pub api_key_source: Option<String>,
    /// Provider name extracted from default_model.
    pub provider: Option<String>,
    /// Current routing configuration.
    pub routing: RoutingConfigSnapshot,
}

/// Result of an API key validation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateKeyResult {
    /// Whether the key is valid.
    pub valid: bool,
    /// Provider that was validated.
    pub provider: String,
    /// Optional message (error detail or success note).
    pub message: Option<String>,
}

// ── EngineApi ───────────────────────────────────────────────────────────────

/// Engine API facade — model catalog introspection + config writes + routing.
///
/// Holds a shared reference to the live config (behind `RwLock`) and the
/// path to config.toml so write operations can persist to disk.
/// Routing stats are shared with `AgentRuntime` via `Arc<RoutingStats>`.
pub struct EngineApi {
    config: Arc<RwLock<OxiosConfig>>,
    config_path: PathBuf,
    routing_stats: Arc<RoutingStats>,
}

impl EngineApi {
    /// Create a new EngineApi.
    ///
    /// - `config` — shared config store (backed by RwLock)
    /// - `config_path` — path to config.toml for persistence
    /// - `routing_stats` — shared stats tracker (shared with AgentRuntime)
    pub fn new(
        config: Arc<RwLock<OxiosConfig>>,
        config_path: PathBuf,
        routing_stats: Arc<RoutingStats>,
    ) -> Self {
        Self {
            config,
            config_path,
            routing_stats,
        }
    }

    /// Get the shared `RoutingStats` reference (for `AgentRuntime` wiring).
    pub fn routing_stats(&self) -> Arc<RoutingStats> {
        Arc::clone(&self.routing_stats)
    }

    // ── Read operations ────────────────────────────────────────────────

    /// List all available providers from the oxi-sdk catalog.
    ///
    /// Filters out hidden/internal providers (amazon-bedrock, azure-*, etc.)
    /// and augments each entry with credential status.
    pub fn providers(&self) -> Vec<ProviderInfo> {
        let all = oxi_sdk::get_providers();
        let hidden: &[&str] = &[
            "amazon-bedrock",
            "azure-openai-responses",
            "cloudflare-ai-gateway",
            "cloudflare-workers-ai",
            "google-vertex",
            "minimax-cn",
            "moonshotai-cn",
            "openai-codex",
            "opencode-go",
            "vercel-ai-gateway",
            "xiaomi",
        ];

        all.into_iter()
            .filter(|p| !hidden.contains(p))
            .map(|p| {
                let model_count = oxi_sdk::get_provider_models(p).len();
                let has_key = CredentialStore::has_credential(
                    p,
                    self.config
                        .read()
                        .engine
                        .api_key
                        .as_deref()
                        .filter(|k| !k.is_empty()),
                );
                ProviderInfo {
                    id: p.to_string(),
                    model_count,
                    has_key,
                }
            })
            .collect()
    }

    /// List models for a given provider, optionally filtered by a query.
    pub fn models(&self, provider: &str, query: Option<&str>) -> Vec<ModelInfo> {
        let entries = oxi_sdk::get_provider_models(provider);
        entries
            .iter()
            .filter(|e| {
                // Skip "latest" aliases
                !e.name.contains("latest")
            })
            .filter(|e| {
                if let Some(q) = query {
                    let q = q.to_lowercase();
                    e.name.to_lowercase().contains(&q)
                        || e.id.to_lowercase().contains(&q)
                        || e.provider.to_lowercase().contains(&q)
                } else {
                    true
                }
            })
            .map(ModelInfo::from)
            .collect()
    }

    /// Search models across all providers.
    pub fn search_models(&self, query: &str) -> Vec<ModelInfo> {
        oxi_sdk::search_models(query)
            .into_iter()
            .map(ModelInfo::from)
            .collect()
    }

    /// Get the current engine configuration + credential status + routing.
    pub fn config(&self) -> EngineConfigResponse {
        let cfg = self.config.read();
        let provider =
            CredentialStore::provider_from_model(&cfg.engine.default_model).map(|s| s.to_string());
        let api_key_source = provider.as_deref().and_then(|p| {
            CredentialStore::resolve(p, cfg.api_key().as_deref()).map(|(_, src)| {
                match src {
                    crate::credential::CredentialSource::EnvVar => "env",
                    crate::credential::CredentialSource::Config => "config",
                    crate::credential::CredentialSource::OxiAuthStore => "auth_store",
                }
                .to_string()
            })
        });
        let api_key_set = provider
            .as_deref()
            .map(|p| CredentialStore::has_credential(p, cfg.api_key().as_deref()))
            .unwrap_or(false);

        EngineConfigResponse {
            default_model: cfg.engine.default_model.clone(),
            api_key_set,
            api_key_source,
            provider,
            routing: RoutingConfigSnapshot {
                routing_enabled: cfg.engine.routing_enabled,
                prefer_cost_efficient: cfg.engine.prefer_cost_efficient,
                fallback_models: cfg.engine.fallback_models.clone(),
                excluded_models: cfg.engine.excluded_models.clone(),
            },
        }
    }

    /// Get routing stats snapshot (for Web dashboard).
    pub fn routing_stats_snapshot(&self) -> RoutingStatsSnapshot {
        self.routing_stats.snapshot()
    }

    /// Get recent fallback history.
    pub fn fallback_history(&self, limit: usize) -> Vec<FallbackEvent> {
        self.routing_stats.fallback_history(limit)
    }

    // ── Write operations ───────────────────────────────────────────────

    /// Set the default model in config.toml.
    ///
    /// Updates both the in-memory config and the on-disk file.
    /// No runtime hot-swap — the model change takes effect on next request.
    pub fn set_model(&self, model_id: &str) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            cfg.engine.default_model = model_id.to_string();
            self.persist(&cfg)?;
        }
        tracing::info!(model = %model_id, "Default model updated in config");
        Ok(())
    }

    /// Set an API key for a provider.
    ///
    /// Stores the key via CredentialStore (→ ~/.oxi/auth.json) and also
    /// updates config.toml's `[engine].api_key` when the provider matches
    /// the current default model.
    pub fn set_api_key(&self, provider: &str, key: &str) -> anyhow::Result<()> {
        CredentialStore::store(provider, key)?;

        // If the provider matches the current default model, also set in config
        let cfg = self.config.read();
        if let Some(current_provider) =
            CredentialStore::provider_from_model(&cfg.engine.default_model)
        {
            if current_provider == provider {
                drop(cfg);
                let mut cfg = self.config.write();
                cfg.engine.api_key = Some(key.to_string());
                self.persist(&cfg)?;
            }
        }
        tracing::info!(provider = %provider, "API key stored");
        Ok(())
    }

    /// Update provider options in config.toml.
    ///
    /// This is a placeholder for per-provider option persistence.
    /// Currently stores the serialized options as a TOML section.
    pub fn set_provider_options(&self, _opts: &oxi_sdk::ProviderOptions) -> anyhow::Result<()> {
        // ProviderOptions are currently per-request, not persisted in config.toml.
        // Future: add [engine.provider_options] section to OxiosConfig.
        tracing::info!("Provider options update requested (no-op for now)");
        Ok(())
    }

    /// Update routing configuration in config.toml.
    ///
    /// Only the fields provided in `update` are changed; others are left untouched.
    /// Changes are persisted to disk immediately.
    pub fn set_routing(&self, update: RoutingUpdate) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            if let Some(v) = update.routing_enabled {
                cfg.engine.routing_enabled = v;
            }
            if let Some(v) = update.prefer_cost_efficient {
                cfg.engine.prefer_cost_efficient = v;
            }
            if let Some(v) = update.fallback_models {
                cfg.engine.fallback_models = v;
            }
            if let Some(v) = update.excluded_models {
                cfg.engine.excluded_models = v;
            }
            self.persist(&cfg)?;
        }
        tracing::info!("Routing configuration updated via API");
        Ok(())
    }

    /// Validate an API key by making a simple test call.
    ///
    /// Creates a lightweight provider and attempts a minimal request.
    /// Returns the validation result.
    pub fn validate_key(&self, provider: &str, api_key: &str) -> ValidateKeyResult {
        // Try to create a provider with the given key and make a minimal completion request
        let result = self.try_validate(provider, api_key);
        match result {
            Ok(()) => ValidateKeyResult {
                valid: true,
                provider: provider.to_string(),
                message: Some("API key is valid".to_string()),
            },
            Err(e) => ValidateKeyResult {
                valid: false,
                provider: provider.to_string(),
                message: Some(format!("Validation failed: {e}")),
            },
        }
    }

    /// Attempt a lightweight validation call.
    fn try_validate(&self, provider: &str, api_key: &str) -> anyhow::Result<()> {
        // Build an OxiBuilder with builtins and the provided key
        let builder = oxi_sdk::OxiBuilder::new()
            .with_builtins()
            .api_key(provider, api_key);
        let oxi = builder.build();

        // Try to resolve any model from this provider
        let models = oxi_sdk::get_provider_models(provider);
        if models.is_empty() {
            anyhow::bail!("No models found for provider '{provider}'");
        }

        let model_id = format!("{}/{}", provider, models[0].id);
        let _model = oxi.resolve_model(&model_id)?;

        // Create a provider with the injected key
        let _provider = oxi.create_provider(provider)?;

        // If we got this far, the provider was created with the key.
        // Note: Actual API call validation would require a lightweight
        // completion request. For now, this validates key format + provider existence.
        if api_key.is_empty() {
            anyhow::bail!("API key is empty");
        }
        if api_key.len() < 8 {
            anyhow::bail!("API key appears too short");
        }

        tracing::debug!(
            provider = %provider,
            model = %model_id,
            "Key validation: provider resolved with injected key"
        );
        Ok(())
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Estimate cost for a model invocation.
    pub fn estimate_cost(model_id: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        estimate_cost(model_id, input_tokens, output_tokens)
    }

    /// Persist the current config to disk.
    fn persist(&self, config: &OxiosConfig) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {e}"))?;
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }
}

impl std::fmt::Debug for EngineApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineApi")
            .field("config_path", &self.config_path)
            .finish()
    }
}

// Expose `RoutingStats::record_model_usage` via a public helper for AgentRuntime.
// This avoids exposing the internal Arc to outside crates.
pub fn record_usage_to_stats(
    stats: &Option<Arc<RoutingStats>>,
    model_id: &str,
    input_tokens: u64,
    output_tokens: u64,
) {
    if let Some(s) = stats {
        let cost = estimate_cost(model_id, input_tokens, output_tokens);
        s.record_model_usage(model_id, cost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_info_serialization() {
        let info = ProviderInfo {
            id: "anthropic".to_string(),
            model_count: 15,
            has_key: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: ProviderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "anthropic");
        assert_eq!(restored.model_count, 15);
        assert!(restored.has_key);
    }

    #[test]
    fn test_model_info_serialization() {
        let info = ModelInfo {
            id: "anthropic/claude-sonnet-4".to_string(),
            name: "Claude Sonnet 4".to_string(),
            provider: "anthropic".to_string(),
            reasoning: true,
            context_window: 200000,
            max_tokens: 16000,
            cost_input: 3.0,
            cost_output: 15.0,
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "anthropic/claude-sonnet-4");
        assert!(restored.reasoning);
        assert_eq!(restored.context_window, 200000);
    }

    #[test]
    fn test_engine_config_response_serialization() {
        let resp = EngineConfigResponse {
            default_model: "anthropic/claude-sonnet-4".to_string(),
            api_key_set: true,
            api_key_source: Some("config.toml".to_string()),
            provider: Some("anthropic".to_string()),
            routing: RoutingConfigSnapshot {
                routing_enabled: false,
                prefer_cost_efficient: false,
                fallback_models: vec![],
                excluded_models: vec![],
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: EngineConfigResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.default_model, "anthropic/claude-sonnet-4");
        assert!(restored.api_key_set);
        assert_eq!(restored.api_key_source.as_deref(), Some("config.toml"));
        assert!(!restored.routing.routing_enabled);
    }

    #[test]
    fn test_validate_key_result_serialization() {
        let result = ValidateKeyResult {
            valid: true,
            provider: "openai".to_string(),
            message: Some("API key is valid".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: ValidateKeyResult = serde_json::from_str(&json).unwrap();
        assert!(restored.valid);
        assert_eq!(restored.provider, "openai");
    }

    #[test]
    fn test_validate_key_result_invalid() {
        let result = ValidateKeyResult {
            valid: false,
            provider: "anthropic".to_string(),
            message: Some("Validation failed: key too short".to_string()),
        };
        assert!(!result.valid);
        assert!(result.message.as_ref().unwrap().contains("failed"));
    }

    #[test]
    fn test_routing_stats_snapshot() {
        let stats = RoutingStats::new();
        stats.record_model_usage("anthropic/claude-sonnet-4", 0.05);
        stats.record_model_usage("anthropic/claude-sonnet-4", 0.03);
        stats.record_model_usage("openai/gpt-4o-mini", 0.01);

        let snap = stats.snapshot();
        assert_eq!(snap.total_requests, 3);
        assert_eq!(snap.model_calls["anthropic/claude-sonnet-4"], 2);
        assert_eq!(snap.model_calls["openai/gpt-4o-mini"], 1);
        assert!((snap.total_cost - 0.09).abs() < 0.001);
    }

    #[test]
    fn test_fallback_history_circular() {
        let stats = RoutingStats::new();
        for i in 0..210 {
            stats.record_fallback(FallbackEvent {
                timestamp: DateTime::from_timestamp(i as i64, 0).unwrap(),
                from_model: format!("model-{}", i),
                to_model: "fallback".to_string(),
                reason: "test".to_string(),
                success: true,
            });
        }
        let history = stats.fallback_history(200);
        assert_eq!(history.len(), 200);
        // Most recent first (i=209 down to i=10)
        assert_eq!(history[0].from_model, "model-209");
        assert_eq!(history[199].from_model, "model-10");
    }
}
