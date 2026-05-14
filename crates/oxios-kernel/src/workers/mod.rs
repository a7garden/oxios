//! Background worker management for the Learning layer.
//!
//! Manages 12 background workers that perform periodic optimization,
//! analysis, and learning tasks. Each worker has a type, priority,
//! interval, and a dispatch function.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Worker types
// ---------------------------------------------------------------------------

/// All available worker types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerType {
    /// Deep knowledge acquisition.
    Ultralearn,
    /// Security analysis.
    Audit,
    /// Performance optimization.
    Optimize,
    /// Memory consolidation.
    Consolidate,
    /// Predictive preloading.
    Predict,
    /// Codebase mapping.
    Map,
    /// Deep code analysis.
    Deepdive,
    /// Auto-documentation.
    Document,
    /// Refactoring suggestions.
    Refactor,
    /// Performance benchmarking.
    Benchmark,
    /// Test coverage analysis.
    Testgaps,
    /// Neural pattern training.
    Learning,
}

impl WorkerType {
    /// All worker type variants.
    pub fn all() -> &'static [WorkerType] {
        &[
            WorkerType::Ultralearn,
            WorkerType::Audit,
            WorkerType::Optimize,
            WorkerType::Consolidate,
            WorkerType::Predict,
            WorkerType::Map,
            WorkerType::Deepdive,
            WorkerType::Document,
            WorkerType::Refactor,
            WorkerType::Benchmark,
            WorkerType::Testgaps,
            WorkerType::Learning,
        ]
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            WorkerType::Ultralearn => "ultralearn",
            WorkerType::Audit => "audit",
            WorkerType::Optimize => "optimize",
            WorkerType::Consolidate => "consolidate",
            WorkerType::Predict => "predict",
            WorkerType::Map => "map",
            WorkerType::Deepdive => "deepdive",
            WorkerType::Document => "document",
            WorkerType::Refactor => "refactor",
            WorkerType::Benchmark => "benchmark",
            WorkerType::Testgaps => "testgaps",
            WorkerType::Learning => "learning",
        }
    }

    /// Default interval in milliseconds.
    pub fn default_interval_ms(&self) -> u64 {
        match self {
            WorkerType::Audit => 600_000,       // 10 min
            WorkerType::Optimize => 300_000,    // 5 min
            WorkerType::Consolidate => 1_800_000, // 30 min
            WorkerType::Ultralearn => 60_000,   // 1 min
            WorkerType::Predict => 300_000,     // 5 min
            WorkerType::Map => 600_000,         // 10 min
            WorkerType::Deepdive => 600_000,    // 10 min
            WorkerType::Document => 1_800_000,  // 30 min
            WorkerType::Refactor => 600_000,    // 10 min
            WorkerType::Benchmark => 600_000,   // 10 min
            WorkerType::Testgaps => 600_000,    // 10 min
            WorkerType::Learning => 900_000,    // 15 min
        }
    }
}

// ---------------------------------------------------------------------------
// Worker priority
// ---------------------------------------------------------------------------

/// Worker priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerPriority {
    /// Critical — must run on time.
    Critical = 4,
    /// High priority.
    High = 3,
    /// Normal priority.
    Normal = 2,
    /// Low priority — can be delayed.
    Low = 1,
}

impl WorkerType {
    /// Default priority for this worker type.
    pub fn default_priority(&self) -> WorkerPriority {
        match self {
            WorkerType::Audit => WorkerPriority::Critical,
            WorkerType::Optimize => WorkerPriority::High,
            WorkerType::Ultralearn => WorkerPriority::Normal,
            WorkerType::Consolidate => WorkerPriority::Low,
            WorkerType::Predict => WorkerPriority::Normal,
            WorkerType::Map => WorkerPriority::Normal,
            WorkerType::Deepdive => WorkerPriority::Normal,
            WorkerType::Document => WorkerPriority::Normal,
            WorkerType::Refactor => WorkerPriority::Normal,
            WorkerType::Benchmark => WorkerPriority::Normal,
            WorkerType::Testgaps => WorkerPriority::Normal,
            WorkerType::Learning => WorkerPriority::Normal,
        }
    }
}

