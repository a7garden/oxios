//! SQLite-backed memory store (RFC-012).
//!
//! Provides `remember()`, `search()`, `recall()`, `get()`, `forget()`
//! operations using the SQLite `memory.db` as the single source of truth.
//!
//! When the `sqlite-memory` feature is enabled and `memory.sqlite.enabled`
//! is true in config, MemoryManager delegates to this store instead of
//! the file-based StateStore.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;

use super::database::MemoryDatabase;
use super::search::{self, RankedMemory};
use super::cache;
use super::{content_hash, dedup_by_id, MemoryEntry, MemoryType, MemoryTier};

/// SQLite-backed memory store.
///
/// Wraps `MemoryDatabase` and provides high-level CRUD + search operations
/// that the existing `MemoryManager` API expects.
pub struct SqliteMemoryStore {
    db: Arc<MemoryDatabase>,
    /// Embedding provider for generating dense vectors.
    embedding: Arc<dyn EmbeddingProvider>,
}

impl std::fmt::Debug for SqliteMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteMemoryStore")
            .field("embedding_dim", &self.db.embedding_dim())
            .finish()
    }
}

impl SqliteMemoryStore {
    /// Create a new SQLite memory store.
    pub fn new(db: Arc<MemoryDatabase>, embedding: Arc<dyn EmbeddingProvider>) -> Self {
        Self { db, embedding }
    }

    /// Returns a reference to the underlying database.
    pub fn db(&self) -> &Arc<MemoryDatabase> {
        &self.db
    }

