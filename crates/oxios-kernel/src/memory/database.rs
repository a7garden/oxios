//! SQLite-backed memory database (RFC-012).
//!
//! Single file: `~/.oxios/workspace/memory.db`
//!
//! Contains:
//! - `memories` — memory entries (replaces JSON StateStore for memory)
//! - `memories_fts` — FTS5 full-text search (BM25)
//! - `memory_vectors` — sqlite-vec vector KNN index
//! - `embedding_cache` — content-hash → embedding cache
//! - `dream_state` — Dream process persistent state
//! - `patterns` — learning patterns (SONA)

use std::path::Path;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;

/// Schema DDL for the memory database.
const SCHEMA: &str = r#"
-- ─────────────────────────────────────────────
-- 1. Memory entries
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY,
    memory_type     TEXT NOT NULL,
    content         TEXT NOT NULL,
    summary         TEXT,
    importance      REAL NOT NULL DEFAULT 0.5,
    tier            TEXT NOT NULL DEFAULT 'warm',
    protection      TEXT NOT NULL DEFAULT 'none',
    source          TEXT NOT NULL DEFAULT 'unknown',
    session_id      TEXT,
    space_id        TEXT,
    tags            TEXT,                       -- JSON array
    metadata        TEXT,                       -- JSON object
    access_count    INTEGER NOT NULL DEFAULT 0,
    pinned          INTEGER NOT NULL DEFAULT 0,
    auto_classified INTEGER NOT NULL DEFAULT 0,
    session_appearances INTEGER NOT NULL DEFAULT 0,
    decay_score     REAL NOT NULL DEFAULT 1.0,
    compaction_level INTEGER NOT NULL DEFAULT 0,
    content_hash    INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    accessed_at     TEXT,
    decay_rate      REAL NOT NULL DEFAULT 0.01
);

CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance);
CREATE INDEX IF NOT EXISTS idx_memories_tier ON memories(tier);

-- ─────────────────────────────────────────────
-- 2. FTS5 full-text search (BM25)
-- ─────────────────────────────────────────────
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id,
    content,
    memory_type,
    content='memories',
    content_rowid='rowid',
    tokenize="unicode61"
);

-- Triggers to keep FTS in sync with memories
CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, id, content, memory_type)
    VALUES (new.rowid, new.id, new.content, new.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, id, content, memory_type)
    VALUES ('delete', old.rowid, old.id, old.content, old.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, id, content, memory_type)
    VALUES ('delete', old.rowid, old.id, old.content, old.memory_type);
    INSERT INTO memories_fts(rowid, id, content, memory_type)
    VALUES (new.rowid, new.id, new.content, new.memory_type);
END;

-- ─────────────────────────────────────────────
-- 3. Embedding cache
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS embedding_cache (
    content_hash TEXT PRIMARY KEY,
    embedding    BLOB NOT NULL,
    created_at   TEXT NOT NULL
);

-- ─────────────────────────────────────────────
-- 4. Dream state
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS dream_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ─────────────────────────────────────────────
-- 5. Learning patterns
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS patterns (
    id           TEXT PRIMARY KEY,
    strategy     TEXT NOT NULL,
    domain       TEXT,
    quality      REAL NOT NULL DEFAULT 0.5,
    use_count    INTEGER NOT NULL DEFAULT 0,
    success_rate REAL NOT NULL DEFAULT 0.0,
    is_long_term INTEGER NOT NULL DEFAULT 0,
    embedding    BLOB,
    data         TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
"#;

/// Vector table DDL (separate because sqlite-vec must be loaded first).
/// The dimension placeholder `{DIM}` must be replaced at runtime.
const VEC_SCHEMA_TEMPLATE: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS memory_vectors USING vec0(
    embedding float[{DIM}]
);
"#;

/// SQLite-backed memory database.
///
/// All memory data lives in a single `.db` file with ACID guarantees.
/// Thread-safe via `Mutex<Connection>` (SQLite supports serialised access).
pub struct MemoryDatabase {
    conn: Mutex<Connection>,
    /// Embedding vector dimension (default 768 for EmbeddingGemma).
    embedding_dim: usize,
}

impl std::fmt::Debug for MemoryDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryDatabase")
            .field("embedding_dim", &self.embedding_dim)
            .finish()
    }
}

