//! Token Maxing configuration (RFC-031 §2).
//!
//! The schema is a small surface — the user must opt in, then list each
//! subscription provider with its plan allocation. The eligibility rule is
//! conservative: only providers with `billing_model = "subscription"` are
//! ever used by [`crate::token_maxing::TokenMaxer`]. A metered key can
//! never be silently drafted.
//!
//! ## Example
//!
//! ```toml
//! [token-maxing]
//! enabled = false
//!
//! [[token-maxing.providers]]
//! provider = "zai"
//! billing_model = "subscription"
//! token_limit = 2000000
//! reset_window_secs = 18000          # 5h
//! min_remaining_percent = 5
//! models = ["zai/glm-4.6", "zai/glm-4.5-air"]
//! ```

use serde::{Deserialize, Serialize};

/// The only accepted value of `billing_model`. This is the single choke
/// point that upholds the "절대 동작하면 안 된다" constraint — the
/// `QuotaTracker` will only ever mark a provider eligible when this exact
/// string is present in the config.
pub const SUBSCRIPTION_BILLING_MODEL: &str = "subscription";

/// Top-level `[token-maxing]` configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenMaxingConfig {
    /// Whether the mode is enabled at all. When `false`, the TokenMaxer
    /// refuses to start a session and the API returns 503.
    #[serde(default)]
    pub enabled: bool,

    /// Per-provider plan data. Providers missing from this list are
    /// **ineligible by default** — a metered key can never be drafted in
    /// even by accident.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<TokenMaxingProviderConfig>,

    /// Default per-provider floor (%). The orchestrator stops draining a
    /// provider below this remaining percentage to avoid thrash. A
    /// per-provider override is also available.
    #[serde(default = "default_min_remaining_percent")]
    pub default_min_remaining_percent: u8,

    /// Cadence (seconds) at which the QuotaTracker tries to recalibrate
    /// the self-tracked counter to a real provider endpoint. Has no effect
    /// for providers that expose no fetcher (ZAI/Minimax today). 0 = off.
    #[serde(default = "default_recalibration_interval_secs")]
    pub recalibration_interval_secs: u64,

    /// Maximum concurrent tasks (Phase 5). Phase 1-3 is single-stream.
    #[serde(default = "default_parallel_providers")]
    pub parallel_providers: bool,
}

fn default_min_remaining_percent() -> u8 {
    5
}

fn default_recalibration_interval_secs() -> u64 {
    60
}

fn default_parallel_providers() -> bool {
    false
}

/// Per-provider subscription plan entry. One row per subscription provider
/// the user has opted in to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMaxingProviderConfig {
    /// Provider id (matches the engine's provider id, e.g. `"zai"`).
    pub provider: String,

    /// Billing model. The only accepted value is `"subscription"`. Any
    /// other value causes this entry to be **rejected at load time** so
    /// the metered-never rule cannot be silently violated.
    pub billing_model: String,

    /// Plan allocation per window (tokens). The self-tracked counter
    /// treats this as the cap; `remaining_percent` is computed from it.
    pub token_limit: u64,

    /// Drain window in seconds. The counter resets when `now - window_start
    /// >= reset_window_secs` AND no real `resets_at` is known.
    pub reset_window_secs: u64,

    /// Floor: stop draining below this remaining percentage. Optional
    /// override of [`TokenMaxingConfig::default_min_remaining_percent`].
    #[serde(default)]
    pub min_remaining_percent: Option<u8>,

    /// Models to round-robin within the provider. Empty = use every model
    /// the provider exposes.
    #[serde(default)]
    pub models: Vec<String>,
}

impl TokenMaxingProviderConfig {
    /// Floor for this provider (the per-provider override, falling back to
    /// the global default).
    pub fn min_remaining_percent(&self, default: u8) -> u8 {
        self.min_remaining_percent.unwrap_or(default)
    }
}

impl TokenMaxingConfig {
    /// Returns the per-provider config for `provider`, if any.
    pub fn get(&self, provider: &str) -> Option<&TokenMaxingProviderConfig> {
        self.providers.iter().find(|p| p.provider == provider)
    }

    /// Whether `provider` is eligible for token-maxing under this config.
    ///
    /// The check is the single guard that upholds the metered-never rule:
    /// the provider must (a) have an entry in `providers`, (b) the entry
    /// must declare `billing_model = "subscription"`, and (c) the entry's
    /// `token_limit` must be > 0 (a zero limit would burn nothing anyway).
    pub fn is_eligible(&self, provider: &str) -> bool {
        match self.get(provider) {
            Some(p) => {
                p.billing_model == SUBSCRIPTION_BILLING_MODEL
                    && p.token_limit > 0
                    && p.reset_window_secs > 0
            }
            None => false,
        }
    }

    /// Validate the config — called at load time. Returns the list of
    /// errors. Empty list = valid.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.enabled && self.providers.is_empty() {
            errors.push(
                "token-maxing.enabled = true but no providers configured — refuse to start"
                    .to_string(),
            );
        }
        for (i, p) in self.providers.iter().enumerate() {
            if p.provider.trim().is_empty() {
                errors.push(format!("token-maxing.providers[{i}].provider is empty"));
            }
            if p.billing_model != SUBSCRIPTION_BILLING_MODEL {
                errors.push(format!(
                    "token-maxing.providers[{i}].billing_model = {:?} is not permitted; the only accepted value is {:?}",
                    p.billing_model, SUBSCRIPTION_BILLING_MODEL,
                ));
            }
            if p.token_limit == 0 {
                errors.push(format!(
                    "token-maxing.providers[{i}].token_limit must be > 0"
                ));
            }
            if p.reset_window_secs == 0 {
                errors.push(format!(
                    "token-maxing.providers[{i}].reset_window_secs must be > 0"
                ));
            }
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_only_rule() {
        let cfg = TokenMaxingConfig {
            enabled: true,
            providers: vec![TokenMaxingProviderConfig {
                provider: "openai".into(),
                billing_model: "metered".into(),
                token_limit: 1_000_000,
                reset_window_secs: 3600,
                min_remaining_percent: None,
                models: vec![],
            }],
            default_min_remaining_percent: 5,
            recalibration_interval_secs: 60,
            parallel_providers: false,
        };
        assert!(!cfg.is_eligible("openai"));
        assert!(!cfg.is_eligible("zai"));
        assert!(!cfg.validate().is_empty());
    }

    #[test]
    fn subscription_passes() {
        let cfg = TokenMaxingConfig {
            enabled: true,
            providers: vec![TokenMaxingProviderConfig {
                provider: "zai".into(),
                billing_model: SUBSCRIPTION_BILLING_MODEL.into(),
                token_limit: 2_000_000,
                reset_window_secs: 18000,
                min_remaining_percent: Some(7),
                models: vec!["zai/glm-4.6".into()],
            }],
            default_min_remaining_percent: 5,
            recalibration_interval_secs: 60,
            parallel_providers: false,
        };
        assert!(cfg.is_eligible("zai"));
        assert!(!cfg.is_eligible("anthropic"));
        assert!(cfg.validate().is_empty());
        let p = cfg.get("zai").unwrap();
        assert_eq!(p.min_remaining_percent(5), 7);
    }

    #[test]
    fn empty_when_disabled_means_no_providers() {
        let cfg = TokenMaxingConfig::default();
        assert!(!cfg.enabled);
        assert!(!cfg.is_eligible("anything"));
    }
}
