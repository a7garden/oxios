//! RVF Learning Store — binary persistence for patterns, trajectories, and EWC state.
//!
//! Uses the RVLS (Rvf Learning Store) binary format for efficient persistence
//! of learning data. The format is a newline-delimited JSON stream wrapped
//! with magic bytes for integrity checking.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Magic bytes
// ---------------------------------------------------------------------------

const MAGIC_START: &[u8; 5] = b"RVLS\n";
const MAGIC_END: &[u8; 4] = b"REND";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A pattern record persisted to the RVF store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRecord {
    /// Unique ID.
    pub id: String,
    /// Strategy description.
    pub strategy: String,
    /// Embedding (sparse TF-IDF weights or dense vector).
    pub embedding_weights: HashMap<String, f64>,
    /// Success rate (0.0–1.0).
    pub success_rate: f32,
    /// Usage count.
    pub use_count: u32,
    /// Domain category.
    pub domain: String,
    /// Quality score.
    pub quality: f32,
    /// Whether this is a long-term pattern.
    pub is_long_term: bool,
    /// Last used timestamp.
    pub last_used: DateTime<Utc>,
}

/// A step within a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Input to this step.
    pub input: String,
    /// Output from this step.
    pub output: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Confidence score (0.0–1.0).
    pub confidence: f32,
}

/// A trajectory record — a sequence of steps with an outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryRecord {
    /// Unique ID.
    pub id: String,
    /// Ordered steps in the trajectory.
    pub steps: Vec<TrajectoryStep>,
    /// Outcome: "success", "partial", "failure".
    pub outcome: String,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Domain or task type.
    pub domain: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Elastic Weight Consolidation (EWC) state for continual learning.
///
/// Tracks importance weights per task to prevent catastrophic forgetting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwcState {
    /// Number of tasks learned so far.
    pub tasks_learned: u32,
    /// Protection strength (lambda in EWC paper).
    pub protection_strength: f32,
    /// Task ID → importance weights (Fisher information diagonal).
    pub task_weights: HashMap<String, Vec<f32>>,
}

impl Default for EwcState {
    fn default() -> Self {
        Self {
            tasks_learned: 0,
            protection_strength: 1000.0, // standard default
            task_weights: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// RVLS record types (for serialization envelope)
// ---------------------------------------------------------------------------

/// Envelope for each line in the RVLS file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum RvlsRecord {
    /// A guidance pattern.
    #[serde(rename = "pattern")]
    Pattern { data: PatternRecord },
    /// A trajectory record.
    #[serde(rename = "trajectory")]
    Trajectory { data: TrajectoryRecord },
    /// An EWC state snapshot.
    #[serde(rename = "ewc")]
    Ewc { data: EwcState },
}

// ---------------------------------------------------------------------------
// RvfLearningStore
// ---------------------------------------------------------------------------

/// Binary learning store for patterns, trajectories, and EWC state.
///
/// Persists data in the RVLS format: magic header + newline-delimited
/// JSON records + magic footer.
pub struct RvfLearningStore {
    /// Path to the .rvls file.
    store_path: PathBuf,
    /// In-memory pattern records.
    patterns: Vec<PatternRecord>,
    /// In-memory trajectory records.
    trajectories: Vec<TrajectoryRecord>,
    /// In-memory EWC state.
    ewc: EwcState,
    /// Whether the store has been modified since last persist.
    dirty: bool,
}

impl std::fmt::Debug for RvfLearningStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RvfLearningStore")
            .field("store_path", &self.store_path)
            .field("patterns", &self.patterns.len())
            .field("trajectories", &self.trajectories.len())
            .field("ewc_tasks", &self.ewc.tasks_learned)
            .finish()
    }
}

impl RvfLearningStore {
    /// Create a new store backed by the given file path.
    pub fn new(store_path: impl AsRef<Path>) -> Self {
        Self {
            store_path: store_path.as_ref().to_path_buf(),
            patterns: Vec::new(),
            trajectories: Vec::new(),
            ewc: EwcState::default(),
            dirty: false,
        }
    }

