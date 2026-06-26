//! TokenMaxing facade — exposes the [`QuotaTracker`] (status/report) and the
//! [`TokenMaxer`] (start/stop/status/sessions) to the HTTP API and Web UI
//! (RFC-031 §9).
//!
//! The single live tracker and maxer are constructed once at boot
//! (`src/kernel.rs`) and shared here, so the API, UI, and orchestrator all
//! reach the same instances.

use std::sync::Arc;

use crate::token_maxing::{
    Availability, CooldownRecord, MaxerStatus, MaxingStart, ProviderSnapshot, QuotaTracker,
    QuotaTrackerSnapshot, RecalibrationRecord, TokenMaxer, TokenMaxingConfig, TokenMaxingSession,
};

/// KernelHandle facade for token-maxing (RFC-031).
///
/// Read-side surface (status panel, report, eligibility) is re-exported here
/// for the API/UI; the underlying tracker/maxer are reachable via
/// [`Self::tracker`] / [`Self::maxer`] for anything not covered.
pub struct TokenMaxingApi {
    tracker: Arc<QuotaTracker>,
    maxer: Arc<TokenMaxer>,
}

impl TokenMaxingApi {
    /// Wrap the shared tracker + maxer.
    pub fn new(tracker: Arc<QuotaTracker>, maxer: Arc<TokenMaxer>) -> Self {
        Self { tracker, maxer }
    }

    /// The underlying tracker (for the recalibration tick).
    pub fn tracker(&self) -> &Arc<QuotaTracker> {
        &self.tracker
    }

    /// The underlying maxer.
    pub fn maxer(&self) -> &Arc<TokenMaxer> {
        &self.maxer
    }

    // ── Maxer control (RFC-031 §9 start/stop/status/sessions) ─────────────

    /// Launch a session (window or manual). Returns the new session id.
    pub fn launch(&self, start: MaxingStart) -> anyhow::Result<String> {
        self.maxer.launch(start)
    }

    /// Request a graceful stop after the in-flight task.
    pub fn stop(&self) {
        self.maxer.stop();
    }

    /// Live status.
    pub fn status(&self) -> MaxerStatus {
        self.maxer.status()
    }

    /// Past sessions (most-recent last).
    pub fn sessions(&self) -> Vec<TokenMaxingSession> {
        self.maxer.sessions()
    }

    /// One past or in-flight session by id.
    pub fn session(&self, id: &str) -> Option<TokenMaxingSession> {
        self.maxer.session(id)
    }

    // ── Tracker status (RFC-031 §9 providers / report) ────────────────────

    /// Whether the mode is enabled AND has at least one eligible provider.
    pub fn enabled(&self) -> bool {
        let cfg = self.tracker.config();
        cfg.enabled && cfg.providers.iter().any(|p| cfg.is_eligible(&p.provider))
    }

    /// Current config snapshot.
    pub fn config(&self) -> TokenMaxingConfig {
        self.tracker.config()
    }

    /// Hot-reload the config, preserving usage counters for surviving providers.
    pub fn reload(&self, config: TokenMaxingConfig) {
        self.tracker.reload(config);
    }

    /// Per-provider availability verdict (RFC-031 §4).
    pub fn availability(&self, provider: &str) -> Availability {
        self.tracker.availability(provider)
    }

    /// All providers' verdicts (status panel + report).
    pub fn snapshots(&self) -> Vec<QuotaTrackerSnapshot> {
        self.tracker.snapshots()
    }

    /// Self-tracked snapshot for one provider (report fidelity).
    pub fn snapshot(&self, provider: &str) -> Option<ProviderSnapshot> {
        self.tracker.snapshot(provider)
    }

    /// Recalibration history (report).
    pub fn recalibration_history(&self) -> Vec<RecalibrationRecord> {
        self.tracker.recalibration_history()
    }

    /// Cooldown history (report).
    pub fn cooldown_history(&self) -> Vec<CooldownRecord> {
        self.tracker.cooldown_history()
    }
}
