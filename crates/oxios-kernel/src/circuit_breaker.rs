//! Circuit breaker for LLM provider calls.
//!
//! States: Closed (normal) → Open (failing) → Half-Open (testing)
//!
//! The circuit breaker prevents cascading failures when the LLM provider
//! is experiencing issues. When the circuit is open, requests are rejected
//! immediately instead of waiting for timeouts.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
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
    /// Atomic flag to ensure only one request passes through in half-open state.
    half_open_probe_sent: AtomicBool,
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
            half_open_probe_sent: AtomicBool::new(false),
            threshold,
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Check if a request is allowed through the circuit.
    ///
    /// Returns `true` if the circuit is closed or half-open (with probe gate).
    /// Returns `false` if the circuit is open or half-open probe already sent.
    pub fn is_allowed(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);

        match state {
            STATE_CLOSED => true,
            STATE_OPEN
                // Check if enough time has passed to attempt a reset.
                if self.should_attempt_reset() => {
                    // Atomically transition to half-open. Only one caller wins.
                    match self.state.compare_exchange(
                        STATE_OPEN,
                        STATE_HALF_OPEN,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            // The winner claims the single probe slot.
                            self.half_open_probe_sent.store(true, Ordering::Release);
                            true
                        }
                        Err(_) => {
                            // Lost the race — state already changed by another thread.
                            // Fall through to the half-open check below by re-reading.
                            self.half_open_probe_sent
                                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                                .is_ok()
                        }
                    }
                }
            STATE_HALF_OPEN => {
                // Only allow a single probe request through in half-open state.
                // compare_exchange ensures only one caller wins the race.
                self.half_open_probe_sent
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            }
            _ => false,
        }
    }

    fn should_attempt_reset(&self) -> bool {
        let last_ts_ms = self.last_failure_ts.load(Ordering::Acquire);
        if last_ts_ms == 0 {
            return true;
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let last = Duration::from_millis(last_ts_ms);
        let elapsed = now.saturating_sub(last);
        elapsed >= self.timeout
    }

    /// Record a successful call. Closes the circuit on success.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Release);
        self.state.store(STATE_CLOSED, Ordering::Release);
        self.half_open_probe_sent.store(false, Ordering::Release);
        crate::metrics::get_metrics()
            .llm_circuit_breaker_state
            .set(0.0);
    }

    /// Record a failed call. Opens the circuit if the failure threshold is exceeded.
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.last_failure_ts.store(now, Ordering::Release);

        if failures >= self.threshold {
            self.state.store(STATE_OPEN, Ordering::Release);
            self.half_open_probe_sent.store(false, Ordering::Release);
            tracing::warn!(
                failures,
                threshold = self.threshold,
                "Circuit breaker OPEN — too many failures"
            );
            crate::metrics::get_metrics()
                .llm_circuit_breaker_state
                .set(1.0);
        }
    }

    /// Get the current state as a string for debugging/metrics.
    pub fn state(&self) -> &'static str {
        match self.state.load(Ordering::Acquire) {
            STATE_CLOSED => "closed",
            STATE_OPEN => "open",
            STATE_HALF_OPEN => "half_open",
            _ => "unknown",
        }
    }

    /// Get the current failure count.
    #[allow(dead_code)]
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Acquire)
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

    #[test]
    fn test_half_open_allows_only_one_probe() {
        let cb = CircuitBreaker::new(1, 1); // opens on 1 failure, 1s timeout
        cb.record_failure(); // circuit is now open
        assert!(!cb.is_allowed()); // still open — timeout hasn't passed

        // Wait for timeout to pass
        std::thread::sleep(std::time::Duration::from_millis(1100));
        // First call transitions OPEN → HALF_OPEN and claims the probe slot.
        assert!(cb.is_allowed()); // first probe allowed
        assert!(!cb.is_allowed()); // second probe blocked
        assert!(!cb.is_allowed()); // third probe blocked
        assert_eq!(cb.state(), "half_open");
    }

    #[test]
    fn test_half_open_opens_on_failure() {
        let cb = CircuitBreaker::new(1, 1);
        cb.record_failure(); // open
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(cb.is_allowed()); // half-open probe

        cb.record_failure(); // probe failed → back to open
        assert_eq!(cb.state(), "open");
        // New timeout hasn't elapsed yet
        assert!(!cb.is_allowed()); // blocked
    }

    #[test]
    fn test_half_open_closes_on_success() {
        let cb = CircuitBreaker::new(1, 1);
        cb.record_failure(); // open
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(cb.is_allowed()); // half-open probe

        cb.record_success(); // probe succeeded → closed
        assert_eq!(cb.state(), "closed");
        assert!(cb.is_allowed()); // all requests allowed again
    }
}
