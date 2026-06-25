//! Failure classification heuristics (RFC-029 §3.3).
//!
//! The strategy, in priority order:
//!
//! 1. **Downcast fast-path** — attempt to downcast to oxi-sdk structured
//!    error types. Today the typed `ProviderError` is stringified at the
//!    oxi-agent boundary, so this rarely succeeds. Kept for forward
//!    compatibility if upstream preserves the type in a future release.
//! 2. **Cause-chain Display walk** — collect every cause's Display
//!    string, then pattern-match. The chain matters because the
//!    outermost `anyhow::Error` is often a wrapper like "Agent run
//!    failed" and the actual `ProviderError` Display is buried one or
//!    two levels deep (e.g. `AgentError::RetriesExhausted { last_error }`
//!    → `last_error: String`).
//! 3. **Default** — `FailureClass::Unknown`. Conservative: the
//!    coordinator will attempt one same-model retry, then escalate.
//!
//! Patterns are matched case-insensitively against the lowercased
//! concatenated cause chain. Keep them under test (this file's
//! `#[cfg(test)]` block) and review on oxi-ai upgrades — a provider
//! changing its error wording degrades classification to Unknown, which
//! is the safe fallback.

use oxios_ouroboros::FailureClass;

/// Classify an `anyhow::Error` into a `FailureClass`.
///
/// The error is expected to come from `agent.run_streaming()` after
/// oxi-agent's internal retries are exhausted (typically as
/// `AgentError::RetriesExhausted { last_error }` or
/// `AgentError::Stream(...)`). The classification is best-effort; when
/// in doubt, returns `FailureClass::Unknown`.
pub fn classify(error: &anyhow::Error) -> FailureClass {
    // 1. Downcast fast-path (forward-compat).
    if let Some(typed) = downcast_class(error) {
        return typed;
    }

    // 2. Cause-chain Display walk.
    let chain = collect_chain(error);
    match_pattern(&chain)
}

/// Attempt to downcast the error (and its causes) to a typed variant
/// that carries an unambiguous classification signal. Returns `Some` on
/// the first match.
///
/// Today oxi-agent stringifies `ProviderError` so this almost never
/// fires. Kept so that a future upstream change preserving the type
/// automatically yields better classification.
fn downcast_class(error: &anyhow::Error) -> Option<FailureClass> {
    for cause in error.chain() {
        if let Some(sdk_err) = cause.downcast_ref::<oxi_sdk::SdkError>() {
            return Some(sdk_to_class(sdk_err));
        }
    }
    None
}

/// Map an `oxi_sdk::SdkError` to a `FailureClass` (typed fast-path).
fn sdk_to_class(err: &oxi_sdk::SdkError) -> FailureClass {
    use oxi_sdk::SdkError;
    match err {
        SdkError::ModelNotFound { .. } | SdkError::ProviderNotFound { .. } => {
            FailureClass::ModelUnavailable
        }
        SdkError::AllProvidersExhausted { .. } => FailureClass::Transient,
        SdkError::TokenBudgetExceeded { .. } | SdkError::CostBudgetExceeded { .. } => {
            FailureClass::BudgetExceeded
        }
        _ => FailureClass::Unknown,
    }
}

/// Collect the full Display text of the error and every cause,
/// joined by newlines. The whole blob is lowercased once.
fn collect_chain(error: &anyhow::Error) -> String {
    let mut buf = String::new();
    for (i, cause) in error.chain().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        buf.push_str(&cause.to_string());
    }
    buf.to_lowercase()
}

