//! Token Maxing Mode (RFC-031).
//!
//! Autonomous burn of subscription-quota providers during a user-configured
//! time window. The hard rule: **only subscription providers** are eligible —
//! metered keys can never be silently drafted into a maxing run.
//!
//! Architecture:
//!
//! - [`config`] — TOML schema (`[token-maxing]`).
//! - [`budget`] — `ProviderBudget`, the self-tracked counter. Reuses the
//!   `BudgetManager` window+reset pattern, re-keyed by provider.
//! - [`quota_tracker`] — `QuotaTracker`, the decision layer that merges
//!   self-tracked, recalibration, and reactive 429 signals into one
//!   `Availability` verdict per provider.
//! - [`planner`] — `WorkPlanner`, three-source task synthesis (autonomous
//!   skills → projects/mounts → recurring patterns). (Phase 3.)
//! - [`maxer`] — `TokenMaxer`, the drain → rotate → wait → resume loop.
//!   (Phase 3.)
//! - [`session`] — `TokenMaxingSession` + persisted report. (Phase 3.)
//!
//! The non-obvious invariant is the eligibility check: providers missing
//! from `[token-maxing.providers]` (or any with `billing_model !=
//! "subscription"`) are **never** eligible. This is the single choke point
//! that upholds the user's "절대 동작하면 안 된다" constraint.

pub mod budget;
pub mod config;
pub mod maxer;
pub mod planner;
pub mod quota_tracker;
pub mod session;

pub use budget::{ProviderBudget, ProviderSnapshot, ProviderState, ReserveError};
pub use config::{SUBSCRIPTION_BILLING_MODEL, TokenMaxingConfig, TokenMaxingProviderConfig};
pub use maxer::TokenMaxer;
pub use planner::{PlannedTask, WorkPlanner};
pub use quota_tracker::{
    Availability, CooldownRecord, QuotaTracker, QuotaTrackerSnapshot, RecalibrationOutcome,
    RecalibrationRecord,
};
pub use session::{
    MaxerStatus, MaxingStart, MaxingWindow, ProviderSessionRecord, ProviderWindowRecord,
    SessionTotals, StopReason, TaskRecord, TaskSource, TokenMaxingSession,
};
