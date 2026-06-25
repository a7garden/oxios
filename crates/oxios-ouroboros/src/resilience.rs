//! Resilience types — failure classification shared by the kernel and
//! any consumer that needs to reason about *why* an agent run failed.
//!
//! This is the result-contract side of the resilience pipeline (see
//! `docs/rfc-029-execution-resilience.md`). The classification *logic*
//! (heuristic, cause-chain walk) lives in `oxios-kernel::resilience::classify`
//! to avoid pulling kernel-only dependencies (anyhow, tracing) downstream.
//!
//! # Honest limitation
//!
//! The typed `oxi_ai::ProviderError` is stringified at the oxi-agent
//! boundary (`From<anyhow::Error>` → `AgentError::Stream(String)` /
//! `RetriesExhausted { last_error: String }`). Downcasting to
//! `ProviderError` across that boundary is **not possible** today. The
//! kernel's `classify` function therefore uses message-pattern heuristics
//! on the `Display` string. The variants here are the *contract*; the
//! patterns are tested and reviewed on oxi-ai upgrades.
//!
//! Keep this enum, its `Serialize`/`Deserialize` shape, and its serde
//! representation stable — it travels over the kernel ↔ gateway boundary
//! and will be persisted in the agent log (RFC-029 §3.6).

use serde::{Deserialize, Serialize};

/// What kind of failure occurred, and the recovery it implies.
///
/// Produced by `oxios-kernel::resilience::classify::classify` and attached
/// to [`ExecutionResult`](crate::ExecutionResult) via
/// [`ExecutionResult::failure_class`].
///
/// `None` on `ExecutionResult.failure_class` means either the run succeeded
/// or the run failed in a way that is NOT a provider/infra failure
/// (cancellation, abort, missing agent, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// 429 / 5xx / network / timeout. Same model may recover with backoff.
    Transient,
    /// 402 billing / quota exhausted. Waiting is pointless — change provider.
    QuotaExhausted,
    /// 401 / 403 invalid key. Must change provider or credential.
    AuthFailure,
    /// Context window exceeded. Compact, or switch to larger-context model.
    ContextOverflow,
    /// Model not found / deprecated / unsupported. Must switch model.
    ModelUnavailable,
    /// Token or cost budget hit — a policy limit, not a provider fault.
    BudgetExceeded,
    /// Unclassified. Conservative: one same-model retry, then escalate.
    Unknown,
}

impl FailureClass {
    /// Does waiting + retrying the SAME model have a chance of working?
    ///
    /// Used by the recovery coordinator (RFC-029 §3.4 L1) to decide
    /// whether to attempt a same-model backoff before model/provider swap.
    pub fn benefits_from_same_model_retry(&self) -> bool {
        matches!(self, Self::Transient | Self::Unknown)
    }

    /// Must we switch provider (waiting on the same one won't help)?
    ///
    /// Quota and auth failures cannot be fixed by waiting on the same
    /// provider — they need a different credential/account.
    pub fn requires_provider_swap(&self) -> bool {
        matches!(self, Self::QuotaExhausted | Self::AuthFailure)
    }

    /// Should the recovery coordinator skip a from-scratch retry and
    /// go straight to model/provider swap? Used for failures that won't
    /// recover with the same model even with a fresh agent (rate limit,
    /// quota, auth, missing model).
    pub fn should_skip_same_model_retry(&self) -> bool {
        matches!(
            self,
            Self::QuotaExhausted
                | Self::AuthFailure
                | Self::ModelUnavailable
                | Self::BudgetExceeded
        )
    }
}

impl std::fmt::Display for FailureClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Transient => "transient",
            Self::QuotaExhausted => "quota_exhausted",
            Self::AuthFailure => "auth_failure",
            Self::ContextOverflow => "context_overflow",
            Self::ModelUnavailable => "model_unavailable",
            Self::BudgetExceeded => "budget_exceeded",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_provider_swap_matches_only_quota_and_auth() {
        assert!(FailureClass::QuotaExhausted.requires_provider_swap());
        assert!(FailureClass::AuthFailure.requires_provider_swap());
        assert!(!FailureClass::Transient.requires_provider_swap());
        assert!(!FailureClass::ContextOverflow.requires_provider_swap());
        assert!(!FailureClass::ModelUnavailable.requires_provider_swap());
        assert!(!FailureClass::BudgetExceeded.requires_provider_swap());
        assert!(!FailureClass::Unknown.requires_provider_swap());
    }

    #[test]
    fn benefits_from_same_model_retry_matches_transient_and_unknown() {
        assert!(FailureClass::Transient.benefits_from_same_model_retry());
        assert!(FailureClass::Unknown.benefits_from_same_model_retry());
        for v in [
            FailureClass::QuotaExhausted,
            FailureClass::AuthFailure,
            FailureClass::ContextOverflow,
            FailureClass::ModelUnavailable,
            FailureClass::BudgetExceeded,
        ] {
            assert!(
                !v.benefits_from_same_model_retry(),
                "{v} should not benefit from same-model retry"
            );
        }
    }

    #[test]
    fn serde_roundtrip() {
        for v in [
            FailureClass::Transient,
            FailureClass::QuotaExhausted,
            FailureClass::AuthFailure,
            FailureClass::ContextOverflow,
            FailureClass::ModelUnavailable,
            FailureClass::BudgetExceeded,
            FailureClass::Unknown,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: FailureClass = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back, "roundtrip failed for {v}");
        }
    }

    #[test]
    fn display_is_snake_case() {
        assert_eq!(FailureClass::Transient.to_string(), "transient");
        assert_eq!(FailureClass::QuotaExhausted.to_string(), "quota_exhausted");
        assert_eq!(
            FailureClass::ContextOverflow.to_string(),
            "context_overflow"
        );
        assert_eq!(
            FailureClass::ModelUnavailable.to_string(),
            "model_unavailable"
        );
        assert_eq!(FailureClass::BudgetExceeded.to_string(), "budget_exceeded");
        assert_eq!(FailureClass::AuthFailure.to_string(), "auth_failure");
        assert_eq!(FailureClass::Unknown.to_string(), "unknown");
    }
}