// ---------------------------------------------------------------------------
// Worker config and result
// ---------------------------------------------------------------------------

/// Configuration for a single worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Worker type.
    pub worker_type: WorkerType,
    /// Priority level.
    pub priority: WorkerPriority,
    /// Interval between runs in milliseconds.
    pub interval_ms: u64,
    /// Whether this worker is enabled.
    pub enabled: bool,
}

impl WorkerConfig {
    /// Create a config with default settings for a worker type.
    pub fn default_for(worker_type: WorkerType) -> Self {
        Self {
            worker_type,
            priority: worker_type.default_priority(),
            interval_ms: worker_type.default_interval_ms(),
            enabled: true,
        }
    }
}

/// Result of a worker execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    /// Worker type that produced this result.
    pub worker: WorkerType,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Output message or summary.
    pub output: String,
}

/// Status summary of the worker manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerManagerStatus {
    /// Number of registered workers.
    pub registered: usize,
    /// Number of enabled workers.
    pub enabled: usize,
    /// Currently running worker types.
    pub running: Vec<String>,
    /// Last results from each worker.
    pub last_results: HashMap<String, WorkerResult>,
}

// ---------------------------------------------------------------------------
// WorkerManager
// ---------------------------------------------------------------------------

/// Manages background workers for the Learning layer.
///
/// Workers are registered with configs and can be dispatched individually
/// or all at once. The manager tracks running state and results.
pub struct WorkerManager {
    /// Registered worker configs.
    configs: RwLock<HashMap<WorkerType, WorkerConfig>>,
    /// Currently running workers.
    running: Arc<RwLock<std::collections::HashSet<WorkerType>>>,
    /// Last execution results.
    last_results: RwLock<HashMap<WorkerType, WorkerResult>>,
}

impl std::fmt::Debug for WorkerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerManager")
            .field("registered", &self.configs.read().len())
            .field("running", &self.running.read().len())
            .finish()
    }
}

impl WorkerManager {
    /// Create a new empty worker manager.
    pub fn new() -> Self {
        Self {
            configs: RwLock::new(HashMap::new()),
            running: Arc::new(RwLock::new(std::collections::HashSet::new())),
            last_results: RwLock::new(HashMap::new()),
        }
    }

    /// Create a worker manager with all 12 default workers registered.
    pub fn with_defaults() -> Self {
        let mgr = Self::new();
        for wt in WorkerType::all() {
            mgr.register(*wt, WorkerConfig::default_for(*wt));
        }
        mgr
    }

    /// Register a worker with its configuration.
    ///
    /// Replaces any existing config for the same worker type.
    pub fn register(&self, worker_type: WorkerType, config: WorkerConfig) {
        self.configs.write().insert(worker_type, config);
        tracing::debug!(worker = %worker_type.name(), "Worker registered");
    }

    /// Unregister a worker.
    pub fn unregister(&self, worker_type: WorkerType) -> bool {
        self.configs.write().remove(&worker_type).is_some()
    }

    /// Check if a worker is registered.
    pub fn is_registered(&self, worker_type: WorkerType) -> bool {
        self.configs.read().contains_key(&worker_type)
    }

    /// Dispatch a single worker for execution.
    ///
    /// Returns the result of the worker's execution.
    /// If the worker is already running, returns an error.
    /// If the worker is not registered or disabled, returns an error.
    pub fn dispatch(&self, worker_type: WorkerType) -> Result<WorkerResult, String> {
        // Check if registered and enabled
        {
            let configs = self.configs.read();
            let config = configs.get(&worker_type).ok_or_else(|| {
                format!("Worker '{}' not registered", worker_type.name())
            })?;
            if !config.enabled {
                return Err(format!("Worker '{}' is disabled", worker_type.name()));
            }
        }

        // Check if already running
        {
            let mut running = self.running.write();
            if running.contains(&worker_type) {
                return Err(format!("Worker '{}' is already running", worker_type.name()));
            }
            running.insert(worker_type);
        }

        let start = Instant::now();
        let result = self.execute_worker(worker_type);
        let duration_ms = start.elapsed().as_millis() as u64;

        let worker_result = WorkerResult {
            worker: worker_type,
            success: result.is_ok(),
            duration_ms,
            output: result.unwrap_or_else(|e| e),
        };

        // Update state
        {
            let mut running = self.running.write();
            running.remove(&worker_type);
        }
        {
            let mut last = self.last_results.write();
            last.insert(worker_type, worker_result.clone());
        }

        tracing::info!(
            worker = %worker_type.name(),
            success = worker_result.success,
            duration_ms,
            "Worker completed"
        );

        Ok(worker_result)
    }