    /// Store a memory entry. Returns the entry ID.
    ///
    /// Inserts into `memories` table, FTS5 (via trigger), and sqlite-vec.
    pub async fn remember(&self, entry: &MemoryEntry) -> Result<String> {
        let id = entry.id.clone();

        let tags_json = serde_json::to_string(&entry.tags)?;
        let tier_label = match entry.tier {
            MemoryTier::Hot => "hot",
            MemoryTier::Warm => "warm",
            MemoryTier::Cold => "cold",
        };
        let protection_label = match entry.protection {
            crate::memory::ProtectionLevel::None => "none",
            crate::memory::ProtectionLevel::Low => "low",
            crate::memory::ProtectionLevel::Medium => "medium",
            crate::memory::ProtectionLevel::High => "high",
            crate::memory::ProtectionLevel::Permanent => "permanent",
        };

        // Insert into SQLite (scoped lock — guard dropped before any await)
        let rowid: i64 = {
            let conn = self.db.conn();
            conn.execute(
                "INSERT OR REPLACE INTO memories
                 (id, memory_type, content, importance, tier, protection, source,
                  session_id, space_id, tags, access_count, pinned, auto_classified,
                  session_appearances, decay_score, compaction_level, content_hash,
                  created_at, updated_at, accessed_at, decay_rate)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                         ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                rusqlite::params![
                    entry.id,
                    entry.memory_type.label(),
                    entry.content,
                    entry.importance,
                    tier_label,
                    protection_label,
                    entry.source,
                    entry.session_id,
                    entry.space_id,
                    tags_json,
                    entry.access_count as i64,
                    entry.pinned as i64,
                    entry.auto_classified as i64,
                    entry.session_appearances as i64,
                    entry.decay_score,
                    entry.compaction_level as i64,
                    entry.content_hash as i64,
                    entry.created_at.to_rfc3339(),
                    entry.modified_at.to_rfc3339(),
                    entry.accessed_at.to_rfc3339(),
                    entry.memory_type.base_decay_rate(),
                ],
            )?;

            conn.query_row(
                "SELECT rowid FROM memories WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            ).unwrap_or(0)
        }; // conn dropped here, before any .await

        // Compute and store dense embedding
        let embedding_vec = self.embedding.embed(&entry.content).await?;
        if let Some(f32_vec) = embedding_vec.to_f32_dense() {
            if let Err(e) = memory_insert_vector(&self.db, rowid, &f32_vec) {
                tracing::debug!(id = %id, error = %e, "Failed to insert vector (non-fatal)");
            }

            // Cache the embedding
            if let Err(e) = cache::put_cached(&self.db, &entry.content, &f32_vec) {
                tracing::debug!(id = %id, error = %e, "Failed to cache embedding (non-fatal)");
            }
        }

        tracing::debug!(id = %id, ty = entry.memory_type.label(), "Memory stored (SQLite)");
        Ok(id)
    }

    /// Retrieve a single memory by ID and type.
    pub fn get(&self, id: &str, _memory_type: MemoryType) -> Result<Option<MemoryEntry>> {
        search::load_memory_by_id(&self.db, id)
    }

    /// Retrieve a memory by ID (searches all types).
    pub fn get_by_id(&self, id: &str) -> Result<Option<MemoryEntry>> {
        search::load_memory_by_id(&self.db, id)
    }

    /// Delete a memory entry.
    pub fn forget(&self, id: &str, _memory_type: MemoryType) -> Result<bool> {
        let conn = self.db.conn();

        // Get rowid for vector deletion
        let rowid: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM memories WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .ok();

        let deleted = conn.execute(
            "DELETE FROM memories WHERE id = ?1",
            rusqlite::params![id],
        )? > 0;

        drop(conn);

        if let Some(rowid) = rowid {
            let _ = memory_delete_vector(&self.db, rowid);
        }

        Ok(deleted)
    }

    /// List memories of a given type, most recent first.
    pub fn list(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<MemoryEntry>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, memory_type, content, importance, tier, protection,
                    source, session_id, space_id, tags, access_count, pinned,
                    auto_classified, session_appearances, decay_score, content_hash,
                    created_at, updated_at, accessed_at
             FROM memories
             WHERE memory_type = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let entries: Vec<MemoryEntry> = stmt
            .query_map(rusqlite::params![memory_type.label(), limit], |row| {
                Ok(search::row_to_memory_entry(row))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Search memories using BM25 + optional vector KNN with RRF fusion.
    pub async fn search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        // Compute query embedding (with caching)
        let query_vec = self.get_query_vector(query).await?;

        let results = search::search(
            &self.db,
            query_vec.as_deref(),
            query,
            memory_type,
            limit,
        )?;

        Ok(results.into_iter().map(|r| r.entry).collect())
    }

    /// Semantic search returning scored results.
    pub async fn semantic_search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<RankedMemory>> {
        let query_vec = self.get_query_vector(query).await?;
        search::search(&self.db, query_vec.as_deref(), query, memory_type, limit)
    }

    /// Recall relevant memories for a new session.
    pub async fn recall(&self, query: &str, max_recall: usize) -> Result<Vec<MemoryEntry>> {
        // 1. Recent conversation summaries
        let recent = self.list(MemoryType::Conversation, 3).unwrap_or_default();

        // 2. Recent session summaries
        let sessions = self.list(MemoryType::Session, 2).unwrap_or_default();

        // 3. Search for relevant facts/episodes
        let relevant = self.search(query, None, max_recall).await.unwrap_or_default();

        // 4. Combine and deduplicate
        let mut combined = recent;
        combined.extend(sessions);
        combined.extend(relevant);
        dedup_by_id(&mut combined);
        combined.truncate(max_recall);
        Ok(combined)
    }

    /// Count total entries in the database.
    pub fn total_entries(&self) -> usize {
        let conn = self.db.conn();
        conn.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Count entries by type.
    pub fn count_by_type(&self, memory_type: MemoryType) -> usize {
        let conn = self.db.conn();
        conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE memory_type = ?1",
            rusqlite::params![memory_type.label()],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize
    }

    /// Blend recalled memories into the system prompt.
    pub fn blend_into_prompt(&self, memories: &[MemoryEntry], system_prompt: &str) -> String {
        if memories.is_empty() {
            return system_prompt.to_string();
        }

        let memory_block = memories
            .iter()
            .map(|m| format!("- [{}] {}", m.memory_type.label(), m.content))
            .collect::<Vec<_>>()
            .join("\n");

        format!("{system_prompt}\n\n## Relevant Memory\n\n{memory_block}")
    }

    /// Check if a memory entry with identical content already exists.
    pub async fn is_duplicate(&self, content: &str) -> bool {
        // Check content hash first (fast)
        let hash = content_hash(content);
        let exists: bool = {
            let conn = self.db.conn();
            conn.query_row(
                "SELECT 1 FROM memories WHERE content_hash = ?1 LIMIT 1",
                rusqlite::params![hash as i64],
                |row| row.get::<_, i64>(0),
            ).is_ok()
        }; // conn dropped

        if exists {
            return true;
        }

        // Then check semantic similarity
        if let Ok(vec) = self.embedding.embed(content).await {
            if let Some(f32_vec) = vec.to_f32_dense() {
                if let Ok(hits) = super::search::vector::search_vector(&self.db, &f32_vec, 5) {
                    for hit in hits {
                        if hit.distance < 0.05 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Store a memory entry only if no duplicate content exists.
    pub async fn remember_unique(&self, entry: &MemoryEntry) -> Result<Option<String>> {
        if self.is_duplicate(&entry.content).await {
            tracing::debug!(id = %entry.id, "Skipping duplicate memory (SQLite)");
            return Ok(None);
        }
        let id = self.remember(entry).await?;
        Ok(Some(id))
    }

    /// Run JSON → SQLite migration if needed.
    pub fn migrate_if_needed(&self, workspace_dir: &std::path::Path) -> Result<()> {
        super::migration::migrate_json_to_sqlite(workspace_dir, &self.db)?;
        Ok(())
    }

    // ── Private helpers ─────────────────────────────────────────────

    /// Get or compute a query embedding, using the cache.
    async fn get_query_vector(&self, query: &str) -> Result<Option<Vec<f32>>> {
        // Check cache first
        if let Ok(Some(cached)) = cache::get_cached(&self.db, query) {
            return Ok(Some(cached));
        }

        // Compute
        let vec = self.embedding.embed(query).await?;
        let f32_vec = match vec.to_f32_dense() {
            Some(v) => v,
            None => return Ok(None),
        };

        // Cache (best effort)
        let _ = cache::put_cached(&self.db, query, &f32_vec);

        Ok(Some(f32_vec))
    }
}

// ---------------------------------------------------------------------------
// Re-export search helper functions from sub-modules
// ---------------------------------------------------------------------------

use crate::embedding::EmbeddingProvider;

fn memory_insert_vector(db: &MemoryDatabase, rowid: i64, vector: &[f32]) -> anyhow::Result<()> {
    super::search::vector::insert_vector(db, rowid, vector)
}

fn memory_delete_vector(db: &MemoryDatabase, rowid: i64) -> anyhow::Result<()> {
    super::search::vector::delete_vector(db, rowid)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::TfIdfEmbeddingProvider;
    use crate::memory::{MemoryTier, ProtectionLevel};

    fn make_test_entry(id: &str, ty: MemoryType, content: &str) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            memory_type: ty,
            tier: MemoryTier::Warm,
            content: content.to_string(),
            content_hash: content_hash(content),
            source: "test".to_string(),
            session_id: None,
            space_id: None,
            tags: vec![],
            importance: 0.5,
            pinned: false,
            protection: ProtectionLevel::None,
            auto_classified: false,
            session_appearances: 0,
            user_corrected: false,
            seen_in_sessions: vec![],
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            access_count: 0,
            decay_score: 1.0,
            compaction_level: 0,
            compacted_from: vec![],
            related_ids: vec![],
            contradicts: None,
        }
    }

    fn make_store() -> SqliteMemoryStore {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let embedding: Arc<dyn EmbeddingProvider> = Arc::new(TfIdfEmbeddingProvider);
        SqliteMemoryStore::new(Arc::new(db), embedding)
    }

    #[tokio::test]
    async fn test_remember_and_get() {
        let store = make_store();

        let entry = make_test_entry("sqlite-test-1", MemoryType::Fact, "Rust is a systems language");
        store.remember(&entry).await.unwrap();

        let loaded = store.get("sqlite-test-1", MemoryType::Fact).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "sqlite-test-1");
        assert_eq!(loaded.content, "Rust is a systems language");
    }

    #[tokio::test]
    async fn test_forget() {
        let store = make_store();

        let entry = make_test_entry("forget-test-1", MemoryType::Fact, "to be deleted");
        store.remember(&entry).await.unwrap();
        assert!(store.get("forget-test-1", MemoryType::Fact).unwrap().is_some());

        let deleted = store.forget("forget-test-1", MemoryType::Fact).unwrap();
        assert!(deleted);
        assert!(store.get("forget-test-1", MemoryType::Fact).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list() {
        let store = make_store();

        store.remember(&make_test_entry("list-1", MemoryType::Fact, "fact 1")).await.unwrap();
        store.remember(&make_test_entry("list-2", MemoryType::Fact, "fact 2")).await.unwrap();
        store.remember(&make_test_entry("list-3", MemoryType::Episode, "episode 1")).await.unwrap();

        let facts = store.list(MemoryType::Fact, 10).unwrap();
        assert_eq!(facts.len(), 2);

        let episodes = store.list(MemoryType::Episode, 10).unwrap();
        assert_eq!(episodes.len(), 1);
    }

    #[tokio::test]
    async fn test_search_bm25() {
        let store = make_store();

        store.remember(&make_test_entry("s-1", MemoryType::Fact, "Rust programming language safety")).await.unwrap();
        store.remember(&make_test_entry("s-2", MemoryType::Fact, "Python data science machine learning")).await.unwrap();

        let results = store.search("Rust programming", None, 10).await.unwrap();
        assert!(!results.is_empty(), "BM25 search should find results");
        assert_eq!(results[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_search_with_type_filter() {
        let store = make_store();

        store.remember(&make_test_entry("tf-1", MemoryType::Fact, "test content fact")).await.unwrap();
        store.remember(&make_test_entry("tf-2", MemoryType::Episode, "test content episode")).await.unwrap();

        let results = store.search("test", Some(MemoryType::Fact), 10).await.unwrap();
        assert!(results.iter().all(|r| r.memory_type == MemoryType::Fact));
    }

    #[tokio::test]
    async fn test_recall() {
        let store = make_store();

        store.remember(&make_test_entry("rc-1", MemoryType::Fact, "Rust memory safety")).await.unwrap();
        store.remember(&make_test_entry("rc-2", MemoryType::Conversation, "User asked about Rust")).await.unwrap();

        let results = store.recall("Rust safety", 10).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_blend_into_prompt() {
        let store = make_store();
        let memories = vec![make_test_entry("bl-1", MemoryType::Fact, "test fact")];
        let result = store.blend_into_prompt(&memories, "You are an agent.");
        assert!(result.contains("## Relevant Memory"));
        assert!(result.contains("[fact]"));
    }

    #[tokio::test]
    async fn test_blend_empty() {
        let store = make_store();
        let result = store.blend_into_prompt(&[], "You are an agent.");
        assert_eq!(result, "You are an agent.");
    }

    #[tokio::test]
    async fn test_total_entries() {
        let store = make_store();
        assert_eq!(store.total_entries(), 0);

        store.remember(&make_test_entry("cnt-1", MemoryType::Fact, "one")).await.unwrap();
        store.remember(&make_test_entry("cnt-2", MemoryType::Episode, "two")).await.unwrap();
        assert_eq!(store.total_entries(), 2);
    }

    #[tokio::test]
    async fn test_update_entry() {
        let store = make_store();

        let mut entry = make_test_entry("upd-1", MemoryType::Fact, "original content");
        store.remember(&entry).await.unwrap();

        entry.content = "updated content".to_string();
        store.remember(&entry).await.unwrap();

        let loaded = store.get("upd-1", MemoryType::Fact).unwrap().unwrap();
        assert_eq!(loaded.content, "updated content");
        assert_eq!(store.total_entries(), 1);
    }

    #[tokio::test]
    async fn test_is_duplicate() {
        let store = make_store();

        store.remember(&make_test_entry("dup-1", MemoryType::Fact, "unique content here")).await.unwrap();

        // Same content hash
        assert!(store.is_duplicate("unique content here").await);

        // Different content
        assert!(!store.is_duplicate("completely different stuff").await);
    }

    #[tokio::test]
    async fn test_remember_unique() {
        let store = make_store();

        let entry = make_test_entry("uniq-1", MemoryType::Fact, "unique entry");
        let result = store.remember_unique(&entry).await.unwrap();
        assert!(result.is_some());

        // Same content → should be skipped
        let entry2 = make_test_entry("uniq-2", MemoryType::Fact, "unique entry");
        let result2 = store.remember_unique(&entry2).await.unwrap();
        assert!(result2.is_none());
    }
}
