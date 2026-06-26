//! `QuotaTracker` — the per-provider availability decision layer (RFC-031 §4).
//!
//! Unifies three signals into one verdict per provider:
//!
//! 1. **Self-tracked counter (primary)** — [`ProviderBudget`] tracks tokens
//!    oxios itself sent. The base signal. Works on ZAI/Minimax today with
//!    zero endpoint work.
//! 2. **Reactive override (safety net)** — when a real request 429s or
//!    hits `QuotaExhausted` / `Transient` (which is what `classify.rs`
//!    maps rate-limit/429 to), we mark the provider `CooledDown` until
//!    `resets_at ?? now + window`. This is the drift failsafe: even if
//!    the counter still shows headroom, a 429 always wins.
//! 3. **Recalibration (accuracy upgrade)** — where a
//!    [`crate::api::quota::QuotaFetcher`] exists, periodic recalibration
//!    snaps the self-tracked counter to real provider state.
//!
//! ## Decision rule
//!
//! ```text
//! availability(p):
//!   if reactive_cooldown[p] active:    return CooledDown(cooldown[p].until)
//!   if not eligible:                    return Ineligible
//!   rem% = counter[p].remaining_percent # snapped to last recalibration
//!   if rem% <= min_remaining_percent:   return Draining
//!   return Available
//! ```

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;

use super::budget::{ProviderBudget, ProviderSnapshot};
use super::config::TokenMaxingConfig;
use crate::resilience::FailureClass;

/// Per-provider availability verdict (RFC-031 §4).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability {
    /// Provider is in the pool and has headroom. TokenMaxer may dispatch
    /// to it.
    Available {
        /// Snapshot of the self-tracked counter (snapped to last
        /// recalibration when one exists).
        snapshot: ProviderSnapshot,
        /// Per-provider floor (%). The TokenMaxer should stop draining
        /// below this — `Available` is only returned while the
        /// remaining% is **above** the floor.
        min_remaining_percent: u8,
    },
    /// Provider is in the pool but the self-tracked counter is at or
    /// below the configured floor. TokenMaxer may finish any in-flight
    /// task on this provider, but should not start a new one.
    Draining {
        snapshot: ProviderSnapshot,
        min_remaining_percent: u8,
    },
    /// Provider is in the pool but a real request just hit a 429 /
    /// `QuotaExhausted` / `Transient`. TokenMaxer MUST NOT dispatch
    /// to this provider until `until`.
    CooledDown {
        /// The expiry the TokenMaxer must wait for.
        until: DateTime<Utc>,
        /// The class that triggered the cooldown (for the report).
        reason: FailureClass,
        /// Optional snapshot if known.
        snapshot: Option<ProviderSnapshot>,
    },
    /// Provider is not in `[token-maxing.providers]` (or its entry is
    /// invalid). TokenMaxer MUST NOT touch it — this is the
    /// metered-never guard.
    Ineligible,
}

impl Availability {
    /// Whether a task may be dispatched to this provider.
    pub fn is_dispatchable(&self) -> bool {
        matches!(self, Availability::Available { .. })
    }
}

/// One recalibration event for the report.
#[derive(Debug, Clone, Serialize)]
pub struct RecalibrationRecord {
    pub provider: String,
    pub at: DateTime<Utc>,
    pub remaining_percent: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
    /// "ok" | "fetch-failed" | "no-fetcher"
    pub outcome: String,
}

/// Reactive cooldown record. The 429 failsafe.
#[derive(Debug, Clone, Serialize)]
pub struct CooldownRecord {
    pub provider: String,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub reason: FailureClass,
}

/// Snapshot of a provider's tracker state — used by the API to render
/// the live status panel.
#[derive(Debug, Clone, Serialize)]
pub struct QuotaTrackerSnapshot {
    pub provider: String,
    pub availability: Availability,
}

