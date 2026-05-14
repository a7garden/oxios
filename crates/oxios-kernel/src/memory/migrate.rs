//! TF-IDF → HNSW migration utilities.
//!
//! Provides migration from the legacy TF-IDF based memory system
//! to the enhanced vector search system. This is a one-time migration
//! that re-embeds all existing memory entries.

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use crate::state_store::StateStore;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{MemoryEntry, MemoryManager, MemoryType};

// ---------------------------------------------------------------------------
// Migration types
// ---------------------------------------------------------------------------

/// Progress tracking for a migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    /// Total entries to migrate.
    pub total: usize,
    /// Entries successfully migrated.
    pub migrated: usize,
    /// Entries that failed migration.
    pub failed: usize,
    /// Error messages keyed by entry ID.
    pub errors: Vec<(String, String)>,
}

impl MigrationProgress {
    /// Create a new progress tracker with the given total.
    pub fn new(total: usize) -> Self {
        Self {
            total,
            migrated: 0,
            failed: 0,
            errors: Vec::new(),
        }
    }

    /// Whether migration is complete (migrated + failed == total).
    pub fn is_complete(&self) -> bool {
        self.migrated + self.failed >= self.total
    }

    /// Success rate (0.0–1.0).
    pub fn success_rate(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            self.migrated as f32 / self.total as f32
        }
    }
}

/// Final report of a migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    /// Detailed progress.
    pub progress: MigrationProgress,
    /// Total migration duration in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Migration implementation
// ---------------------------------------------------------------------------

/// Batch size for embedding during migration.
const BATCH_SIZE: usize = 32;

impl MemoryManager {
    /// Migrate all existing memory entries to the vector index.
    ///
    /// This is the primary migration path from TF-IDF to enhanced
    /// vector search. It:
    /// 1. Loads all entries from the StateStore
    /// 2. Re-embeds them using the current embedding provider
    /// 3. Inserts them into the in-memory vector index
    ///
    /// Entries are processed in batches to manage memory usage.
    pub async fn migrate_from_tfidf(&self) -> Result<MigrationReport> {
        let start = std::time::Instant::now();

        // Collect all entries
        let all_types = [
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ];

        let mut all_entries: Vec<MemoryEntry> = Vec::new();
        for mt in &all_types {
            if let Ok(entries) = self.list(*mt, 100_000).await {
                all_entries.extend(entries);
            }
        }

        let total = all_entries.len();
        let mut progress = MigrationProgress::new(total);

        tracing::info!(total, "Starting TF-IDF migration");

        // Process in batches
        for chunk in all_entries.chunks(BATCH_SIZE) {
            for entry in chunk {
                match self.re_index_entry(entry).await {
                    Ok(()) => {
                        progress.migrated += 1;
                    }
                    Err(e) => {
                        progress.failed += 1;
                        progress.errors.push((entry.id.clone(), e.to_string()));
                        tracing::warn!(id = %entry.id, error = %e, "Migration failed for entry");
                    }
                }
            }

            // Log progress every batch
            if progress.migrated % BATCH_SIZE == 0 {
                tracing::info!(
                    migrated = progress.migrated,
                    failed = progress.failed,
                    total = progress.total,
                    "Migration progress"
                );
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            migrated = progress.migrated,
            failed = progress.failed,
            total = progress.total,
            duration_ms,
            "Migration complete"
        );

        Ok(MigrationReport {
            progress,
            duration_ms,
        })
    }

    /// Re-index a single memory entry.
    ///
    /// Generates a new embedding and updates the vector index.
    async fn re_index_entry(&self, entry: &MemoryEntry) -> Result<()> {
        let vector = self.embedding.embed(&entry.content).await?;

        {
            let mut index = self.vector_index.write();
            index.insert(entry.id.clone(), vector);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_progress_new() {
        let p = MigrationProgress::new(100);
        assert_eq!(p.total, 100);
        assert_eq!(p.migrated, 0);
        assert_eq!(p.failed, 0);
        assert!(!p.is_complete());
    }

    #[test]
    fn test_migration_progress_complete() {
        let mut p = MigrationProgress::new(10);
        p.migrated = 8;
        p.failed = 2;
        assert!(p.is_complete());
        assert!((p.success_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_migration_progress_empty() {
        let p = MigrationProgress::new(0);
        assert!(p.is_complete());
        assert_eq!(p.success_rate(), 1.0);
    }

    #[tokio::test]
    async fn test_migrate_empty_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store);

        let report = mgr.migrate_from_tfidf().await.unwrap();
        assert_eq!(report.progress.total, 0);
        assert_eq!(report.progress.migrated, 0);
    }

    #[tokio::test]
    async fn test_migrate_with_entries() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store.clone());

        // Store some entries
        for i in 0..5 {
            let entry = MemoryEntry {
                id: format!("migrate-test-{}", i),
                memory_type: MemoryType::Fact,
                content: format!("Test content for migration entry {}", i),
                source: "test".to_string(),
                session_id: None,
                tags: vec![],
                importance: 0.5,
                created_at: chrono::Utc::now(),
                accessed_at: chrono::Utc::now(),
                access_count: 0,
            };
            mgr.remember(entry).await.unwrap();
        }

        // Clear the index to simulate pre-migration state
        {
            let mut index = mgr.vector_index.write();
            index.clear();
        }
        assert_eq!(mgr.vector_index_size(), 0);

        // Run migration
        let report = mgr.migrate_from_tfidf().await.unwrap();
        assert_eq!(report.progress.total, 5);
        assert_eq!(report.progress.migrated, 5);
        assert_eq!(report.progress.failed, 0);
        assert_eq!(mgr.vector_index_size(), 5);
    }
}