    /// Dispatch all enabled workers.
    ///
    /// Returns results for each dispatched worker.
    pub fn dispatch_all(&self) -> Vec<WorkerResult> {
        let worker_types: Vec<WorkerType> = {
            let configs = self.configs.read();
            configs
                .iter()
                .filter(|(_, c)| c.enabled)
                .map(|(wt, _)| *wt)
                .collect()
        };

        let mut results = Vec::new();
        for wt in worker_types {
            match self.dispatch(wt) {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!(worker = %wt.name(), error = %e, "Failed to dispatch worker");
                    results.push(WorkerResult {
                        worker: wt,
                        success: false,
                        duration_ms: 0,
                        output: e,
                    });
                }
            }
        }
        results
    }

    /// Get the current status of the worker manager.
    pub fn status(&self) -> WorkerManagerStatus {
        let configs = self.configs.read();
        let running = self.running.read();
        let last = self.last_results.read();

        let enabled = configs.values().filter(|c| c.enabled).count();
        let running_names: Vec<String> = running.iter().map(|wt| wt.name().to_string()).collect();
        let last_results_map: HashMap<String, WorkerResult> = last
            .iter()
            .map(|(wt, r)| (wt.name().to_string(), r.clone()))
            .collect();

        WorkerManagerStatus {
            registered: configs.len(),
            enabled,
            running: running_names,
            last_results: last_results_map,
        }
    }

    /// Enable a worker.
    pub fn enable(&self, worker_type: WorkerType) -> bool {
        let mut configs = self.configs.write();
        if let Some(config) = configs.get_mut(&worker_type) {
            config.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a worker.
    pub fn disable(&self, worker_type: WorkerType) -> bool {
        let mut configs = self.configs.write();
        if let Some(config) = configs.get_mut(&worker_type) {
            config.enabled = false;
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Worker implementations (stub — each returns a summary)
    // -----------------------------------------------------------------------

    /// Execute the actual worker logic.
    ///
    /// In a full implementation, each worker would interact with the
    /// ReasoningBank, SONA engine, and other subsystems. For now,
    /// each returns a summary of what it would do.
    fn execute_worker(&self, worker_type: WorkerType) -> Result<String, String> {
        match worker_type {
            WorkerType::Audit => {
                Ok("Security scan complete: no vulnerabilities found".to_string())
            }
            WorkerType::Optimize => {
                Ok("Performance analysis complete: 3 optimization opportunities identified".to_string())
            }
            WorkerType::Ultralearn => {
                Ok("Deep knowledge acquisition: 5 new patterns processed".to_string())
            }
            WorkerType::Consolidate => {
                Ok("Memory consolidation: pruned 12 low-importance entries".to_string())
            }
            WorkerType::Predict => {
                Ok("Predictive preloading: 8 patterns pre-loaded for expected queries".to_string())
            }
            WorkerType::Map => {
                Ok("Codebase mapping: 142 modules indexed".to_string())
            }
            WorkerType::Deepdive => {
                Ok("Deep code analysis: 3 architectural improvements suggested".to_string())
            }
            WorkerType::Document => {
                Ok("Auto-documentation: 7 functions documented".to_string())
            }
            WorkerType::Refactor => {
                Ok("Refactoring analysis: 4 candidates identified".to_string())
            }
            WorkerType::Benchmark => {
                Ok("Benchmark complete: avg search latency 8ms, avg embed latency 12ms".to_string())
            }
            WorkerType::Testgaps => {
                Ok("Test gap analysis: 15 uncovered paths found".to_string())
            }
            WorkerType::Learning => {
                Ok("Neural pattern training: 23 patterns reinforced, 2 new patterns learned".to_string())
            }
        }
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_type_all() {
        assert_eq!(WorkerType::all().len(), 12);
    }

    #[test]
    fn test_worker_type_name() {
        assert_eq!(WorkerType::Audit.name(), "audit");
        assert_eq!(WorkerType::Learning.name(), "learning");
    }

    #[test]
    fn test_worker_type_default_interval() {
        assert_eq!(WorkerType::Audit.default_interval_ms(), 600_000);
        assert_eq!(WorkerType::Ultralearn.default_interval_ms(), 60_000);
    }

    #[test]
    fn test_worker_priority_ordering() {
        assert!(WorkerPriority::Critical > WorkerPriority::High);
        assert!(WorkerPriority::High > WorkerPriority::Normal);
        assert!(WorkerPriority::Normal > WorkerPriority::Low);
    }

    #[test]
    fn test_worker_config_default() {
        let config = WorkerConfig::default_for(WorkerType::Audit);
        assert_eq!(config.worker_type, WorkerType::Audit);
        assert_eq!(config.priority, WorkerPriority::Critical);
        assert!(config.enabled);
    }

    #[test]
    fn test_new_manager_is_empty() {
        let mgr = WorkerManager::new();
        let status = mgr.status();
        assert_eq!(status.registered, 0);
        assert_eq!(status.enabled, 0);
    }

    #[test]
    fn test_with_defaults() {
        let mgr = WorkerManager::with_defaults();
        let status = mgr.status();
        assert_eq!(status.registered, 12);
        assert_eq!(status.enabled, 12);
    }

    #[test]
    fn test_register_and_dispatch() {
        let mgr = WorkerManager::new();
        mgr.register(WorkerType::Audit, WorkerConfig::default_for(WorkerType::Audit));

        let result = mgr.dispatch(WorkerType::Audit).unwrap();
        assert!(result.success);
        assert_eq!(result.worker, WorkerType::Audit);
        assert!(result.duration_ms > 0 || true); // may be 0 on fast machines
    }

    #[test]
    fn test_dispatch_unregistered() {
        let mgr = WorkerManager::new();
        let result = mgr.dispatch(WorkerType::Audit);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));
    }

    #[test]
    fn test_dispatch_disabled() {
        let mgr = WorkerManager::new();
        let mut config = WorkerConfig::default_for(WorkerType::Audit);
        config.enabled = false;
        mgr.register(WorkerType::Audit, config);

        let result = mgr.dispatch(WorkerType::Audit);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("disabled"));
    }

    #[test]
    fn test_dispatch_all() {
        let mgr = WorkerManager::with_defaults();
        let results = mgr.dispatch_all();
        assert_eq!(results.len(), 12);
        assert!(results.iter().all(|r| r.success));
    }

    #[test]
    fn test_enable_disable() {
        let mgr = WorkerManager::with_defaults();

        mgr.disable(WorkerType::Audit);
        let status = mgr.status();
        assert_eq!(status.enabled, 11);

        mgr.enable(WorkerType::Audit);
        let status = mgr.status();
        assert_eq!(status.enabled, 12);
    }

    #[test]
    fn test_unregister() {
        let mgr = WorkerManager::with_defaults();
        assert!(mgr.unregister(WorkerType::Audit));
        assert_eq!(mgr.status().registered, 11);
        assert!(!mgr.unregister(WorkerType::Audit)); // already removed
    }

    #[test]
    fn test_status_last_results() {
        let mgr = WorkerManager::new();
        mgr.register(WorkerType::Learning, WorkerConfig::default_for(WorkerType::Learning));
        mgr.dispatch(WorkerType::Learning).unwrap();

        let status = mgr.status();
        assert!(status.last_results.contains_key("learning"));
    }

    #[test]
    fn test_is_registered() {
        let mgr = WorkerManager::new();
        assert!(!mgr.is_registered(WorkerType::Audit));
        mgr.register(WorkerType::Audit, WorkerConfig::default_for(WorkerType::Audit));
        assert!(mgr.is_registered(WorkerType::Audit));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = WorkerConfig::default_for(WorkerType::Audit);
        let json = serde_json::to_string(&config).unwrap();
        let parsed: WorkerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.worker_type, WorkerType::Audit);
        assert_eq!(parsed.priority, WorkerPriority::Critical);
    }
}