    /// Load all records from the RVLS file.
    ///
    /// If the file does not exist, starts with an empty store.
    pub fn initialize(&mut self) -> Result<()> {
        if !self.store_path.exists() {
            tracing::info!(path = %self.store_path.display(), "No existing RVLS file, starting fresh");
            return Ok(());
        }

        let data = std::fs::read(&self.store_path)
            .with_context(|| format!("Failed to read RVLS file: {}", self.store_path.display()))?;

        // Validate magic header
        if data.len() < MAGIC_START.len() + MAGIC_END.len() {
            anyhow::bail!("RVLS file too short: {}", self.store_path.display());
        }

        if &data[..MAGIC_START.len()] != MAGIC_START {
            anyhow::bail!("Invalid RVLS magic header in: {}", self.store_path.display());
        }

        if &data[data.len() - MAGIC_END.len()..] != MAGIC_END {
            anyhow::bail!("Invalid RVLS magic footer in: {}", self.store_path.display());
        }

        // Parse records between header and footer
        let body = &data[MAGIC_START.len()..data.len() - MAGIC_END.len()];
        let body_str = std::str::from_utf8(body)
            .with_context(|| "RVLS body is not valid UTF-8")?;

        let mut patterns = 0usize;
        let mut trajectories = 0usize;

        for line in body_str.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<RvlsRecord>(line) {
                Ok(RvlsRecord::Pattern { data }) => {
                    self.patterns.push(data);
                    patterns += 1;
                }
                Ok(RvlsRecord::Trajectory { data }) => {
                    self.trajectories.push(data);
                    trajectories += 1;
                }
                Ok(RvlsRecord::Ewc { data }) => {
                    self.ewc = data;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Skipping malformed RVLS record");
                }
            }
        }

