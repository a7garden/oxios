//! A2A protocol circuit breaker for delegation reliability.
//!
//! Prevents cascading failures when A2A delegation repeatedly fails.
//!
//! # Example
//!
//! ```
//! use oxios_kernel::a2a_circuit_breaker::{A2ACircuitBreaker, CircuitState};
//!
//! let cb = A2ACircuitBreaker::new(3, 30);  // 3 failures, 30s reset
//! assert_eq!(cb.state(), CircuitState::Closed);
//! assert!(cb.is_allowed());
//! ```

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests allowed.
    Closed,
    /// Too many failures — requests blocked.
    Open,
    /// Testing recovery — limited requests allowed.
    HalfOpen,
}

impl CircuitState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
}

/// A2A delegation circuit breaker.
///
/// Tracks failures and opens the circuit when threshold is exceeded.
/// After timeout, allows limited test requests (half-open state).
#[derive(Debug)]
pub struct A2ACircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,
    threshold: u32,
    reset_timeout: Duration,
}

impl A2ACircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// # Arguments
    /// * `threshold` - Number of consecutive failures before opening
    /// * `reset_timeout_secs` - Seconds to wait before testing recovery
    pub fn new(threshold: u32, reset_timeout_secs: u64) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            threshold,
            reset_timeout: Duration::from_secs(reset_timeout_secs),
        }
    }

    /// Current circuit state.
    pub fn state(&self) -> CircuitState {
        CircuitState::from_u8(self.state.load(Ordering::Relaxed))
    }

    /// Whether a request is allowed through the circuit.
    pub fn is_allowed(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if reset timeout has passed
                let last_failure = self.last_failure_time.load(Ordering::Relaxed);
                let now = Instant::now().elapsed().as_secs();
                if now.saturating_sub(last_failure) > self.reset_timeout.as_secs() as u64 {
                    // Transition to half-open
                    self.state.store(CircuitState::HalfOpen as u8, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited requests (up to 2)
                self.success_count.load(Ordering::Relaxed) < 2
            }
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        match self.state() {
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= 2 {
                    // Recovery successful → closed
                    self.state.store(CircuitState::Closed as u8, Ordering::Relaxed);
                    self.failure_count.store(0, Ordering::Relaxed);
                    tracing::info!("A2A circuit breaker CLOSED (recovery successful)");
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_failure_time.store(
            Instant::now().elapsed().as_secs(),
            Ordering::Relaxed,
        );

        if failures >= self.threshold && self.state() != CircuitState::Open {
            self.state.store(CircuitState::Open as u8, Ordering::Relaxed);
            tracing::warn!(
                failures,
                threshold = self.threshold,
                "A2A circuit breaker OPEN"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = A2ACircuitBreaker::new(3, 10);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_opens_after_threshold() {
        let cb = A2ACircuitBreaker::new(3, 10);
        
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        
        cb.record_failure();  // Now at threshold
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let cb = A2ACircuitBreaker::new(3, 10);
        
        cb.record_failure();
        cb.record_failure();
        cb.record_success();  // Should reset
        
        assert_eq!(cb.failure_count.load(Ordering::Relaxed), 0);
    }
}
