//! `QuotaTracker` — the per-provider availability decision layer
//! (RFC-031 v2 §4).
//!
//! Unifies three signals into one verdict per provider:
//!
//! 1. **Live quota snapshot (v2 primary)** — when a
//!    [`crate::api::quota::QuotaFetcher`] exists for the provider,
//!    its most recent [`super::live_quota::QuotaSnapshot`] is the
//!    authoritative signal for both **eligibility** and
//!    `remaining_percent`. Auto-discovers providers that the user
//!    has not added to `[[token-maxing.providers]]`.
//! 2. **Self-tracked counter (fallback)** — [`ProviderBudget`]
//!    tracks tokens oxios itself sent. Used when no live fetcher
//!    exists, or as a consistency check between polls.
//! 3. **Reactive cooldown (safety net)** — when a real request 429s
//!    or hits `QuotaExhausted` / `Transient`, we mark the provider
//!    `CooledDown` until `resets_at ?? now + window`. Drift failsafe.
//!
//! ## Decision rule (v2)
//!
//! ```text
//! availability(p):
//!   if reactive_cooldown[p] active:    return CooledDown(...)
//!   if live_snapshot(p) exists:
//!     if plan_type == Metered:         return Ineligible  // metered-never
//!     if plan_type == Subscription && usable signal:
//!       rem% = live.remaining_percent
//!       if rem% <= min_remaining_percent: return Draining
//!       return Available
//!   // v1 fallback: config gate + self-tracked counter.
//!   if !cfg.is_eligible(p):            return Ineligible
//!   rem% = counter[p].remaining_percent
//!   ...
//! ```

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;

use super::budget::{ProviderBudget, ProviderSnapshot};
use super::config::TokenMaxingConfig;
use super::live_quota::{PlanType, QuotaSnapshot};
use crate::resilience::FailureClass;

/// Per-provider availability verdict (RFC-031 §4).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability {
    Available {
        snapshot: ProviderSnapshot,
        min_remaining_percent: u8,
    },
    Draining {
        snapshot: ProviderSnapshot,
        min_remaining_percent: u8,
    },
    CooledDown {
        until: DateTime<Utc>,
        reason: FailureClass,
        snapshot: Option<ProviderSnapshot>,
    },
    Ineligible,
}