impl MemoryDatabase {
    /// Open (or create) the memory database at the given path.
    ///
    /// Loads the sqlite-vec extension, sets WAL mode, and initialises the schema.
    pub fn open(db_path: &Path, embedding_dim: usize) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Creating memory DB directory: {}", parent.display()))?;
        }

        // Register sqlite-vec globally (must be before any Connection::open)
        Self::register_vec_extension();

        let conn = Connection::open(db_path)
            .with_context(|| format!("Opening memory DB: {}", db_path.display()))?;

        // Enable WAL mode for concurrent reads
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;

        // Initialize main schema
        conn.execute_batch(SCHEMA)
            .context("Initializing memory database schema")?;

        // Initialize vector table (requires sqlite-vec loaded)
        conn.execute_batch(&VEC_SCHEMA_TEMPLATE.replace("{DIM}", &embedding_dim.to_string()))
            .context("Initializing sqlite-vec virtual table")?;

        tracing::info!(
            path = %db_path.display(),
            dim = embedding_dim,
            "Memory database opened"
            );

        Ok(Self {
            conn: Mutex::new(conn),
            embedding_dim,
        })
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory(embedding_dim: usize) -> Result<Self> {
        // Register sqlite-vec globally
        Self::register_vec_extension();

        let conn = Connection::open_in_memory()?;

        conn.execute_batch(SCHEMA)?;
        conn.execute_batch(&VEC_SCHEMA_TEMPLATE.replace("{DIM}", &embedding_dim.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
            embedding_dim,
        })
    }

    /// Register the sqlite-vec extension globally.
    ///
    /// Uses `sqlite3_auto_extension` to ensure vec0 is available
    /// for all connections opened after this call.
    fn register_vec_extension() {
        // Use a static flag to ensure we only register once.
        // sqlite3_auto_extension is process-global.
        static REGISTERED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !REGISTERED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            unsafe {
                // SAFETY: sqlite3_vec_init matches the sqlite3_auto_extension prototype.
                #[allow(clippy::missing_transmute_annotations)]
                rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                    sqlite_vec::sqlite3_vec_init as *const (),
                )));
            }
        }
    }

    /// Get a locked connection reference.
    ///
    /// Returns a `MutexGuard<Connection>` for executing queries.
    /// `parking_lot::Mutex` is `Send` and safe to use in async contexts.
    /// IMPORTANT: Always drop the guard before any `.await` point.
    pub fn conn(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.conn.lock()
    }

    /// Returns the configured embedding dimension.
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Backup the database by copying the file.
    ///
    /// For best results, call after a checkpoint to ensure WAL is flushed.
    /// Simply copies the `.db` file (does not use VACUUM INTO to avoid
    /// compatibility issues with sqlite-vec virtual tables).
    pub fn backup(&self, backup_path: &Path) -> Result<()> {
        // First, checkpoint WAL into the main database
        {
            let conn = self.conn();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        }

        // Now copy the file
        let db_path = {
            let conn = self.conn();
            conn.path()
                .map(std::path::PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("Cannot backup in-memory database"))?
        };

        std::fs::copy(&db_path, backup_path)
            .with_context(|| format!("Copying {} to {}", db_path.display(), backup_path.display()))?;

        tracing::info!(path = %backup_path.display(), "Memory database backed up");
        Ok(())
    }

    /// Get a dream state value.
    pub fn get_dream_state(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT value FROM dream_state WHERE key = ?1")?;
        let mut rows = stmt.query(rusqlite::params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Set a dream state value.
    pub fn set_dream_state(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO dream_state (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Check whether the JSON→SQLite migration has been completed.
    pub fn is_migration_complete(&self) -> bool {
        self.get_dream_state("migration_v1_complete")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false)
    }
}

/// Convert a `&[f32]` to a byte blob for sqlite-vec storage.
pub fn f32_slice_to_bytes(vec: &[f32]) -> Vec<u8> {
    // Safety: f32 is 4 bytes, and we're reading the raw representation.
    // zerocopy would be cleaner but this avoids an extra dependency.
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &v in vec {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// Convert a byte blob back to `Vec<f32>`.
pub fn bytes_to_f32_slice(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().expect("chunk must be 4 bytes");
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_schema_init() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        // Verify all tables exist
        let conn = db.conn();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' OR type='view' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
            .collect();

        assert!(tables.contains(&"memories".to_string()), "memories table missing");
        assert!(tables.contains(&"embedding_cache".to_string()), "embedding_cache table missing");
        assert!(tables.contains(&"dream_state".to_string()), "dream_state table missing");
        assert!(tables.contains(&"patterns".to_string()), "patterns table missing");
    }

    #[test]
    fn test_db_fts5_tables() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let conn = db.conn();

        // Verify FTS5 virtual table
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' OR type='view' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
            .collect();

        assert!(tables.contains(&"memories_fts".to_string()), "memories_fts missing");
    }

    #[test]
    fn test_dream_state() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        assert_eq!(db.get_dream_state("test_key").unwrap(), None);

        db.set_dream_state("test_key", "hello").unwrap();
        assert_eq!(db.get_dream_state("test_key").unwrap(), Some("hello".to_string()));

        db.set_dream_state("test_key", "updated").unwrap();
        assert_eq!(db.get_dream_state("test_key").unwrap(), Some("updated".to_string()));
    }

    #[test]
    fn test_migration_flag() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        assert!(!db.is_migration_complete());

        db.set_dream_state("migration_v1_complete", "true").unwrap();
        assert!(db.is_migration_complete());
    }

    #[test]
    fn test_f32_bytes_roundtrip() {
        let original: Vec<f32> = vec![0.1, 0.2, 0.3, -1.5, 42.0, 0.0];
        let bytes = f32_slice_to_bytes(&original);
        let restored = bytes_to_f32_slice(&bytes);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6, "Mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_insert_and_query_memory() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let conn = db.conn();

        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "test-id-1",
                "fact",
                "Rust is a systems programming language",
                0.6,
                "warm",
                "test",
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ],
        ).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let content: String = conn
            .query_row(
                "SELECT content FROM memories WHERE id = ?1",
                rusqlite::params!["test-id-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "Rust is a systems programming language");
    }

    #[test]
    fn test_fts5_korean_search() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let conn = db.conn();

        // Insert test data
        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('kr-1', 'fact', '한국어 테스트 메모리입니다', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('kr-2', 'fact', '영어 테스트 데이터입니다', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();

        // FTS5 search for Korean
        let results: Vec<String> = conn
            .prepare("SELECT id FROM memories_fts WHERE memories_fts MATCH ?1")
            .unwrap()
            .query_map(rusqlite::params!["한국어"], |row| row.get(0))
            .unwrap()
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
            .collect();

        assert!(
            results.contains(&"kr-1".to_string()),
            "Korean FTS should find kr-1, got: {:?}",
            results
        );
    }

    #[test]
    fn test_fts5_bm25_scoring() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let conn = db.conn();

        // Insert multiple entries with varying relevance
        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('bm-1', 'fact', 'Rust programming language safety', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('bm-2', 'fact', 'Python programming data science', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('bm-3', 'fact', 'Rust Rust Rust systems programming', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();

        // BM25 search for "Rust"
        let results: Vec<(String, f64)> = conn
            .prepare(
                "SELECT m.id, -bm25(memories_fts) as score
                 FROM memories_fts f
                 JOIN memories m ON m.id = f.id
                 WHERE memories_fts MATCH 'Rust'
                 ORDER BY score DESC"
            )
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
            .unwrap()
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
            .collect();

        // bm-3 has "Rust" 3 times → should rank highest
        assert!(!results.is_empty(), "BM25 should return results");
        assert_eq!(results[0].0, "bm-3", "Most relevant should be bm-3");
    }

    #[test]
    fn test_backup_skipped_in_memory() {
        // Backup is not supported for in-memory databases.
        // File-based backup is tested in integration tests.
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let backup_path = dir.path().join("backup.db");
        assert!(db.backup(&backup_path).is_err());
    }
}
