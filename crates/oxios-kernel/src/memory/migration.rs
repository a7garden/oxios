//! JSON → SQLite migration for existing memory data.
//!
//! One-time migration from the file-based StateStore to the SQLite
//! `memory.db`. Runs automatically on first launch after upgrade.
//! Original JSON files are preserved (not deleted).

use std::path::Path;

use anyhow::Result;

use super::database::MemoryDatabase;
use crate::memory::{MemoryEntry, MemoryType};

/// Migration report.
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    /// Number of entries successfully migrated.
    pub migrated: usize,
    /// Number of entries that failed to migrate.
    pub failed: usize,
    /// Memory types processed.
    pub types_processed: Vec<String>,
}

/// Execute the JSON → SQLite migration.
///
/// Scans all memory categories in the workspace directory, reads JSON files,
/// and inserts them into the SQLite database. Skips if already migrated.
pub fn migrate_json_to_sqlite(
    workspace_dir: &Path,
    db: &MemoryDatabase,
) -> Result<MigrationReport> {
    // Skip if already done
    if db.is_migration_complete() {
        tracing::debug!("Migration already complete, skipping");
        return Ok(MigrationReport::default());
    }

    tracing::info!("Starting JSON → SQLite memory migration...");
    let mut report = MigrationReport::default();

    for mt in MemoryType::all() {
        let category = mt.category();
        let category_dir = workspace_dir.join(category);

        if !category_dir.exists() {
            continue;
        }

        report.types_processed.push(mt.label().to_string());

        let entries = std::fs::read_dir(&category_dir)?;
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let json_str = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to read JSON file");
                    report.failed += 1;
                    continue;
                }
            };

            let mem: MemoryEntry = match serde_json::from_str(&json_str) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to parse JSON");
                    report.failed += 1;
                    continue;
                }
            };

            match insert_memory_entry(db, &mem) {
                Ok(_) => {
                    report.migrated += 1;
                }
                Err(e) => {
                    // Duplicate ID is OK — skip
                    if e.to_string().contains("UNIQUE constraint") {
                        tracing::debug!(id = %mem.id, "Memory already exists in SQLite, skipping");
                    } else {
                        tracing::warn!(id = %mem.id, error = %e, "Failed to insert memory");
                        report.failed += 1;
                    }
                }
            }
        }
    }

    // Mark migration complete
    db.set_dream_state("migration_v1_complete", "true")?;

    tracing::info!(
        migrated = report.migrated,
        failed = report.failed,
        "JSON → SQLite migration complete"
    );

    Ok(report)
}

/// Insert a MemoryEntry into the SQLite memories table.
fn insert_memory_entry(db: &MemoryDatabase, entry: &MemoryEntry) -> Result<()> {
    let conn = db.conn();

    let tags_json = serde_json::to_string(&entry.tags)?;
    let created_at = entry.created_at.to_rfc3339();
    let updated_at = entry.modified_at.to_rfc3339();
    let accessed_at = entry.accessed_at.to_rfc3339();

    let tier_label = match entry.tier {
        crate::memory::MemoryTier::Hot => "hot",
        crate::memory::MemoryTier::Warm => "warm",
        crate::memory::MemoryTier::Cold => "cold",
    };

    let protection_label = match entry.protection {
        crate::memory::ProtectionLevel::None => "none",
        crate::memory::ProtectionLevel::Low => "low",
        crate::memory::ProtectionLevel::Medium => "medium",
        crate::memory::ProtectionLevel::High => "high",
        crate::memory::ProtectionLevel::Permanent => "permanent",
    };

    conn.execute(
        "INSERT OR IGNORE INTO memories
         (id, memory_type, content, importance, tier, protection, source,
          session_id, tags, access_count, pinned, auto_classified,
          session_appearances, decay_score, compaction_level, content_hash,
          created_at, updated_at, accessed_at, decay_rate)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        rusqlite::params![
            entry.id,
            entry.memory_type.label(),
            entry.content,
            entry.importance,
            tier_label,
            protection_label,
            entry.source,
            entry.session_id,
            tags_json,
            entry.access_count as i64,
            entry.pinned as i64,
            entry.auto_classified as i64,
            entry.session_appearances as i64,
            entry.decay_score,
            entry.compaction_level as i64,
            entry.content_hash as i64,
            created_at,
            updated_at,
            accessed_at,
            entry.memory_type.base_decay_rate(),
        ],
    )?;

    Ok(())
}

/// Create a test MemoryEntry.
#[cfg(test)]
fn make_test_entry(id: &str, ty: crate::memory::MemoryType) -> crate::memory::MemoryEntry {
    use crate::memory::{MemoryEntry, MemoryTier, ProtectionLevel};
    use chrono::Utc;

    MemoryEntry {
        id: id.to_string(),
        memory_type: ty,
        tier: MemoryTier::Warm,
        content: format!("Test content for {}", id),
        content_hash: 0,
        source: "test".to_string(),
        session_id: None,
        tags: vec![],
        importance: 0.5,
        pinned: false,
        protection: ProtectionLevel::None,
        auto_classified: false,
        session_appearances: 0,
        user_corrected: false,
        seen_in_sessions: vec![],
        created_at: Utc::now(),
        accessed_at: Utc::now(),
        modified_at: Utc::now(),
        access_count: 0,
        decay_score: 1.0,
        compaction_level: 0,
        compacted_from: vec![],
        related_ids: vec![],
        contradicts: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_skip_if_complete() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        db.set_dream_state("migration_v1_complete", "true").unwrap();

        let dir = tempfile::tempdir().unwrap();
        let report = migrate_json_to_sqlite(dir.path(), &db).unwrap();
        assert_eq!(report.migrated, 0);
    }

    #[test]
    fn test_migration_empty_dir() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let dir = tempfile::tempdir().unwrap();

        let report = migrate_json_to_sqlite(dir.path(), &db).unwrap();
        assert_eq!(report.migrated, 0);
        assert!(db.is_migration_complete());
    }

    #[test]
    fn test_migration_with_data() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let dir = tempfile::tempdir().unwrap();

        // Create a mock memory JSON file
        let facts_dir = dir.path().join("memory/facts");
        std::fs::create_dir_all(&facts_dir).unwrap();

        let entry = make_test_entry("migrate-test-1", crate::memory::MemoryType::Fact);
        let json = serde_json::to_string(&entry).unwrap();
        std::fs::write(facts_dir.join("migrate-test-1.json"), json).unwrap();

        let report = migrate_json_to_sqlite(dir.path(), &db).unwrap();
        assert_eq!(report.migrated, 1);
        assert!(db.is_migration_complete());

        // Verify the data was inserted
        let loaded = super::super::search::load_memory_by_id(&db, "migrate-test-1").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().content, "Test content for migrate-test-1");
    }
}
