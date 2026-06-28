//! Self-tracked per-provider budget (RFC-031 §4, Phase 1).
//!
//! Reuses the [`crate::budget::BudgetManager`] sliding-window+reset pattern
//! but re-keyed from `AgentId` to provider id. The cap is the subscription
//! plan allocation; the reset window is the plan's reset cadence.
//!
//! ## Why self-tracked
//!
//! ZAI / Minimax may not expose a `remaining_percent` / `resets_at` field on
//! their usage APIs. The self-tracked counter is the **primary** mechanism
//! precisely because it never depends on those fields. The
//! [`crate::token_maxing::quota_tracker::QuotaTracker`] layers
//! recalibration (where an endpoint exists) and reactive 429 cooldown
//! on top.
//!
//! ## Drift
//!
//! The counter only sees tokens oxios itself sent. If the same API key is
//! used by another app/instance, the counter under-counts. That's why
//! `QuotaTracker` recalibrates from real provider state when available,
//! and falls back to a 429 cooldown when a real request hits a hard limit.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::TokenMaxingConfig;

/// Per-provider state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderState {
    /// Plan allocation per window.
    pub token_limit: u64,
    /// Window length (seconds).
    pub window_secs: u64,
    /// Tokens consumed in the current window.
    pub tokens_used: u64,
    /// When the current window started.
    pub window_start: DateTime<Utc>,
    /// When the window actually resets, if the provider has ever given us
    /// a real `resets_at` (via recalibration). When `Some`, it overrides
    /// the sliding-window reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<DateTime<Utc>>,
}

impl ProviderState {
    fn new(token_limit: u64, window_secs: u64, now: DateTime<Utc>) -> Self {
        Self {
            token_limit,
            window_secs,
            tokens_used: 0,
            window_start: now,
            resets_at: None,
        }
    }

    /// Whether the sliding window has expired.
    fn window_expired(&self, now: DateTime<Utc>) -> bool {
        now.signed_duration_since(self.window_start) >= Duration::seconds(self.window_secs as i64)
    }

    /// Reset the counter, honoring `resets_at` if present.
    fn reset(&mut self, now: DateTime<Utc>) {
        self.tokens_used = 0;
        self.window_start = now;
        if let Some(r) = self.resets_at
            && r <= now
        {
            self.resets_at = None;
        }
    }
}

/// Per-provider budget manager.
///
/// One [`ProviderBudget`] per kernel. The map is keyed by provider id (the
/// same string the engine uses, e.g. `"zai"`).
pub struct ProviderBudget {
    states: RwLock<HashMap<String, ProviderState>>,
}

impl Default for ProviderBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderBudget {
    /// Create an empty budget. Use [`Self::from_config`] to populate from
    /// the user's `[token-maxing]` config.
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Build from the user's config. Providers not eligible under the
    /// metered-never rule are silently skipped — the caller should have
    /// validated the config first.
    pub fn from_config(config: &TokenMaxingConfig) -> Self {
        let now = Utc::now();
        let mut states = HashMap::new();
        for p in &config.providers {
            if !config.is_eligible(&p.provider) {
                continue;
            }
            states.insert(
                p.provider.clone(),
                ProviderState::new(p.token_limit, p.reset_window_secs, now),
            );
        }
        Self {
            states: RwLock::new(states),
        }
    }

    /// Reload the configured providers while preserving existing usage
    /// counters. New providers start fresh at 0; removed providers are
    /// dropped; existing providers keep their `tokens_used` and window.
    pub fn reload(&self, config: &TokenMaxingConfig) {
        let now = Utc::now();
        let mut states = self.states.write();
        let eligible_ids: Vec<String> = config
            .providers
            .iter()
            .filter(|p| config.is_eligible(&p.provider))
            .map(|p| p.provider.clone())
            .collect();
        // Drop providers that disappeared from config.
        states.retain(|id, _| eligible_ids.contains(id));
        // Insert new providers.
        for p in &config.providers {
            if !config.is_eligible(&p.provider) {
                continue;
            }
            states
                .entry(p.provider.clone())
                .or_insert_with(|| ProviderState::new(p.token_limit, p.reset_window_secs, now));
        }
    }

