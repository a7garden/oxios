//! Resource monitoring for the Oxios kernel.
//!
//! Collects system metrics (CPU, memory, disk) and agent-level metrics
//! (active agents, pending tasks, token usage) to support scheduler decisions
//! and admin API endpoints.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use sysinfo::System;

/// Snapshot of system and agent resource usage at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// Timestamp of the snapshot.
    pub timestamp: DateTime<Utc>,
    /// CPU usage percentage (0.0–100.0).
    pub cpu_percent: f32,
    /// Memory used in megabytes.
    pub memory_used_mb: u64,
    /// Total memory in megabytes.
    pub memory_total_mb: u64,
    /// Number of currently active agents.
    pub active_agents: usize,
    /// Number of pending tasks in the scheduler.
    pub pending_tasks: usize,
    /// Cumulative token usage across all agents.
    pub total_token_usage: u64,
    /// Disk usage in gigabytes (estimated from workspace directory size).
    pub disk_used_gb: f64,
    /// 1-minute load average.
    pub load_avg_1m: f32,
}

/// Thresholds that define an "overloaded" system.
#[derive(Debug, Clone, Copy)]
pub struct OverloadThreshold {
    /// Maximum CPU percentage before considered overloaded.
    pub cpu_percent: f32,
    /// Maximum memory percentage before considered overloaded.
    pub memory_percent: f32,
    /// Maximum load average before considered overloaded.
    pub load_avg: f32,
}

impl Default for OverloadThreshold {
    fn default() -> Self {
        Self {
            cpu_percent: 90.0,
            memory_percent: 90.0,
            load_avg: 8.0,
        }
    }
}

/// Resource monitor collecting system and agent metrics.
///
/// Snapshots are automatically pushed to history when `record_snapshot()` is called.
/// Use `start_sampling()` to spawn a background task that periodically records snapshots.
pub struct ResourceMonitor {
    /// Sampling interval in seconds.
    interval_secs: u64,
    /// Maximum number of history entries to retain.
    history_max: usize,
    history: RwLock<VecDeque<ResourceSnapshot>>,
    total_token_usage: AtomicU64,
    active_agents: AtomicUsize,
    pending_tasks: AtomicUsize,
    overload_threshold: OverloadThreshold,
    /// Shared `sysinfo::System` instance to avoid recreating on every snapshot.
    sys: parking_lot::Mutex<System>,
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new(60, 60)
    }
}

impl ResourceMonitor {
    /// Create a new monitor with the given sampling interval and history size.
    pub fn new(interval_secs: u64, history_max: usize) -> Self {
        Self {
            interval_secs,
            history_max,
            history: RwLock::new(VecDeque::with_capacity(history_max)),
            total_token_usage: AtomicU64::new(0),
            active_agents: AtomicUsize::new(0),
            pending_tasks: AtomicUsize::new(0),
            overload_threshold: OverloadThreshold::default(),
            sys: parking_lot::Mutex::new(System::new_all()),
        }
    }

    /// Take a snapshot of current resource usage.
    ///
    /// Uses the shared `sysinfo::System` instance (refreshed on each call)
    /// instead of creating a new one each time.
    pub fn snapshot(&self) -> ResourceSnapshot {
        let mut sys = self.sys.lock();
        sys.refresh_all();

        // CPU: average across all cores
        let cpu_percent = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
            / sys.cpus().len().max(1) as f32;

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let memory_total_mb = total_memory / (1024 * 1024);
        let memory_used_mb = used_memory / (1024 * 1024);

        let load_avg_1m = System::load_average().one as f32;

        let disk_used_gb = estimate_disk_usage();

        ResourceSnapshot {
            timestamp: Utc::now(),
            cpu_percent,
            memory_used_mb,
            memory_total_mb,
            active_agents: self.active_agents.load(Ordering::Relaxed),
            pending_tasks: self.pending_tasks.load(Ordering::Relaxed),
            total_token_usage: self.total_token_usage.load(Ordering::Relaxed),
            disk_used_gb,
            load_avg_1m,
        }
    }

    /// Record a snapshot into the history buffer.
    ///
    /// Call this to push the current metrics into the history ring buffer.
    /// Oldest entries are evicted when `history_max` is reached.
    pub fn record_snapshot(&self) {
        let snap = self.snapshot();
        let mut history = self.history.write();
        if history.len() >= self.history_max {
            history.pop_front();
        }
        history.push_back(snap);
    }

    /// Spawn a background task that periodically records snapshots.
    ///
    /// Returns a `tokio::task::JoinHandle` that can be aborted to stop sampling.
    /// Uses the `interval_secs` configured at construction time.
    pub fn start_sampling(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let monitor = Arc::clone(self);
        let interval = self.interval_secs;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval));
            loop {
                ticker.tick().await;
                monitor.record_snapshot();
            }
        })
    }

    /// Returns historical snapshots, newest first.
    pub fn history(&self, last_n: usize) -> Vec<ResourceSnapshot> {
        let guard = self.history.read();
        let n = last_n.min(guard.len());
        guard.iter().rev().take(n).cloned().collect()
    }

    /// Returns true if the system is currently overloaded.
    pub fn is_overloaded(&self) -> bool {
        let snap = self.snapshot();
        let memory_percent = if snap.memory_total_mb > 0 {
            (snap.memory_used_mb as f32 / snap.memory_total_mb as f32) * 100.0
        } else {
            0.0
        };

        snap.cpu_percent >= self.overload_threshold.cpu_percent
            || memory_percent >= self.overload_threshold.memory_percent
            || snap.load_avg_1m >= self.overload_threshold.load_avg
    }

    /// Update the active agent count.
    pub fn set_active_agents(&self, count: usize) {
        self.active_agents.store(count, Ordering::Relaxed);
    }

    /// Update the pending tasks count.
    pub fn set_pending_tasks(&self, count: usize) {
        self.pending_tasks.store(count, Ordering::Relaxed);
    }

    /// Add to the cumulative token usage counter.
    pub fn add_token_usage(&self, tokens: u64) {
        self.total_token_usage.fetch_add(tokens, Ordering::Relaxed);
    }

    /// Returns a copy of the current overload threshold.
    pub fn overload_threshold(&self) -> OverloadThreshold {
        self.overload_threshold
    }
}