        tracing::info!(
            path = %self.store_path.display(),
            patterns,
            trajectories,
            ewc_tasks = self.ewc.tasks_learned,
            "RVLS store initialized"
        );
        Ok(())
    }

    /// Persist all in-memory records to the RVLS file.
    pub fn persist(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let mut buf = Vec::new();

        // Magic header
        buf.extend_from_slice(MAGIC_START);

        // Pattern records
        for pattern in &self.patterns {
            let record = RvlsRecord::Pattern { data: pattern.clone() };
            serde_json::to_string(&record)
                .map(|s| {
                    buf.extend_from_slice(s.as_bytes());
                    buf.push(b'\n');
                })
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to serialize pattern");
                });
        }

        // Trajectory records
        for trajectory in &self.trajectories {
            let record = RvlsRecord::Trajectory { data: trajectory.clone() };
            serde_json::to_string(&record)
                .map(|s| {
                    buf.extend_from_slice(s.as_bytes());
                    buf.push(b'\n');
                })
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to serialize trajectory");
                });
        }

        // EWC state
        {
            let record = RvlsRecord::Ewc { data: self.ewc.clone() };
            serde_json::to_string(&record)
                .map(|s| {
                    buf.extend_from_slice(s.as_bytes());
                    buf.push(b'\n');
                })
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to serialize EWC state");
                });
        }

        // Magic footer
        buf.extend_from_slice(MAGIC_END);

        std::fs::write(&self.store_path, &buf)
            .with_context(|| format!("Failed to write RVLS file: {}", self.store_path.display()))?;

        tracing::debug!(
            path = %self.store_path.display(),
            patterns = self.patterns.len(),
            trajectories = self.trajectories.len(),
            "RVLS store persisted"
        );
        Ok(())
    }

    // -- Pattern operations --

    /// Save a new pattern record.
    pub fn save_pattern(&mut self, pattern: PatternRecord) {
        // Replace if ID already exists, otherwise append
        if let Some(idx) = self.patterns.iter().position(|p| p.id == pattern.id) {
            self.patterns[idx] = pattern;
        } else {
            self.patterns.push(pattern);
        }
        self.dirty = true;
    }

    /// Get all stored pattern records.
    pub fn get_all_patterns(&self) -> &[PatternRecord] {
        &self.patterns
    }

    /// Get pattern records filtered by domain.
    pub fn get_patterns_by_domain(&self, domain: &str) -> Vec<&PatternRecord> {
        self.patterns
            .iter()
            .filter(|p| p.domain == domain)
            .collect()
    }

    /// Remove a pattern by ID.
    pub fn remove_pattern(&mut self, id: &str) -> bool {
        let before = self.patterns.len();
        self.patterns.retain(|p| p.id != id);
        let removed = self.patterns.len() < before;
        if removed {
            self.dirty = true;
        }
        removed
    }

    // -- Trajectory operations --

    /// Save a trajectory record.
    pub fn save_trajectory(&mut self, trajectory: TrajectoryRecord) {
        if let Some(idx) = self.trajectories.iter().position(|t| t.id == trajectory.id) {
            self.trajectories[idx] = trajectory;
        } else {
            self.trajectories.push(trajectory);
        }
        self.dirty = true;
    }

    /// Get all stored trajectory records.
    pub fn get_all_trajectories(&self) -> &[TrajectoryRecord] {
        &self.trajectories
    }

    /// Get trajectories filtered by outcome.
    pub fn get_trajectories_by_outcome(&self, outcome: &str) -> Vec<&TrajectoryRecord> {
        self.trajectories
            .iter()
            .filter(|t| t.outcome == outcome)
            .collect()
    }

    /// Get the most recent N trajectories.
    pub fn recent_trajectories(&self, limit: usize) -> Vec<&TrajectoryRecord> {
        let mut refs: Vec<_> = self.trajectories.iter().collect();
        refs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        refs.truncate(limit);
        refs
    }

    // -- EWC operations --

    /// Save/update EWC state.
    pub fn save_ewc(&mut self, ewc: EwcState) {
        self.ewc = ewc;
        self.dirty = true;
    }

    /// Get current EWC state.
    pub fn get_ewc(&self) -> &EwcState {
        &self.ewc
    }

    /// Record a new task in EWC state with its importance weights.
    pub fn record_ewc_task(&mut self, task_id: String, weights: Vec<f32>) {
        self.ewc.task_weights.insert(task_id, weights);
        self.ewc.tasks_learned += 1;
        self.dirty = true;
    }

    // -- Stats --

    /// Return summary statistics.
    pub fn stats(&self) -> RvfStoreStats {
        RvfStoreStats {
            pattern_count: self.patterns.len(),
            trajectory_count: self.trajectories.len(),
            ewc_tasks_learned: self.ewc.tasks_learned,
            is_dirty: self.dirty,
        }
    }
}