    /// Reserve `tokens` against `provider`'s budget.
    ///
    /// `Ok(())` if the reservation fits; `Err` otherwise. The reserve
    /// here is the second line of defense — the primary gate is the
    /// availability verdict before the agent is dispatched.
    pub fn reserve(&self, provider: &str, tokens: u64) -> Result<(), ReserveError> {
        let now = Utc::now();
        let mut states = self.states.write();
        let state = states
            .get_mut(provider)
            .ok_or_else(|| ReserveError::UnknownProvider(provider.to_string()))?;

        if state.window_expired(now) {
            state.reset(now);
        }

        if state.tokens_used + tokens > state.token_limit {
            let remaining = state.token_limit.saturating_sub(state.tokens_used);
            return Err(ReserveError::Exceeded {
                limit: state.token_limit,
                used: state.tokens_used,
                requested: tokens,
                remaining,
            });
        }
        state.tokens_used += tokens;
        Ok(())
    }

    /// Release `tokens` back. Used on error/retry to free headroom.
    pub fn release(&self, provider: &str, tokens: u64) {
        let mut states = self.states.write();
        if let Some(state) = states.get_mut(provider) {
            state.tokens_used = state.tokens_used.saturating_sub(tokens);
        }
    }

    /// Read a snapshot of a provider's state, applying the reset-if-expired
    /// rule for callers that need an up-to-the-moment view.
    pub fn snapshot(&self, provider: &str) -> Option<ProviderSnapshot> {
        let now = Utc::now();
        let mut states = self.states.write();
        let state = states.get_mut(provider)?;
        if state.window_expired(now) {
            state.reset(now);
        }
        Some(ProviderSnapshot::from(&*state))
    }

    /// All provider states, snapshotted. Used by the `QuotaTracker`.
    pub fn snapshots(&self) -> Vec<(String, ProviderSnapshot)> {
        let now = Utc::now();
        let mut states = self.states.write();
        states
            .iter_mut()
            .map(|(k, v)| {
                if v.window_expired(now) {
                    v.reset(now);
                }
                (k.clone(), ProviderSnapshot::from(&*v))
            })
            .collect()
    }

    /// Snap a state's `tokens_used` to a real value, then close the
    /// window. Called by `QuotaTracker` when a fetcher returns
    /// `remaining_percent` / `resets_at`.
    ///
    /// `used_after_calibration` is what the counter should show right
    /// after the snap (i.e. `token_limit - remaining`).
    pub fn recalibrate(
        &self,
        provider: &str,
        used_after_calibration: u64,
        resets_at: Option<DateTime<Utc>>,
    ) -> bool {
        let now = Utc::now();
        let mut states = self.states.write();
        if let Some(state) = states.get_mut(provider) {
            state.tokens_used = used_after_calibration.min(state.token_limit);
            state.window_start = now;
            state.resets_at = resets_at;
            true
        } else {
            false
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
/// Read-only view of a [`ProviderState`] at a moment in time.
pub struct ProviderSnapshot {
    pub token_limit: u64,
    pub window_secs: u64,
    pub tokens_used: u64,
    pub tokens_remaining: u64,
    pub remaining_percent: f64,
    pub window_start: DateTime<Utc>,
    pub resets_at: Option<DateTime<Utc>>,
    /// Seconds until the window resets. Honours `resets_at` when present.
    pub window_remaining_secs: u64,
}

impl From<&ProviderState> for ProviderSnapshot {
    fn from(s: &ProviderState) -> Self {
        let tokens_remaining = s.token_limit.saturating_sub(s.tokens_used);
        let remaining_percent = if s.token_limit == 0 {
            0.0
        } else {
            (tokens_remaining as f64 / s.token_limit as f64) * 100.0
        };
        let window_remaining_secs = match s.resets_at {
            Some(r) => (r - Utc::now()).num_seconds().max(0) as u64,
            None => {
                let elapsed = (Utc::now() - s.window_start).num_seconds().max(0) as u64;
                s.window_secs.saturating_sub(elapsed)
            }
        };
        Self {
            token_limit: s.token_limit,
            window_secs: s.window_secs,
            tokens_used: s.tokens_used,
            tokens_remaining,
            remaining_percent,
            window_start: s.window_start,
            resets_at: s.resets_at,
            window_remaining_secs,
        }
    }
}

impl ProviderSnapshot {
    /// Build a snapshot directly from parts — used by the v2 live
    /// quota path. `window_secs` defaults to 1h as a placeholder
    /// when no live `resets_at` is known.
    pub fn from_parts(
        _provider: &str,
        tokens_used: u64,
        token_limit: u64,
        resets_at: Option<DateTime<Utc>>,
        _floor_percent: u8,
    ) -> Self {
        let tokens_remaining = token_limit.saturating_sub(tokens_used);
        let remaining_percent = if token_limit == 0 {
            0.0
        } else {
            (tokens_remaining as f64 / token_limit as f64) * 100.0
        };
        let now = Utc::now();
        let window_remaining_secs = match resets_at {
            Some(r) => (r - now).num_seconds().max(0) as u64,
            None => 3600,
        };
        Self {
            token_limit,
            window_secs: 3600,
            tokens_used,
            tokens_remaining,
            remaining_percent,
            window_start: now,
            resets_at,
            window_remaining_secs,
        }
    }
}

 /// Errors from [`ProviderBudget::reserve`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum ReserveError {
    /// Provider has no budget configured (ineligible).
    #[error("no budget configured for provider '{0}'")]
    UnknownProvider(String),
    /// Requested tokens would push the counter past the plan limit.
    #[error(
        "token limit exceeded: requested {requested} but only {remaining} remaining (limit {limit})"
    )]
    Exceeded {
        limit: u64,
        used: u64,
        requested: u64,
        /// Convenience field — equal to `limit - used` at error time.
        /// thiserror can format it directly without a helper.
        remaining: u64,
    },
}