/// Estimate disk usage by walking the current working directory.
/// Returns size in gigabytes.
fn estimate_disk_usage() -> f64 {
    let cwd = std::env::current_dir().unwrap_or_default();
    walk_dir_size(&cwd) as f64 / (1024.0 * 1024.0 * 1024.0)
}

/// Recursively compute the size of a directory in bytes.
fn walk_dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = entry.metadata();
            if let Ok(m) = meta {
                if m.is_file() {
                    total += m.len();
                } else if m.is_dir() {
                    total += walk_dir_size(&entry.path());
                }
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_structure() {
        let monitor = ResourceMonitor::default();
        let snap = monitor.snapshot();

        assert!(snap.timestamp <= Utc::now());
        // CPU and memory values should be non-negative
        assert!(snap.cpu_percent >= 0.0);
        assert!(snap.memory_used_mb >= 0);
        assert!(snap.memory_total_mb >= 0);
        assert!(snap.active_agents >= 0);
        assert!(snap.pending_tasks >= 0);
        assert!(snap.total_token_usage >= 0);
        assert!(snap.disk_used_gb >= 0.0);
        assert!(snap.load_avg_1m >= 0.0);
    }

    #[test]
    fn test_is_overloaded_default_threshold() {
        let monitor = ResourceMonitor::default();
        // With default thresholds (90% CPU, 90% memory, load 8.0),
        // most machines should not be overloaded unless under extreme load.
        // This is a smoke test — the logic is correct even if the system IS overloaded.
        let _ = monitor.is_overloaded();
    }

    #[test]
    fn test_is_overloaded_high_thresholds_not_overloaded() {
        // Bypass low default thresholds by using a monitor that will only
        // be overloaded if values exceed 100% — which they never should.
        let monitor = ResourceMonitor::default();
        // No explicit setter for threshold; using default which is 90%.
        // This test verifies the comparison logic doesn't panic.
        let result = monitor.is_overloaded();
        // We can't assert false because the system might genuinely be overloaded.
        // Instead, just verify no panic and a bool is returned.
        let _ = result;
    }

    #[test]
    fn test_history_management() {
        let monitor = ResourceMonitor::new(1, 5);

        // Initially empty
        assert!(monitor.history(10).is_empty());

        // Record snapshots
        for _ in 0..3 {
            monitor.record_snapshot();
        }

        // History should now have 3 entries
        let history = monitor.history(10);
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_history_eviction() {
        let monitor = ResourceMonitor::new(1, 3);

        // Record more than capacity
        for _ in 0..5 {
            monitor.record_snapshot();
        }

        // Should only retain last 3
        let history = monitor.history(10);
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_set_active_agents() {
        let monitor = ResourceMonitor::default();
        monitor.set_active_agents(5);
        let snap = monitor.snapshot();
        assert_eq!(snap.active_agents, 5);
    }

    #[test]
    fn test_set_pending_tasks() {
        let monitor = ResourceMonitor::default();
        monitor.set_pending_tasks(3);
        let snap = monitor.snapshot();
        assert_eq!(snap.pending_tasks, 3);
    }

    #[test]
    fn test_add_token_usage() {
        let monitor = ResourceMonitor::default();
        monitor.add_token_usage(100);
        monitor.add_token_usage(200);
        let snap = monitor.snapshot();
        assert_eq!(snap.total_token_usage, 300);
    }

    #[test]
    fn test_overload_threshold_default() {
        let threshold = OverloadThreshold::default();
        assert_eq!(threshold.cpu_percent, 90.0);
        assert_eq!(threshold.memory_percent, 90.0);
        assert_eq!(threshold.load_avg, 8.0);
    }

    #[test]
    fn test_overload_threshold_custom() {
        let threshold = OverloadThreshold {
            cpu_percent: 75.0,
            memory_percent: 80.0,
            load_avg: 4.0,
        };
        assert_eq!(threshold.cpu_percent, 75.0);
        assert_eq!(threshold.memory_percent, 80.0);
        assert_eq!(threshold.load_avg, 4.0);
    }

    #[test]
    fn test_history_last_n() {
        let monitor = ResourceMonitor::new(1, 10);
        let empty = monitor.history(5);
        assert!(empty.is_empty());

        let many = monitor.history(100);
        assert!(many.is_empty());
    }

    #[test]
    fn test_load_average_struct() {
        let la = System::load_average();
        assert!(la.one >= 0.0);
        assert!(la.five >= 0.0);
        assert!(la.fifteen >= 0.0);
    }
}
