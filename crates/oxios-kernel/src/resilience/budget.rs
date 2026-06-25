//! Attempt budget — bounds total directive executions across the recovery
//! ladder (RFC-029 §3.4, D7).
//!
//! A single budget caps how many times a directive may be (re-)executed,
//! shared between the resilience ladder and (in P5) the orchestrator's
//! quality-retry (`verify_or_retry`). When exhausted, the ladder
//! short-circuits to terminal failure instead of burning cost/latency.
//!
//! P2: the budget is local to a single `RecoveryCoordinator::execute`
//! call. P5 will lift it to a shared counter so quality-retry and
//! resilience-retry draw from the same pool.

use std::sync::atomic::{AtomicU32, Ordering};

/// Bounded attempt counter. Thread-safe via a single atomic.
///
/// Construct with [`AttemptBudget::new`] giving the max number of
/// attempts. [`AttemptBudget::try_consume`] atomically decrements and
/// returns `false` once the limit is hit. A budget of `0` means
/// unlimited (always returns `true`).
pub struct AttemptBudget {
    remaining: AtomicU32,
    /// `0` = unlimited. Stored so `try_consume` is infallible when 0.
    unlimited: bool,
}

impl AttemptBudget {
    /// Create a budget allowing `max` total attempts. `max == 0` means
    /// unlimited (no cap).
    pub fn new(max: u32) -> Self {
        Self {
            remaining: AtomicU32::new(max),
            unlimited: max == 0,
        }
    }

    /// Try to consume one attempt. Returns `false` if no attempts remain.
    pub fn try_consume(&self) -> bool {
        if self.unlimited {
            return true;
        }
        // fetch_update avoids underflow past 0 and is CAS-clean.
        self.remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                if v > 0 { Some(v - 1) } else { None }
            })
            .is_ok()
    }

    /// Remaining attempts, or `u32::MAX` when unlimited.
    pub fn remaining(&self) -> u32 {
        if self.unlimited {
            u32::MAX
        } else {
            self.remaining.load(Ordering::SeqCst)
        }
    }
}

impl std::fmt::Debug for AttemptBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttemptBudget")
            .field("remaining", &self.remaining())
            .field("unlimited", &self.unlimited)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumes_until_exhausted() {
        let b = AttemptBudget::new(3);
        assert!(b.try_consume());
        assert!(b.try_consume());
        assert!(b.try_consume());
        assert!(!b.try_consume()); // exhausted
        assert_eq!(b.remaining(), 0);
    }

    #[test]
    fn zero_means_unlimited() {
        let b = AttemptBudget::new(0);
        for _ in 0..1000 {
            assert!(b.try_consume());
        }
    }

    #[test]
    fn remaining_tracks_consumption() {
        let b = AttemptBudget::new(2);
        assert_eq!(b.remaining(), 2);
        b.try_consume();
        assert_eq!(b.remaining(), 1);
        b.try_consume();
        assert_eq!(b.remaining(), 0);
    }
}
