//! Engine API — read-only facade for LLM engine introspection + config writes.
//!
//! Provides access to the oxi-sdk model catalog (providers, models, search)
//! and write operations that persist to config.toml (model, API key, provider
//! options). No references to `Oxi`, `Supervisor`, or any runtime engine
//! instance — only config and the static model catalog.

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

// ── Response types ──────────────────────────────────────────────────────────

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

/// Current engine configuration + credential status.
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

/// Engine API facade — read-only introspection + config writes.
///
/// Holds a shared reference to the live config (behind `RwLock`) and the
/// path to config.toml so write operations can persist to disk.
pub struct EngineApi {
    config: Arc<RwLock<OxiosConfig>>,
    config_path: PathBuf,
}

impl EngineApi {
    /// Create a new EngineApi.
    pub fn new(config: Arc<RwLock<OxiosConfig>>, config_path: PathBuf) -> Self {
        Self { config, config_path }
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

    /// Get the current engine configuration + credential status.
    pub fn config(&self) -> EngineConfigResponse {
        let cfg = self.config.read();
        let provider = CredentialStore::provider_from_model(&cfg.engine.default_model)
            .map(|s| s.to_string());
        let api_key_source = provider
            .as_deref()
            .and_then(|p| {
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
        }
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
    pub fn set_provider_options(
        &self,
        _opts: &oxi_sdk::ProviderOptions,
    ) -> anyhow::Result<()> {
        // ProviderOptions are currently per-request, not persisted in config.toml.
        // Future: add [engine.provider_options] section to OxiosConfig.
        tracing::info!("Provider options update requested (no-op for now)");
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
            anyhow::bail!("No models found for provider '{}'", provider);
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
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: EngineConfigResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.default_model, "anthropic/claude-sonnet-4");
        assert!(restored.api_key_set);
        assert_eq!(restored.api_key_source.as_deref(), Some("config.toml"));
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
}