/// The `QuotaTracker`.
pub struct QuotaTracker {
    /// Self-tracked counter (primary signal).
    budget: ProviderBudget,
    /// Reactive cooldowns (safety net). Keyed by provider id.
    cooldowns: RwLock<HashMap<String, CooldownRecord>>,
    /// Recalibration history (capped). Used by the report and the API.
    recalibrations: RwLock<Vec<RecalibrationRecord>>,
    /// Per-provider configuration — the only place "eligibility" lives.
    config: RwLock<TokenMaxingConfig>,
}

impl QuotaTracker {
    /// Build a tracker from the user's config. Providers not eligible
    /// under the metered-never rule are dropped from the budget.
    pub fn new(config: TokenMaxingConfig) -> Self {
        let budget = ProviderBudget::from_config(&config);
        Self {
            budget,
            cooldowns: RwLock::new(HashMap::new()),
            recalibrations: RwLock::new(Vec::new()),
            config: RwLock::new(config),
        }
    }

    /// Reload the config (e.g. on `PUT /api/config`). Preserves
    /// `tokens_used` for providers that remain in the config.
    pub fn reload(&self, config: TokenMaxingConfig) {
        self.budget.reload(&config);
        *self.config.write() = config;
    }

    /// The verdict for a single provider.
    pub fn availability(&self, provider: &str) -> Availability {
        let cfg = self.config.read();
        if !cfg.is_eligible(provider) {
            return Availability::Ineligible;
        }
        // Reactive cooldown wins.
        if let Some(cd) = self.cooldowns.read().get(provider).cloned()
            && Utc::now() < cd.until
        {
            return Availability::CooledDown {
                until: cd.until,
                reason: cd.reason,
                snapshot: self.budget.snapshot(provider),
            };
        }
        // Self-tracked snapshot.
        let snapshot = match self.budget.snapshot(provider) {
            Some(s) => s,
            None => return Availability::Ineligible,
        };
        let floor = cfg
            .get(provider)
            .map(|p| p.min_remaining_percent(cfg.default_min_remaining_percent))
            .unwrap_or(cfg.default_min_remaining_percent);
        if snapshot.remaining_percent <= floor as f64 {
            Availability::Draining {
                snapshot,
                min_remaining_percent: floor,
            }
        } else {
            Availability::Available {
                snapshot,
                min_remaining_percent: floor,
            }
        }
    }

    /// All providers (eligible or not), in declared order. Used by the
    /// orchestrator to build the live status panel and the report.
    pub fn snapshots(&self) -> Vec<QuotaTrackerSnapshot> {
        let mut out = Vec::new();
        let cfg = self.config.read();
        for p in &cfg.providers {
            out.push(QuotaTrackerSnapshot {
                provider: p.provider.clone(),
                availability: self.availability(&p.provider),
            });
        }
        for (k, _) in self.budget.snapshots() {
            if !cfg.providers.iter().any(|p| p.provider == k) {
                out.push(QuotaTrackerSnapshot {
                    provider: k.clone(),
                    availability: self.availability(&k),
                });
            }
        }
        out
    }

