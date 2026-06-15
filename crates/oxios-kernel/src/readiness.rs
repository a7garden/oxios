//! Subsystem readiness tracking (RFC-024 SP4).
//!
//! A daemon can answer HTTP requests before every subsystem has finished
//! initializing (state store loading, engine provider warm-up, etc.). Naive
//! handling causes `500`/`Internal` errors for the first few hundred
//! milliseconds of every restart, plus hangs when the orchestrator is
//! permanently unavailable. This module gives callers a single atomic
//! gate: a route is "ready" only when both the state store and the engine
//! have reached `Ready` or `Degraded`.
//!
//! **Three-state model** (per subsystem):
//! - `Warming` — startup, not yet `Ready`. Counts as "not ready".
//! - `Ready` — fully operational. Counts as "ready".
//! - `Degraded` — operational with limitations (e.g. engine initialized but no API key;
//!   only a fallback model available). **Counts as "ready"** so a missing API key does
//!   not lock the user out of `/api/status` for diagnosis.
//! - `Failed` — startup aborted (engine init crashed). The state store is still useful
//!   for inspection so it is allowed to become `Ready` independently; the engine `Failed`
//!   state keeps the readiness gate closed and `/api/status` is the only API that
//!   bypasses it (RFC-024 §7.1.1).
//!
//! **Deadline.** Callers set a deadline (default 30 s) after which any
//! subsystem still in `Warming` is force-promoted to `Degraded` to prevent
//! the gate from staying closed forever.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const STATE_WARMING: u8 = 0;
const STATE_READY: u8 = 1;
const STATE_DEGRADED: u8 = 2;
const STATE_FAILED: u8 = 3;

/// Coarse readiness of a single subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubsystemState {
    /// Startup in progress.
    Warming,
    /// Fully operational.
    Ready,
    /// Operational with limitations (still counts as "ready" for the gate).
    Degraded,
    /// Startup aborted; the subsystem is not usable.
    Failed,
}

impl SubsystemState {
    fn to_u8(self) -> u8 {
        match self {
            Self::Warming => STATE_WARMING,
            Self::Ready => STATE_READY,
            Self::Degraded => STATE_DEGRADED,
            Self::Failed => STATE_FAILED,
        }
    }
    fn from_u8(v: u8) -> Self {
        match v {
            STATE_READY => Self::Ready,
            STATE_DEGRADED => Self::Degraded,
            STATE_FAILED => Self::Failed,
            _ => Self::Warming,
        }
    }
}

// Manual Serialize/Deserialize without external derive (used by `KernelHandle::readiness`
// in tests + status JSON).
use serde::{Deserialize, Serialize};

/// Readiness gate: tracks two subsystems (state store, engine) and exposes
/// a single `is_ready()` that returns `true` when the daemon can safely
/// serve protected API routes.
pub struct ReadinessGate {
    state_store: AtomicU8,
    engine: AtomicU8,
    /// Unix-epoch seconds at which still-Warming subsystems are force-promoted
    /// to Degraded. `0` means "no deadline" (caller is responsible).
    deadline_secs: AtomicU64,
}

impl ReadinessGate {
    /// Create a new gate in `Warming` state for both subsystems. `deadline_secs`
    /// is the wall-clock (Unix epoch) at which any still-Warming subsystem
    /// is force-promoted to Degraded. Pass `0` to disable the deadline.
    pub fn new(deadline_secs: u64) -> Self {
        Self {
            state_store: AtomicU8::new(STATE_WARMING),
            engine: AtomicU8::new(STATE_WARMING),
            deadline_secs: AtomicU64::new(deadline_secs),
        }
    }

    /// Update the wall-clock deadline for force-promoting Warming → Degraded.
    /// Pass `0` to disable enforcement.
    pub fn set_deadline_secs(&self, secs: u64) {
        self.deadline_secs.store(secs, Ordering::SeqCst);
    }

    /// Read the current deadline (Unix-epoch seconds, or `0` if disabled).
    pub fn deadline_secs(&self) -> u64 {
        self.deadline_secs.load(Ordering::SeqCst)
    }

    /// Update the state-store readiness.
    pub fn set_state_store(&self, s: SubsystemState) {
        self.state_store.store(s.to_u8(), Ordering::SeqCst);
    }

