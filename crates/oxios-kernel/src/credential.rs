//! Multi-source credential resolution.
//!
//! Reads API keys from multiple sources with clear priority:
//! 1. `config.toml` → `[engine].api_key` (explicit override)
//! 2. `~/.oxi/auth.json` (shared with oxi CLI if installed)
//! 3. oxi-ai env var fallback (CI/CD, containers)
//!
//! Handles legacy `oxi-cli` auth.json entries (`{"type":"api_key","key":"..."}`)
//! by auto-migrating them to the `TokenBundle` format on first write.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

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
    /// Priority: OXIOS_<PROVIDER>_API_KEY env → config.toml → oxi auth.json → oxi-ai env fallback
    /// Environment variables take highest priority for container/K8s deployments.
    pub fn resolve(provider: &str, config_key: Option<&str>) -> Option<(String, CredentialSource)> {
        // 1. Explicit Oxios env var: OXIOS_<PROVIDER>_API_KEY (highest priority for containers)
        let env_var = format!("OXIOS_{}_API_KEY", provider.to_uppercase());
        if let Ok(key) = std::env::var(&env_var)
            && !key.is_empty()
        {
            return Some((key, CredentialSource::EnvVar));
        }

        // 2. config.toml explicit key
        if let Some(key) = config_key
            && !key.is_empty()
        {
            return Some((key.to_string(), CredentialSource::Config));
        }

        // 3. oxi auth store (~/.oxi/auth.json)
        //    Try standard TokenBundle format first, then fall back to legacy
        //    oxi-cli format (`{"type":"api_key","key":"..."}`).
        if let Ok(Some(token)) = oxi_sdk::load_token(provider) {
            if !token.access_token.is_empty() {
                return Some((token.access_token, CredentialSource::OxiAuthStore));
            }
        } else if let Some(key) = try_load_legacy_key(provider) {
            return Some((key, CredentialSource::OxiAuthStore));
        }

        // 4. oxi-ai env var fallback
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
    ///
    /// If the auth store contains legacy entries from `oxi-cli` that don't
    /// deserialize as `TokenBundle`, they are auto-migrated before saving.
    pub fn store(provider: &str, api_key: &str) -> Result<()> {
        let token = oxi_sdk::TokenBundle {
            access_token: api_key.to_string(),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            obtained_at: chrono::Utc::now(),
            expires_in: 0,
            scope: None,
        };

        // Try the normal path first.
        if let Err(e) = oxi_sdk::save_token(provider, &token) {
            // If the auth store has legacy entries (e.g. `oxi-cli` wrote
            // `{"type":"api_key","key":"..."}`), `save_token` fails because
            // it can't deserialize them as `TokenBundle`.  Migrate and retry.
            if is_legacy_auth_error(&e) {
                tracing::info!("auth.json has legacy format, migrating to TokenBundle");
                migrate_legacy_auth_store(provider, &token)?;
            } else {
                return Err(e.into());
            }
        }

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

// ── Legacy auth.json migration ─────────────────────────────────────────────

/// Legacy entry from `oxi-cli`: `{"type":"api_key","key":"..."}`.
#[derive(serde::Deserialize)]
struct LegacyEntry {
    #[allow(dead_code)]
    r#type: String,
    key: String,
}

/// Try to load a legacy `oxi-cli` API key from auth.json.
///
/// Returns `Some(key)` if the provider entry exists in the legacy
/// `{"type":"api_key","key":"..."}` format.
fn try_load_legacy_key(provider: &str) -> Option<String> {
    let raw = std::fs::read_to_string(auth_json_path().ok()?).ok()?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).ok()?;
    let entry = map.get(provider)?;
    let legacy: LegacyEntry = serde_json::from_value(entry.clone()).ok()?;
    if legacy.key.is_empty() {
        None
    } else {
        Some(legacy.key)
    }
}

/// Check if an error is caused by a legacy-format auth.json.
fn is_legacy_auth_error(err: &oxi_sdk::OAuthError) -> bool {
    matches!(err, oxi_sdk::OAuthError::Json(_))
}

/// Migrate a legacy auth.json to `TokenBundle` format, preserving entries that
/// can be converted and writing the new token for `provider`.
fn migrate_legacy_auth_store(provider: &str, new_token: &oxi_sdk::TokenBundle) -> Result<()> {
    let path = auth_json_path()?;
    let raw = std::fs::read_to_string(&path)?;

    // Parse as a flat JSON map.
    let entries: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&raw).unwrap_or_default();

    let mut migrated = HashMap::new();

    for (key, value) in &entries {
        if key == provider {
            continue; // will be replaced with new_token below
        }

        // Try parsing as TokenBundle first.
        if let Ok(bundle) = serde_json::from_value::<oxi_sdk::TokenBundle>(value.clone()) {
            migrated.insert(key.clone(), bundle);
            continue;
        }

        // Try parsing as legacy `{"type":"api_key","key":"..."}`.
        if let Ok(legacy) = serde_json::from_value::<LegacyEntry>(value.clone()) {
            migrated.insert(
                key.clone(),
                oxi_sdk::TokenBundle {
                    access_token: legacy.key,
                    refresh_token: None,
                    token_type: "Bearer".to_string(),
                    obtained_at: chrono::Utc::now(),
                    expires_in: 0,
                    scope: None,
                },
            );
            continue;
        }

        tracing::warn!(provider = %key, "skipping unparseable auth.json entry during migration");
    }

    // Insert the new token.
    migrated.insert(provider.to_string(), new_token.clone());

    // Write back as proper AuthStore.
    let store = oxi_sdk::AuthStore { tokens: migrated };
    oxi_sdk::save_auth_store(&store)?;
    Ok(())
}

/// Resolve `~/.oxi/auth.json` path without depending on oxi_sdk's error type.
fn auth_json_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(PathBuf::from(home).join(".oxi").join("auth.json"))
}

/// Discover all provider names stored in `~/.oxi/auth.json`.
///
/// Returns a list of provider IDs (top-level keys in the JSON file).
/// Special keys like `"version"` are filtered out. Used by `OxiosEngine::from_config`
/// to ensure credentials from the auth store are always injected, even for
/// providers not in the hardcoded known list.
pub fn discover_auth_store_providers() -> Result<Vec<String>> {
    let path = auth_json_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = std::fs::read_to_string(&path)?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw)?;
    Ok(map
        .keys()
        .filter(|k| *k != "version" && !k.starts_with('_'))
        .cloned()
        .collect())
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
        assert_eq!(CredentialStore::provider_from_model("bare-model"), None);
        assert_eq!(CredentialStore::provider_from_model(""), None);
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
