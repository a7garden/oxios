//! Per-provider circuit breaker — `ProviderHealthRegistry` (RFC-029 §3.6).
//!
//! Unlike the existing global `LLM_CIRCUIT_BREAKER` (which only records
//! metrics, never gates), this registry tracks each provider's health
//! independently. When a provider trips (e.g. 5 consecutive failures),
//! the recovery coordinator skips every model on that provider in the
//! fallback chain and jumps to the next healthy one.
//!
//! Half-open semantics: after `reset_after` seconds, one test request is
//! allowed. If it succeeds, the provider returns to healthy; if it fails,
//! the timeout resets.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Configuration for per-provider circuit breakers.
#[derive(Debug, Clone)]
pub struct BreakerConfig {
    /// Number of consecutive failures before the breaker opens.
    pub failure_threshold: u32,
    /// Seconds to wait before half-opening (allowing a test request).
    pub reset_after_secs: u64,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_after_secs: 60,
        }
    }
}

/// State tracked for a single provider.
#[derive(Debug)]
struct ProviderState {
    failures: AtomicU32,
    /// Timestamp of the first failure in the current window (ms since epoch).
    first_failure_at: AtomicU64,
    /// Timestamp of when the breaker opened (ms since epoch). 0 = closed.
    opened_at: AtomicU64,
}

impl ProviderState {
    fn new() -> Self {
        Self {
            failures: AtomicU32::new(0),
            first_failure_at: AtomicU64::new(0),
            opened_at: AtomicU64::new(0),
        }
    }
}

/// Health registry keyed by provider name ("anthropic", "openai", …).
///
/// Each provider has its own circuit breaker so a failing provider can
/// be skipped independently while healthy ones keep serving.
pub struct ProviderHealthRegistry {
    states: RwLock<HashMap<String, ProviderState>>,
    config: BreakerConfig,
}

impl ProviderHealthRegistry {
    /// Create a registry with the given breaker config.
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Whether the provider should be attempted.
    ///
    /// Returns `true` when:
    /// - The breaker is closed (healthy).
    /// - The breaker is half-open and the reset timer has elapsed
    ///   (one test request allowed).
    pub fn is_healthy(&self, provider: &str) -> bool {
        let states = self.states.read();
        let state = match states.get(provider) {
            Some(s) => s,
            None => return true, // never seen = healthy
        };
        let opened = state.opened_at.load(Ordering::SeqCst);
        if opened == 0 {
            return true; // closed
        }
        // Half-open: allow one test request after the timeout.
        let elapsed = now_ms().saturating_sub(opened);
        elapsed >= self.config.reset_after_secs * 1000
    }

    /// Record a successful request — resets the failure counter and
    /// closes the breaker if it was half-open.
    pub fn record_success(&self, provider: &str) {
        let mut states = self.states.write();
        let state = states
            .entry(provider.to_string())
            .or_insert_with(ProviderState::new);
        state.failures.store(0, Ordering::SeqCst);
        state.first_failure_at.store(0, Ordering::SeqCst);
        state.opened_at.store(0, Ordering::SeqCst);
    }

    /// Record a failed request. If the consecutive failure count exceeds
    /// the threshold, the breaker opens.
    pub fn record_failure(&self, provider: &str) {
        let mut states = self.states.write();
        let state = states
            .entry(provider.to_string())
            .or_insert_with(ProviderState::new);
        let n = state.failures.fetch_add(1, Ordering::SeqCst) + 1;
        // Set first-failure timestamp on the first failure in a window.
        let _ = state.first_failure_at.compare_exchange(
            0,
            now_ms(),
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        if n >= self.config.failure_threshold {
            // Open: record when it opened, reset first_failure.
            state.opened_at.store(now_ms(), Ordering::SeqCst);
            state.first_failure_at.store(0, Ordering::SeqCst);
        }
    }
}

fn now_ms() -> u64 {
    // Fast: SystemTime epoch millis is ~1μs on macOS/arm64.
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_by_default() {
        let reg = ProviderHealthRegistry::new(BreakerConfig::default());
        assert!(reg.is_healthy("unknown-provider"));
    }

    #[test]
    fn opens_after_threshold_failures() {
        let reg = ProviderHealthRegistry::new(BreakerConfig {
            failure_threshold: 3,
            reset_after_secs: 60,
        });
        reg.record_failure("p");
        reg.record_failure("p");
        // Below threshold: still healthy.
        assert!(reg.is_healthy("p"));
        reg.record_failure("p");
        // Threshold hit: now unhealthy.
        assert!(!reg.is_healthy("p"));
    }

    #[test]
    fn success_resets_failures() {
        let reg = ProviderHealthRegistry::new(BreakerConfig {
            failure_threshold: 3,
            reset_after_secs: 3600, // won't reset in test
        });
        reg.record_failure("p");
        reg.record_failure("p");
        reg.record_success("p");
        // Reset to zero, so one failure isn't enough.
        reg.record_failure("p");
        assert!(reg.is_healthy("p"));
    }
}