impl Availability {
    pub fn is_dispatchable(&self) -> bool {
        matches!(self, Availability::Available { .. })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RecalibrationRecord {
    pub provider: String,
    pub at: DateTime<Utc>,
    pub remaining_percent: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CooldownRecord {
    pub provider: String,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub reason: FailureClass,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuotaTrackerSnapshot {
    pub provider: String,
    pub availability: Availability,
}

/// The `QuotaTracker`.
pub struct QuotaTracker {
    budget: ProviderBudget,
    cooldowns: RwLock<HashMap<String, CooldownRecord>>,
    recalibrations: RwLock<Vec<RecalibrationRecord>>,
    config: RwLock<TokenMaxingConfig>,
    /// v2: live quota snapshots from `QuotaFetcher`s, keyed by
    /// provider. A `Subscription` snapshot makes the provider
    /// eligible for token maxing regardless of `[[token-maxing.providers]]`.
    live_snapshots: RwLock<HashMap<String, QuotaSnapshot>>,
}

impl QuotaTracker {
    pub fn new(config: TokenMaxingConfig) -> Self {
        let budget = ProviderBudget::from_config(&config);
        Self {
            budget,
            cooldowns: RwLock::new(HashMap::new()),
            recalibrations: RwLock::new(Vec::new()),
            config: RwLock::new(config),
            live_snapshots: RwLock::new(HashMap::new()),
        }
    }

    pub fn reload(&self, config: TokenMaxingConfig) {
        self.budget.reload(&config);
        *self.config.write() = config;
    }

    /// Store the most recent live `QuotaSnapshot` for `provider`.
    pub fn update_live_snapshot(&self, snapshot: QuotaSnapshot) {
        self.live_snapshots
            .write()
            .insert(snapshot.provider.clone(), snapshot);
    }

    pub fn live_snapshot(&self, provider: &str) -> Option<QuotaSnapshot> {
        self.live_snapshots.read().get(provider).cloned()
    }

    pub fn live_providers(&self) -> Vec<String> {
        self.live_snapshots.read().keys().cloned().collect()
    }

    /// `Some(true)` = subscription, `Some(false)` = metered (or
    /// unknown), `None` = no live data.
    pub fn live_eligible(&self, provider: &str) -> Option<bool> {
        self.live_snapshots
            .read()
            .get(provider)
            .map(|s| matches!(s.plan_type, PlanType::Subscription))
    }

    /// Build a `ProviderSnapshot` from a live `QuotaSnapshot` so the
    /// existing `Available`/`Draining` variant payload shape is
    /// preserved.
    fn snapshot_from_live(
        live: &QuotaSnapshot,
        rem_pct: Option<f64>,
        resets_at: Option<DateTime<Utc>>,
        floor: u8,
    ) -> ProviderSnapshot {
        let token_limit = live
            .token_limit
            .and_then(|l| if l > 0.0 { Some(l as u64) } else { None })
            .or_else(|| {
                live.rate_windows.iter().find_map(|w| {
                    w.limit
                        .and_then(|l| if l > 0.0 { Some(l as u64) } else { None })
                })
            })
            .unwrap_or(0);
        let remaining_percent = rem_pct.unwrap_or(100.0);
        let tokens_used = if token_limit > 0 {
            let remaining = (token_limit as f64 * remaining_percent / 100.0) as u64;
            token_limit.saturating_sub(remaining)
        } else {
            0
        };
        ProviderSnapshot::from_parts(&live.provider, tokens_used, token_limit, resets_at, floor)
    }

    /// The verdict for a single provider (v2 — live snapshot first).
    pub fn availability(&self, provider: &str) -> Availability {
        // Reactive cooldown wins (drift failsafe).
        if let Some(cd) = self.cooldowns.read().get(provider).cloned()
            && Utc::now() < cd.until
        {
            return Availability::CooledDown {
                until: cd.until,
                reason: cd.reason,
                snapshot: self.budget.snapshot(provider),
            };
        }

        // v2: live snapshot is the primary signal.
        if let Some(snap) = self.live_snapshot(provider) {
            // metered-never guard: live Metered response excludes
            // even if the user marked it `subscription` in TOML.
            if matches!(snap.plan_type, PlanType::Metered) {
                return Availability::Ineligible;
            }
            if matches!(snap.plan_type, PlanType::Subscription) && snap.is_subscription_signal() {
                let rem_pct = snap.best_remaining_percent();
                let resets_at = snap.best_resets_at();
                let floor = self.config.read().default_min_remaining_percent;
                let s = Self::snapshot_from_live(&snap, rem_pct, resets_at, floor);
                if rem_pct.unwrap_or(100.0) <= floor as f64 {
                    return Availability::Draining {
                        snapshot: s,
                        min_remaining_percent: floor,
                    };
                }
                return Availability::Available {
                    snapshot: s,
                    min_remaining_percent: floor,
                };
            }
        }

        // v1 fallback: config gate + self-tracked counter.
        let cfg = self.config.read();
        if !cfg.is_eligible(provider) {
            return Availability::Ineligible;
        }
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

    /// All providers (eligible or not). v2 includes config entries,
    /// self-tracked budget providers, AND auto-discovered live
    /// providers.
    pub fn snapshots(&self) -> Vec<QuotaTrackerSnapshot> {
        let mut out = Vec::new();
        let cfg = self.config.read();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for p in &cfg.providers {
            seen.insert(p.provider.clone());
            out.push(QuotaTrackerSnapshot {
                provider: p.provider.clone(),
                availability: self.availability(&p.provider),
            });
        }
        for (k, _) in self.budget.snapshots() {
            if seen.insert(k.clone()) {
                out.push(QuotaTrackerSnapshot {
                    provider: k.clone(),
                    availability: self.availability(&k),
                });
            }
        }
        for p in self.live_providers() {
            if seen.insert(p.clone()) {
                out.push(QuotaTrackerSnapshot {
                    provider: p.clone(),
                    availability: self.availability(&p),
                });
            }
        }
        out
    }

    /// Reactive override — record a failure class.
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
        if !self.config.read().is_eligible(provider)
            && !matches!(self.live_eligible(provider), Some(true))
        {
            return;
        }
        let now = Utc::now();
        let until = match (class, resets_at) {
            (FailureClass::QuotaExhausted, Some(r)) if r > now => r,
            (FailureClass::QuotaExhausted, _) => {
                let secs = self
                    .config
                    .read()
                    .get(provider)
                    .map(|p| p.reset_window_secs)
                    .unwrap_or(3600);
                now + chrono::Duration::seconds(secs as i64)
            }
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

    pub fn clear_cooldown(&self, provider: &str) {
        self.cooldowns.write().remove(provider);
    }

    /// v2: takes `token_limit: Option<u64>` from the live response
    /// (replaces the v1 config lookup). When `None`, falls back to
    /// `[[token-maxing.providers]]` config.
    pub fn apply_recalibration(
        &self,
        provider: &str,
        remaining_percent: Option<f64>,
        resets_at: Option<DateTime<Utc>>,
        token_limit: Option<u64>,
        outcome: RecalibrationOutcome,
    ) -> bool {
        let limit: u64 = match token_limit {
            Some(l) if l > 0 => l,
            _ => match self.config.read().get(provider) {
                Some(p) => p.token_limit,
                None => 0,
            },
        };
        let used = if limit > 0 {
            remaining_percent
                .map(|pct| {
                    let remaining = (limit as f64 * pct.clamp(0.0, 100.0) / 100.0) as u64;
                    limit.saturating_sub(remaining)
                })
                .unwrap_or(0)
        } else {
            0
        };
        let applied = self.budget.recalibrate(provider, used, resets_at);
        if applied
            && let Some(r) = resets_at
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
        let len = log.len();
        if len > 1024 {
            log.drain(0..(len - 1024));
        }
        applied
    }

    pub fn reserve(&self, provider: &str, tokens: u64) -> Result<(), super::budget::ReserveError> {
        self.budget.reserve(provider, tokens)
    }

    pub fn release(&self, provider: &str, tokens: u64) {
        self.budget.release(provider, tokens)
    }

    pub fn snapshot(&self, provider: &str) -> Option<ProviderSnapshot> {
        self.budget.snapshot(provider)
    }

    pub fn recalibration_history(&self) -> Vec<RecalibrationRecord> {
        self.recalibrations.read().clone()
    }

    pub fn cooldown_history(&self) -> Vec<CooldownRecord> {
        self.cooldowns.read().values().cloned().collect()
    }

    pub fn config(&self) -> TokenMaxingConfig {
        self.config.read().clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RecalibrationOutcome {
    Ok,
    FetchFailed,
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
        assert!(t.availability("zai").is_dispatchable());
    }

    #[test]
    fn draining_when_below_floor() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        t.reserve("zai", 950).unwrap();
        match t.availability("zai") {
            Availability::Draining { .. } => {}
            other => panic!("expected Draining, got {other:?}"),
        }
    }

    #[test]
    fn reactive_cooldown_wins() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
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
        assert!(t.availability("zai").is_dispatchable());
    }

    #[test]
    fn recalibrate_snaps_counter_and_clears_stale_cooldown() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        t.reserve("zai", 200).unwrap();
        let past = Utc::now() - chrono::Duration::seconds(1);
        t.apply_recalibration(
            "zai",
            Some(100.0),
            Some(past),
            None,
            RecalibrationOutcome::Ok,
        );
        assert_eq!(t.snapshot("zai").unwrap().tokens_used, 0);
    }

    #[test]
    fn recalibrate_unknown_provider_returns_false() {
        let t = QuotaTracker::new(cfg("zai", 1000, 3600));
        let ok = t.apply_recalibration(
            "anthropic",
            Some(100.0),
            None,
            None,
            RecalibrationOutcome::Ok,
        );
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
        for s in &t.snapshots() {
            assert!(matches!(s.availability, Availability::Available { .. }));
        }
    }

    /// v2: a live Subscription snapshot makes a provider eligible
    /// even with no `[[token-maxing.providers]]` entry — this is
    /// the user's actual bug fix.
    #[test]
    fn live_subscription_snapshot_auto_eligibility() {
        let t = QuotaTracker::new(TokenMaxingConfig {
            enabled: true,
            providers: vec![],
            default_min_remaining_percent: 5,
            recalibration_interval_secs: 60,
            parallel_providers: false,
        });
        // No config entry for zai — v1 would return Ineligible.
        assert!(matches!(t.availability("zai"), Availability::Ineligible));
        // Cache a live Subscription snapshot. v2: zai becomes Available.
        t.update_live_snapshot(QuotaSnapshot {
            provider: "zai".into(),
            plan: Some("coding-plan".into()),
            plan_type: PlanType::Subscription,
            token_limit: Some(2_000_000.0),
            rate_windows: vec![super::super::live_quota::RateWindow {
                name: "5h".into(),
                used: Some(500_000.0),
                limit: Some(2_000_000.0),
                remaining_percent: Some(75.0),
                resets_at: None,
            }],
            fetched_at: Utc::now(),
            error: None,
        });
        match t.availability("zai") {
            Availability::Available { .. } => {}
            other => panic!("expected Available from live snapshot, got {other:?}"),
        }
    }

    /// v2: a live Metered snapshot returns Ineligible (metered-never
    /// guard). Even if the user marked it `subscription` in TOML.
    #[test]
    fn live_metered_snapshot_excludes_even_with_config() {
        let t = QuotaTracker::new(cfg("openai", 1000, 3600));
        t.update_live_snapshot(QuotaSnapshot {
            provider: "openai".into(),
            plan: None,
            plan_type: PlanType::Metered,
            token_limit: None,
            rate_windows: vec![],
            fetched_at: Utc::now(),
            error: None,
        });
        assert!(matches!(t.availability("openai"), Availability::Ineligible));
    }
}