/// Summary statistics for the RVF store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvfStoreStats {
    /// Number of stored patterns.
    pub pattern_count: usize,
    /// Number of stored trajectories.
    pub trajectory_count: usize,
    /// Number of EWC tasks learned.
    pub ewc_tasks_learned: u32,
    /// Whether the store has unsaved changes.
    pub is_dirty: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pattern(id: &str, strategy: &str, domain: &str) -> PatternRecord {
        PatternRecord {
            id: id.to_string(),
            strategy: strategy.to_string(),
            embedding_weights: HashMap::new(),
            success_rate: 0.9,
            use_count: 5,
            domain: domain.to_string(),
            quality: 0.85,
            is_long_term: false,
            last_used: Utc::now(),
        }
    }

    fn make_trajectory(id: &str, outcome: &str) -> TrajectoryRecord {
        TrajectoryRecord {
            id: id.to_string(),
            steps: vec![TrajectoryStep {
                input: "test input".to_string(),
                output: "test output".to_string(),
                duration_ms: 100,
                confidence: 0.8,
            }],
            outcome: outcome.to_string(),
            duration_ms: 200,
            domain: "testing".to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_new_store_is_empty() {
        let store = RvfLearningStore::new("/tmp/test_new.rvls");
        let stats = store.stats();
        assert_eq!(stats.pattern_count, 0);
        assert_eq!(stats.trajectory_count, 0);
        assert_eq!(stats.ewc_tasks_learned, 0);
    }

    #[test]
    fn test_save_and_get_patterns() {
        let mut store = RvfLearningStore::new("/tmp/test_patterns.rvls");
        store.save_pattern(make_pattern("p1", "Use HTTPS", "security"));
        store.save_pattern(make_pattern("p2", "Write tests", "testing"));

        assert_eq!(store.get_all_patterns().len(), 2);
        assert_eq!(store.get_patterns_by_domain("security").len(), 1);
    }

    #[test]
    fn test_save_and_get_trajectories() {
        let mut store = RvfLearningStore::new("/tmp/test_traj.rvls");
        store.save_trajectory(make_trajectory("t1", "success"));
        store.save_trajectory(make_trajectory("t2", "failure"));

        assert_eq!(store.get_all_trajectories().len(), 2);
        assert_eq!(store.get_trajectories_by_outcome("success").len(), 1);
    }

    #[test]
    fn test_ewc_state() {
        let mut store = RvfLearningStore::new("/tmp/test_ewc.rvls");
        assert_eq!(store.get_ewc().tasks_learned, 0);

        store.record_ewc_task("task-1".to_string(), vec![0.1, 0.2, 0.3]);
        assert_eq!(store.get_ewc().tasks_learned, 1);
        assert!(store.get_ewc().task_weights.contains_key("task-1"));
    }

    #[test]
    fn test_persist_and_initialize() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rvls");

        // Write
        {
            let mut store = RvfLearningStore::new(&path);
            store.save_pattern(make_pattern("p1", "Use HTTPS", "security"));
            store.save_trajectory(make_trajectory("t1", "success"));
            store.record_ewc_task("task-1".to_string(), vec![0.5]);
            store.persist().unwrap();
        }

        // Verify magic bytes
        let data = std::fs::read(&path).unwrap();
        assert_eq!(&data[..5], MAGIC_START);
        assert_eq!(&data[data.len() - 4..], MAGIC_END);

        // Read back
        {
            let mut store = RvfLearningStore::new(&path);
            store.initialize().unwrap();
            assert_eq!(store.get_all_patterns().len(), 1);
            assert_eq!(store.get_all_trajectories().len(), 1);
            assert_eq!(store.get_ewc().tasks_learned, 1);
        }
    }

    #[test]
    fn test_initialize_missing_file() {
        let mut store = RvfLearningStore::new("/tmp/nonexistent_test.rvls");
        store.initialize().unwrap(); // should not fail
        assert_eq!(store.get_all_patterns().len(), 0);
    }

    #[test]
    fn test_remove_pattern() {
        let mut store = RvfLearningStore::new("/tmp/test_remove.rvls");
        store.save_pattern(make_pattern("p1", "Test 1", "testing"));
        store.save_pattern(make_pattern("p2", "Test 2", "testing"));

        assert!(store.remove_pattern("p1"));
        assert_eq!(store.get_all_patterns().len(), 1);
        assert!(!store.remove_pattern("p1")); // already removed
    }

    #[test]
    fn test_pattern_upsert() {
        let mut store = RvfLearningStore::new("/tmp/test_upsert.rvls");
        store.save_pattern(make_pattern("p1", "Original", "testing"));
        assert_eq!(store.get_all_patterns()[0].strategy, "Original");

        let mut updated = make_pattern("p1", "Updated", "testing");
        updated.quality = 0.99;
        store.save_pattern(updated);
        assert_eq!(store.get_all_patterns().len(), 1);
        assert_eq!(store.get_all_patterns()[0].strategy, "Updated");
    }

    #[test]
    fn test_recent_trajectories() {
        let mut store = RvfLearningStore::new("/tmp/test_recent.rvls");
        store.save_trajectory(make_trajectory("t1", "success"));
        store.save_trajectory(make_trajectory("t2", "failure"));
        store.save_trajectory(make_trajectory("t3", "success"));

        let recent = store.recent_trajectories(2);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_invalid_magic_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.rvls");
        std::fs::write(&path, b"INVALID\nsome data\nREND").unwrap();

        let mut store = RvfLearningStore::new(&path);
        assert!(store.initialize().is_err());
    }
}