/// Match the collected cause chain (already lowercased) against the
/// classification patterns.
fn match_pattern(lower_chain: &str) -> FailureClass {
    // Order matters: check the more specific patterns first so that
    // e.g. "rate limit" doesn't pre-empt "quota" if both happen to
    // appear. In practice, providers emit one signal at a time, but
    // being explicit avoids surprises.

    if contains_any(
        lower_chain,
        &["token budget", "cost budget", "budget exceeded"],
    ) {
        return FailureClass::BudgetExceeded;
    }

    if contains_any(
        lower_chain,
        &[
            "402",
            "payment required",
            "quota",
            "insufficient_quota",
            "billing",
            "credit balance",
            "plan limit",
        ],
    ) {
        return FailureClass::QuotaExhausted;
    }

    if contains_any(
        lower_chain,
        &[
            "401",
            "403",
            "unauthorized",
            "forbidden",
            "invalid api key",
            "missing api key",
            "authentication",
            "permission denied",
            "access denied",
            "credential",
        ],
    ) {
        return FailureClass::AuthFailure;
    }

    if contains_any(
        lower_chain,
        &[
            "model not found",
            "unknown model",
            "model does not exist",
            "model deprecated",
            "model_unsupported",
        ],
    ) {
        return FailureClass::ModelUnavailable;
    }

    if contains_any(
        lower_chain,
        &[
            "context overflow",
            "context length",
            "context_length",
            "maximum context",
            "context window",
            "too many tokens",
            "prompt is too long",
        ],
    ) {
        return FailureClass::ContextOverflow;
    }

    if contains_any(
        lower_chain,
        &[
            "429",
            "too many requests",
            "rate limit",
            "rate limited",
            "rate_limit",
            "503",
            "502",
            "500",
            "service unavailable",
            "bad gateway",
            "internal server error",
            "overloaded",
            "network error",
            "connection",
            "timeout",
            "timed out", // oxi-ai ProviderError::Timeout Display
            "deadline exceeded",
            "request failed",
        ],
    ) {
        return FailureClass::Transient;
    }

    FailureClass::Unknown
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    fn err(msg: &str) -> anyhow::Error {
        anyhow!("{}", msg)
    }

    #[test]
    fn classifies_429_as_transient() {
        assert_eq!(
            classify(&err("HTTP error 429: rate limited")),
            FailureClass::Transient
        );
        assert_eq!(
            classify(&err("Rate limit exceeded (429)")),
            FailureClass::Transient
        );
    }

    #[test]
    fn classifies_5xx_as_transient() {
        assert_eq!(
            classify(&err("HTTP error 503: service unavailable")),
            FailureClass::Transient
        );
        assert_eq!(
            classify(&err("HTTP error 502: bad gateway")),
            FailureClass::Transient
        );
        assert_eq!(
            classify(&err("HTTP error 500: internal server error")),
            FailureClass::Transient
        );
        assert_eq!(
            classify(&err("provider overloaded")),
            FailureClass::Transient
        );
    }

    #[test]
    fn classifies_network_and_timeout_as_transient() {
        assert_eq!(
            classify(&err("network error: connection reset")),
            FailureClass::Transient
        );
        assert_eq!(classify(&err("request timed out")), FailureClass::Transient);
        assert_eq!(classify(&err("deadline exceeded")), FailureClass::Transient);
        assert_eq!(
            classify(&err("Request failed: dns lookup")),
            FailureClass::Transient
        );
    }

    #[test]
    fn classifies_402_as_quota() {
        assert_eq!(
            classify(&err("HTTP error 402: payment required")),
            FailureClass::QuotaExhausted
        );
        assert_eq!(
            classify(&err("insufficient_quota")),
            FailureClass::QuotaExhausted
        );
        assert_eq!(
            classify(&err("billing: credit balance is 0")),
            FailureClass::QuotaExhausted
        );
    }

    #[test]
    fn classifies_401_403_as_auth_failure() {
        assert_eq!(
            classify(&err("HTTP error 401: unauthorized")),
            FailureClass::AuthFailure
        );
        assert_eq!(
            classify(&err("HTTP error 403: forbidden")),
            FailureClass::AuthFailure
        );
        assert_eq!(classify(&err("Missing API key")), FailureClass::AuthFailure);
        assert_eq!(
            classify(&err("Invalid API key format")),
            FailureClass::AuthFailure
        );
    }

    #[test]
    fn classifies_context_overflow() {
        assert_eq!(
            classify(&err("Context overflow")),
            FailureClass::ContextOverflow
        );
        assert_eq!(
            classify(&err("context length exceeded maximum")),
            FailureClass::ContextOverflow
        );
        assert_eq!(
            classify(&err("prompt is too long for the model")),
            FailureClass::ContextOverflow
        );
    }

    #[test]
    fn classifies_model_unavailable() {
        assert_eq!(
            classify(&err("model not found: foo/bar")),
            FailureClass::ModelUnavailable
        );
        assert_eq!(
            classify(&err("unknown model 'gpt-9'")),
            FailureClass::ModelUnavailable
        );
    }

    #[test]
    fn classifies_budget_exceeded() {
        assert_eq!(
            classify(&err("token budget exceeded: 10000 / 8000")),
            FailureClass::BudgetExceeded
        );
        assert_eq!(
            classify(&err("cost budget exceeded: $0.50 / $0.40")),
            FailureClass::BudgetExceeded
        );
    }

    #[test]
    fn classifies_anything_else_as_unknown() {
        assert_eq!(
            classify(&err("something went wrong")),
            FailureClass::Unknown
        );
        assert_eq!(classify(&err("")), FailureClass::Unknown);
    }

    #[test]
    fn cause_chain_walks_buried_signals() {
        let outer = anyhow!("Failed after 3 retries: HTTP error 429: rate limited");
        assert_eq!(classify(&outer), FailureClass::Transient);

        let outer = anyhow!("Failed after 3 retries: HTTP error 401: unauthorized");
        assert_eq!(classify(&outer), FailureClass::AuthFailure);

        let outer = anyhow!("Failed after 3 retries: insufficient_quota");
        assert_eq!(classify(&outer), FailureClass::QuotaExhausted);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(
            classify(&err("HTTP ERROR 429: RATE LIMITED")),
            FailureClass::Transient
        );
        assert_eq!(
            classify(&err("Context Overflow")),
            FailureClass::ContextOverflow
        );
    }
}
