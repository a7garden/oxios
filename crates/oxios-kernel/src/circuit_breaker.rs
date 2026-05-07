//! Circuit breaker for LLM provider calls.
//!
//! States: Closed (normal) → Open (failing) → Half-Open (testing)
//!
//! The circuit breaker prevents cascading failures when the LLM provider
//! is experiencing issues. When the circuit is open, requests are rejected
//! immediately instead of waiting for timeouts.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

/// Circuit is allowing requests (normal operation).
const STATE_CLOSED: u32 = 0;
/// Circuit is rejecting requests (provider is failing).
const STATE_OPEN: u32 = 1;
/// Circuit is allowing one request to test if provider is healthy.
const STATE_HALF_OPEN: u32 = 2;

/// A simple 3-state circuit breaker for protecting against cascading failures.
///
/// # States
/// - **Closed**: Normal operation. All requests pass through.
/// - **Open**: Provider is failing. Requests are rejected immediately.
/// - **Half-Open**: Testing if provider recovered. One request is allowed through.
///
/// # Transitions
/// - Closed → Open: After `threshold` consecutive failures
/// - Open → Half-Open: After `timeout` seconds have passed
/// - Half-Open → Closed: On success
/// - Half-Open → Open: On failure
pub struct CircuitBreaker {
    state: AtomicU32,
    failure_count: AtomicU32,
    last_failure_ts: AtomicU64,
    threshold: u32,
    timeout: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// - `threshold`: Number of consecutive failures before opening the circuit.
    /// - `timeout_secs`: Seconds to wait before attempting to close the circuit.
    pub fn new(threshold: u32, timeout_secs: u64) -> Self {
        Self {
            state: AtomicU32::new(STATE_CLOSED),
            failure_count: AtomicU32::new(0),
            last_failure_ts: AtomicU64::new(0),
            threshold,
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Check if a request is allowed through the circuit.
    ///
    /// Returns `true` if the circuit is closed or half-open.
    /// Returns `false` if the circuit is open (request should be rejected).
    pub fn is_allowed(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);

        match state {
            STATE_CLOSED => true,
            STATE_OPEN => {
                // Check if enough time has passed to attempt a reset.
                if self.should_attempt_reset() {
                    self.state.store(STATE_HALF_OPEN, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }
            STATE_HALF_OPEN => true,
            _ => false,
        }
    }

    fn should_attempt_reset(&self) -> bool {
        let last_ts = self.last_failure_ts.load(Ordering::Relaxed);
        if last_ts == 0 {
            return true;
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let elapsed = now.saturating_sub(last_ts);
        elapsed >= self.timeout.as_secs()
    }

    /// Record a successful call. Closes the circuit on success.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        self.state.store(STATE_CLOSED, Ordering::Relaxed);
    }

    /// Record a failed call. Opens the circuit if the failure threshold is exceeded.
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_failure_ts.store(now, Ordering::Relaxed);

        if failures >= self.threshold {
            self.state.store(STATE_OPEN, Ordering::Relaxed);
            tracing::warn!(
                failures,
                threshold = self.threshold,
                "Circuit breaker OPEN — too many failures"
            );
        }
    }

    /// Get the current state as a string for debugging/metrics.
    pub fn state(&self) -> &'static str {
        match self.state.load(Ordering::Relaxed) {
            STATE_CLOSED => "closed",
            STATE_OPEN => "open",
            STATE_HALF_OPEN => "half_open",
            _ => "unknown",
        }
    }

    /// Get the current failure count.
    #[allow(dead_code)]
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        // 5 failures opens the circuit, 30 second timeout before attempting reset
        Self::new(5, 30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::default();
        assert!(cb.is_allowed());
        assert_eq!(cb.state(), "closed");
    }

    #[test]
    fn test_circuit_opens_after_threshold_failures() {
        let cb = CircuitBreaker::new(3, 60);
        for _ in 0..2 {
            cb.record_failure();
        }
        assert!(cb.is_allowed()); // still closed

        cb.record_failure(); // 3rd failure
        assert!(!cb.is_allowed()); // now open
        assert_eq!(cb.state(), "open");
    }

    #[test]
    fn test_circuit_closes_on_success() {
        let cb = CircuitBreaker::default();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure(); // circuit is open

        cb.record_success();
        assert!(cb.is_allowed());
        assert_eq!(cb.state(), "closed");
    }
}