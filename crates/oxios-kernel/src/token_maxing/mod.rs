//! Token Maxing Mode (RFC-031 v2).
//!
//! Autonomous burn of subscription-quota providers during a user-configured
//! time window. v2 derives eligibility from live quota API responses
//! instead of the v1 `[[token-maxing.providers]]` opt-in block.
//!
//! Architecture:
//!
//! - [`config`] — TOML schema (`[token-maxing]`). The v1
//!   `[[token-maxing.providers]]` block is now optional — providers
//!   are auto-discovered when a live subscription snapshot exists.
//! - [`budget`] — `ProviderBudget`, the self-tracked counter. Fallback
//!   when no live quota API exists.
//! - [`live_quota`] — kernel-side `QuotaSnapshot`/`PlanType`/
//!   `QuotaFetcher` trait. The v2 eligibility signal #1.
//! - [`quota_tracker`] — `QuotaTracker`, the decision layer. v2 reads
//!   live snapshots first; v1 config gate is the fallback.
//! - [`planner`], [`maxer`], [`session`] — Phase 3.

pub mod budget;
pub mod config;
pub mod live_quota;
pub mod maxer;
pub mod planner;
pub mod quota_tracker;
pub mod session;

pub use budget::{ProviderBudget, ProviderSnapshot, ProviderState, ReserveError};
pub use config::{SUBSCRIPTION_BILLING_MODEL, TokenMaxingConfig, TokenMaxingProviderConfig};
pub use live_quota::{PlanType, QuotaFetcher, QuotaSnapshot, RateWindow};
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