    /// Reactive override — record a failure class for `provider` from a
    /// real agent run.
    ///
    /// Cooldown policy (kept separate so a single transient hiccup
    /// doesn't drain the pool for an hour):
    ///
    /// - `QuotaExhausted` → full reset-window cooldown. The provider's
    ///   plan limit has been hit; waiting makes sense.
    /// - `Transient` → **short** cooldown (60s by default). `classify.rs`
    ///   lumps 429/rate-limit into the same bucket as 500/timeout/
    ///   network blip. We don't want a single upstream hiccup to mark
    ///   a provider `CooledDown` for the whole reset window — that
    ///   defeats the drain-by-rotating-across-providers loop. A short
    ///   cooldown lets the next retry succeed on the same provider
    ///   once the blip clears, while still preventing hot-looping.
    /// - Anything else (auth failure, model unavailable, etc.) → no
    ///   cooldown. Those are recovery-coordinator concerns, not pool
    ///   concerns.
    pub fn record_failure(
        &self,
        provider: &str,
        class: FailureClass,
        resets_at: Option<DateTime<Utc>>,
    ) {
        if !matches!(
            class,
            FailureClass::QuotaExhausted | FailureClass::Transient
        ) {
            return;
        }
        if !self.config.read().is_eligible(provider) {
            return;
        }
        let now = Utc::now();
        let until = match (class, resets_at) {
            (FailureClass::QuotaExhausted, Some(r)) if r > now => r,
            (FailureClass::QuotaExhausted, _) => {
                // Provider's real quota is hit; wait the full reset window.
                let secs = self
                    .config
                    .read()
                    .get(provider)
                    .map(|p| p.reset_window_secs)
                    .unwrap_or(3600);
                now + chrono::Duration::seconds(secs as i64)
            }
            // Transient: short backoff so a 1-minute pause can ride out
            // a 500/timeout/network blip, and a 429 retry still has
            // room. Tunable but kept tight on purpose — if a provider
            // needs the long wait, the QuotaExhausted arm will fire
            // when its quota bucket really runs dry.
            (FailureClass::Transient, _) => now + chrono::Duration::seconds(60),
            _ => unreachable!("matches! above already filtered"),
        };
        self.cooldowns.write().insert(
            provider.to_string(),
            CooldownRecord {
                provider: provider.to_string(),
                since: now,
                until,
                reason: class,
            },
        );
    }

    /// Clear the cooldown for a provider (e.g. when a window is observed
    /// to have actually reset).
    pub fn clear_cooldown(&self, provider: &str) {
        self.cooldowns.write().remove(provider);
    }

    /// Record a recalibration event and snap the budget to the new
    /// state. Returns `true` if the snap was applied.
    pub fn apply_recalibration(
        &self,
        provider: &str,
        remaining_percent: Option<f64>,
        resets_at: Option<DateTime<Utc>>,
        outcome: RecalibrationOutcome,
    ) -> bool {
        let limit = match self.config.read().get(provider) {
            Some(p) => p.token_limit,
            None => return false,
        };
        let used = remaining_percent
            .map(|pct| {
                let remaining = (limit as f64 * pct.clamp(0.0, 100.0) / 100.0) as u64;
                limit.saturating_sub(remaining)
            })
            .unwrap_or(0);
        let applied = self.budget.recalibrate(provider, used, resets_at);
        if applied {
            // If the provider says it has reset, clear any stale cooldown.
            if let Some(r) = resets_at
                && r <= Utc::now()
            {
                self.clear_cooldown(provider);
            }
            let mut log = self.recalibrations.write();
            log.push(RecalibrationRecord {
                provider: provider.to_string(),
                at: Utc::now(),
                remaining_percent,
                resets_at,
                outcome: outcome.label().to_string(),
            });
            // Cap at 1024 entries to bound memory.
            let len = log.len();
            if len > 1024 {
                log.drain(0..(len - 1024));
            }
        }
        applied
    }

    /// Reserve tokens against the self-tracked counter. Used by the
    /// TokenMaxer after dispatching each unit of work.
    pub fn reserve(&self, provider: &str, tokens: u64) -> Result<(), super::budget::ReserveError> {
        self.budget.reserve(provider, tokens)
    }

    /// Release reserved tokens. Used on task failure / retry.
    pub fn release(&self, provider: &str, tokens: u64) {
        self.budget.release(provider, tokens)
    }

    /// The full self-tracked state for a provider. Used by the report.
    pub fn snapshot(&self, provider: &str) -> Option<ProviderSnapshot> {
        self.budget.snapshot(provider)
    }

    /// Recalibration history.
    pub fn recalibration_history(&self) -> Vec<RecalibrationRecord> {
        self.recalibrations.read().clone()
    }

    /// Cooldown records. Used by the report.
    pub fn cooldown_history(&self) -> Vec<CooldownRecord> {
        self.cooldowns.read().values().cloned().collect()
    }

