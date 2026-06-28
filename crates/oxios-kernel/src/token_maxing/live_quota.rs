//! Live quota snapshot types — the v2 signal #3 data source for
//! `QuotaTracker` (RFC-031 v2 §4).
//!
//! These types live in the **kernel** crate so `QuotaTracker` can use
//! them without taking a dependency on the binary crate's
//! `crate::api::quota` module (AGENTS.md §10: "Star topology, no
//! circular deps"). The binary crate implements [`QuotaFetcher`] for
//! each provider and reuses these types directly via
//! `oxios_kernel::token_maxing::live_quota::*`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Billing-model classification returned by a provider's quota API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlanType {
    /// Reset-window allocation (Coding Plan, Pro, etc.) — maxable.
    Subscription,
    /// Pay-per-token — excluded from token maxing.
    Metered,
    /// No live data yet (fetcher never ran or returned an error).
    #[default]
    Unknown,
}

/// A rate-limit / quota window with optional reset time.
#[derive(Debug, Clone, Serialize)]
pub struct RateWindow {
    pub name: String,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
}

/// Snapshot of a provider account's quota/balance state.
#[derive(Debug, Clone, Serialize)]
pub struct QuotaSnapshot {
    pub provider: String,
    pub plan: Option<String>,
    #[serde(default)]
    pub plan_type: PlanType,
    #[serde(default)]
    pub token_limit: Option<f64>,
    pub rate_windows: Vec<RateWindow>,
    pub fetched_at: DateTime<Utc>,
    pub error: Option<String>,
}

impl QuotaSnapshot {
    pub fn blank(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            plan: None,
            plan_type: PlanType::Unknown,
            token_limit: None,
            rate_windows: Vec::new(),
            fetched_at: Utc::now(),
            error: None,
        }
    }

    pub fn is_subscription_signal(&self) -> bool {
        if self.error.is_some() || self.plan_type != PlanType::Subscription {
            return false;
        }
        self.rate_windows
            .iter()
            .any(|w| w.remaining_percent.is_some() || w.resets_at.is_some())
    }

    pub fn best_remaining_percent(&self) -> Option<f64> {
        self.rate_windows
            .iter()
            .filter_map(|w| w.remaining_percent)
            .next()
    }

    pub fn best_resets_at(&self) -> Option<DateTime<Utc>> {
        self.rate_windows.iter().filter_map(|w| w.resets_at).next()
    }
}

/// Fetches account-level quota/balance from a provider's API.
#[async_trait]
pub trait QuotaFetcher: Send + Sync {
    fn provider(&self) -> &str;
    fn has_credentials(&self, api_key: Option<&str>) -> bool {
        api_key.is_some_and(|k| !k.is_empty())
    }
    async fn fetch(&self, api_key: Option<&str>) -> anyhow::Result<QuotaSnapshot>;
}