impl ReserveError {
    /// Tokens still available in the current window. `0` for
    /// `UnknownProvider`.
    pub fn remaining(&self) -> u64 {
        match self {
            ReserveError::UnknownProvider(_) => 0,
            ReserveError::Exceeded { remaining, .. } => *remaining,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::TokenMaxingProviderConfig;
    use super::*;

    fn cfg_with(provider: &str, limit: u64, window: u64) -> TokenMaxingConfig {
        TokenMaxingConfig {
            enabled: true,
            providers: vec![TokenMaxingProviderConfig {
                provider: provider.into(),
                billing_model: "subscription".into(),
                token_limit: limit,
                reset_window_secs: window,
                min_remaining_percent: None,
                models: vec![],
            }],
            default_min_remaining_percent: 5,
            recalibration_interval_secs: 0,
            parallel_providers: false,
        }
    }

    #[test]
    fn reserve_within_budget_succeeds() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        assert!(b.reserve("zai", 500).is_ok());
        let s = b.snapshot("zai").unwrap();
        assert_eq!(s.tokens_used, 500);
        assert_eq!(s.tokens_remaining, 500);
    }

    #[test]
    fn reserve_exceeds_budget() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        b.reserve("zai", 1000).unwrap();
        let err = b.reserve("zai", 1).unwrap_err();
        match err {
            ReserveError::Exceeded {
                limit,
                used,
                requested,
                ..
            } => {
                assert_eq!(limit, 1000);
                assert_eq!(used, 1000);
                assert_eq!(requested, 1);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn unknown_provider_rejected() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        assert!(matches!(
            b.reserve("anthropic", 1).unwrap_err(),
            ReserveError::UnknownProvider(_)
        ));
    }

    #[test]
    fn release_returns_tokens() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        b.reserve("zai", 800).unwrap();
        b.release("zai", 300);
        let s = b.snapshot("zai").unwrap();
        assert_eq!(s.tokens_used, 500);
    }

    #[test]
    fn recalibrate_snaps_counter() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        b.reserve("zai", 700).unwrap();
        let r = b.recalibrate("zai", 850, None);
        assert!(r);
        let s = b.snapshot("zai").unwrap();
        assert_eq!(s.tokens_used, 850);
    }

    #[test]
    fn reload_preserves_existing_usage() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        b.reserve("zai", 400).unwrap();
        b.reload(&cfg_with("zai", 1000, 3600));
        let s = b.snapshot("zai").unwrap();
        assert_eq!(s.tokens_used, 400);
    }

    #[test]
    fn reload_drops_removed_providers() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        let new_cfg = TokenMaxingConfig {
            enabled: true,
            ..TokenMaxingConfig::default()
        };
        b.reload(&new_cfg);
        assert!(b.snapshot("zai").is_none());
    }

    #[test]
    fn eligibility_filter_blocks_metered() {
        let mut cfg = cfg_with("openai", 1000, 3600);
        cfg.providers[0].billing_model = "metered".into();
        let b = ProviderBudget::from_config(&cfg);
        assert!(b.snapshot("openai").is_none());
        assert!(matches!(
            b.reserve("openai", 1).unwrap_err(),
            ReserveError::UnknownProvider(_)
        ));
    }

    #[test]
    fn remaining_percent_computed() {
        let b = ProviderBudget::from_config(&cfg_with("zai", 1000, 3600));
        b.reserve("zai", 250).unwrap();
        let s = b.snapshot("zai").unwrap();
        assert!((s.remaining_percent - 75.0).abs() < 1e-9);
    }
}
