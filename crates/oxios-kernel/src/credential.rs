//! Multi-source credential resolution.
//!
//! Reads API keys from multiple sources with clear priority:
//! 1. `config.toml` → `[engine].api_key` (explicit override)
//! 2. `~/.oxi/auth.json` (shared with oxi CLI if installed)
//! 3. oxi-ai env var fallback (CI/CD, containers)

use anyhow::Result;

/// Where a credential was found.
#[derive(Debug, Clone)]
pub enum CredentialSource {
    /// From config.toml [engine].api_key
    Config,
    /// From ~/.oxi/auth.json (oxi CLI credential store)
    OxiAuthStore,
    /// From environment variable
    EnvVar,
}

/// Multi-source credential resolver.
pub struct CredentialStore;

impl CredentialStore {
    /// Resolve the best available API key for a provider.
    ///
    /// Priority: config.toml → oxi auth.json → env var
    pub fn resolve(provider: &str, config_key: Option<&str>) -> Option<(String, CredentialSource)> {
        // 1. config.toml explicit key
        if let Some(key) = config_key {
            if !key.is_empty() {
                return Some((key.to_string(), CredentialSource::Config));
            }
        }

        // 2. oxi auth store (~/.oxi/auth.json)
        if let Ok(Some(token)) = oxi_sdk::load_token(provider) {
            if !token.access_token.is_empty() {
                return Some((token.access_token, CredentialSource::OxiAuthStore));
            }
        }

        // 3. oxi-ai env var fallback
        if let Some(key) = oxi_sdk::get_env_api_key(provider) {
            return Some((key, CredentialSource::EnvVar));
        }

        None
    }

    /// Check if any credential is available for a provider.
    pub fn has_credential(provider: &str, config_key: Option<&str>) -> bool {
        Self::resolve(provider, config_key).is_some()
    }

    /// Store an API key to oxi's auth store (~/.oxi/auth.json).
    ///
    /// This is called by the onboarding wizard. If oxi CLI is also
    /// installed on this machine, it will pick up the same credential.
    pub fn store(provider: &str, api_key: &str) -> Result<()> {
        let token = oxi_sdk::TokenBundle {
            access_token: api_key.to_string(),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            obtained_at: chrono::Utc::now(),
            expires_in: 0,
            scope: None,
        };
        oxi_sdk::save_token(provider, &token)?;
        tracing::info!(provider = %provider, "API key stored to oxi auth store");
        Ok(())
    }

    /// Extract the provider name from a model ID.
    /// "anthropic/claude-sonnet-4-20250514" → "anthropic"
    /// Returns `None` if the model ID is empty or has no provider prefix.
    pub fn provider_from_model(model_id: &str) -> Option<&str> {
        if model_id.is_empty() {
            return None;
        }
        model_id.split_once('/').map(|(p, _)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_model() {
        assert_eq!(
            CredentialStore::provider_from_model("anthropic/claude-sonnet-4-20250514"),
            Some("anthropic")
        );
        assert_eq!(
            CredentialStore::provider_from_model("openai/gpt-4o"),
            Some("openai")
        );
        assert_eq!(
            CredentialStore::provider_from_model("bare-model"),
            None
        );
        assert_eq!(
            CredentialStore::provider_from_model(""),
            None
        );
    }

    #[test]
    fn test_config_key_takes_priority() {
        // If config_key is set, it's always returned (even if other sources exist)
        let result = CredentialStore::resolve("anthropic", Some("sk-test-config-key"));
        assert!(result.is_some());
        let (key, source) = result.unwrap();
        assert_eq!(key, "sk-test-config-key");
        assert!(matches!(source, CredentialSource::Config));
    }

    #[test]
    fn test_empty_config_key_skipped() {
        let result = CredentialStore::resolve("anthropic", Some(""));
        // Empty string is treated as None — falls through to next source
        // (result depends on whether auth.json or env vars exist)
        // Just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_none_config_key_skipped() {
        let result = CredentialStore::resolve("anthropic", None);
        let _ = result; // depends on system state
    }
}