    /// Read-only access to the current config.
    pub fn config(&self) -> TokenMaxingConfig {
        self.config.read().clone()
    }
}

/// Outcome of one recalibration attempt.
#[derive(Debug, Clone, Copy)]
pub enum RecalibrationOutcome {
    /// Fetcher succeeded and produced a real `remaining_percent`.
    Ok,
    /// Fetcher exists but the request failed.
    FetchFailed,
    /// No fetcher is registered for this provider.
    NoFetcher,
}

impl RecalibrationOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            RecalibrationOutcome::Ok => "ok",
            RecalibrationOutcome::FetchFailed => "fetch-failed",
            RecalibrationOutcome::NoFetcher => "no-fetcher",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::TokenMaxingProviderConfig;
    use super::*;
    use crate::resilience::FailureClass;

    fn cfg(p: &str, limit: u64, window: u64) -> TokenMaxingConfig {
        TokenMaxingConfig {
            enabled: true,
            providers: vec![TokenMaxingProviderConfig {
                provider: p.into(),
                billing_model: "subscription".into(),
                token_limit: limit,
                reset_window_secs: window,
                min_remaining_percent: Some(10),
                models: vec![],
            }],
            default_min_remaining_percent: 5,
            recalibration_interval_secs: 0,
            parallel_providers: false,
        }
    }

    #[test]
    fn ineligible_provider_returns_ineligible() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        match t.availability("anthropic") {
            Availability::Ineligible => {}
            other => panic!("expected Ineligible, got {other:?}"),
        }
    }

    #[test]
    fn fresh_provider_is_available() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        let a = t.availability("zai");
        assert!(a.is_dispatchable());
    }

    #[test]
    fn draining_when_below_floor() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        t.reserve("zai", 950).unwrap(); // 5% remaining, below 10% floor
        match t.availability("zai") {
            Availability::Draining { .. } => {}
            other => panic!("expected Draining, got {other:?}"),
        }
    }

    #[test]
    fn reactive_cooldown_wins() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        // Even though we have headroom, a 429 takes the verdict.
        t.record_failure("zai", FailureClass::Transient, None);
        match t.availability("zai") {
            Availability::CooledDown { reason, .. } => {
                assert_eq!(reason, FailureClass::Transient);
            }
            other => panic!("expected CooledDown, got {other:?}"),
        }
    }

    #[test]
    fn reactive_non_quota_failure_ignored() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        t.record_failure("zai", FailureClass::AuthFailure, None);
        // Auth failure doesn't cool the provider.
        assert!(t.availability("zai").is_dispatchable());
    }

    #[test]
    fn recalibrate_snaps_counter_and_clears_stale_cooldown() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        t.reserve("zai", 200).unwrap();
        // Tell the tracker the provider is already reset.
        let past = Utc::now() - chrono::Duration::seconds(1);
        t.apply_recalibration("zai", Some(100.0), Some(past), RecalibrationOutcome::Ok);
        let s = t.snapshot("zai").unwrap();
        assert_eq!(s.tokens_used, 0);
    }

    #[test]
    fn recalibrate_unknown_provider_returns_false() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        let ok = t.apply_recalibration("anthropic", Some(100.0), None, RecalibrationOutcome::Ok);
        assert!(!ok);
    }

    #[test]
    fn multiple_providers_snapshots() {
        let mut c = cfg("zai", 1000, 3600);
        c.providers.push(TokenMaxingProviderConfig {
            provider: "minimax".into(),
            billing_model: "subscription".into(),
            token_limit: 1000,
            reset_window_secs: 3600,
            min_remaining_percent: Some(5),
            models: vec![],
        });
        let t = QuotaTracker::new(c);
        t.reserve("zai", 200).unwrap();
        t.reserve("minimax", 800).unwrap();
        let snaps = t.snapshots();
        // Both should be Available (above their floors).
        for s in &snaps {
            assert!(matches!(s.availability, Availability::Available { .. }));
        }
    }
}