    /// Update the engine readiness.
    pub fn set_engine(&self, s: SubsystemState) {
        self.engine.store(s.to_u8(), Ordering::SeqCst);
    }

    /// Read the current state-store state.
    pub fn state_store_state(&self) -> SubsystemState {
        SubsystemState::from_u8(self.state_store.load(Ordering::SeqCst))
    }

    /// Read the current engine state.
    pub fn engine_state(&self) -> SubsystemState {
        SubsystemState::from_u8(self.engine.load(Ordering::SeqCst))
    }

    /// `true` when the gate is open: both subsystems are `Ready` or
    /// `Degraded`. A `Failed` (or still-`Warming`) subsystem keeps the gate
    /// closed. `Degraded` counts as ready so a missing API key (engine)
    /// or a slow-but-functional state store does not lock the user out
    /// after the deadline elapses (RFC-024 SP4).
    pub fn is_ready(&self) -> bool {
        let s = self.state_store_state();
        let e = self.engine_state();
        let s_ok = s == SubsystemState::Ready || s == SubsystemState::Degraded;
        let e_ok = e == SubsystemState::Ready || e == SubsystemState::Degraded;
        s_ok && e_ok
    }

    /// Force-promote any still-Warming subsystem to Degraded once the
    /// deadline elapses. Idempotent. Should be called by the kernel
    /// during init and by the readiness middleware to enforce a ceiling
    /// on how long a misconfigured engine can lock the gate.
    pub fn enforce_deadline(&self) {
        let deadline = self.deadline_secs.load(Ordering::SeqCst);
        if deadline == 0 {
            return;
        }
        if self.is_ready() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now < deadline {
            return;
        }
        if self.state_store_state() == SubsystemState::Warming {
            self.set_state_store(SubsystemState::Degraded);
        }
        if self.engine_state() == SubsystemState::Warming {
            self.set_engine(SubsystemState::Degraded);
        }
    }
}

impl std::fmt::Debug for ReadinessGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadinessGate")
            .field("state_store", &self.state_store_state())
            .field("engine", &self.engine_state())
            .field("is_ready", &self.is_ready())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_warming_and_not_ready() {
        let g = ReadinessGate::new(0);
        assert!(!g.is_ready());
        assert_eq!(g.state_store_state(), SubsystemState::Warming);
        assert_eq!(g.engine_state(), SubsystemState::Warming);
    }

    #[test]
    fn both_ready_means_ready() {
        let g = ReadinessGate::new(0);
        g.set_state_store(SubsystemState::Ready);
        g.set_engine(SubsystemState::Ready);
        assert!(g.is_ready());
    }

    #[test]
    fn engine_degraded_still_counts_as_ready() {
        let g = ReadinessGate::new(0);
        g.set_state_store(SubsystemState::Ready);
        g.set_engine(SubsystemState::Degraded);
        assert!(g.is_ready());
    }

    #[test]
    fn engine_failed_keeps_gate_closed() {
        let g = ReadinessGate::new(0);
        g.set_state_store(SubsystemState::Ready);
        g.set_engine(SubsystemState::Failed);
        assert!(!g.is_ready());
    }

    #[test]
    fn state_store_not_ready_keeps_gate_closed() {
        let g = ReadinessGate::new(0);
        g.set_engine(SubsystemState::Ready);
        assert!(!g.is_ready());
    }

    #[test]
    fn deadline_elapsed_promotes_warming_to_degraded() {
        // Deadline in the past.
        let g = ReadinessGate::new(1);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        g.enforce_deadline();
        assert_eq!(g.state_store_state(), SubsystemState::Degraded);
        assert_eq!(g.engine_state(), SubsystemState::Degraded);
        assert!(g.is_ready());
    }

    #[test]
    fn deadline_not_yet_elapsed_keeps_warming() {
        let deadline = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60;
        let g = ReadinessGate::new(deadline);
        g.enforce_deadline();
        assert_eq!(g.state_store_state(), SubsystemState::Warming);
        assert!(!g.is_ready());
    }

    #[test]
    fn deadline_zero_disables_enforcement() {
        let g = ReadinessGate::new(0);
        g.enforce_deadline();
        assert_eq!(g.state_store_state(), SubsystemState::Warming);
    }
}
