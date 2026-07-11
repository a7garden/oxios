//! SQLite-backed memory store (RFC-012).
//!
//! Provides `remember()`, `search()`, `recall()`, `get()`, `forget()`
//! operations using the SQLite `memory.db` as the single source of truth.
//!
//! When the `sqlite-memory` feature is enabled and `memory.sqlite.enabled`
//! is true in config, MemoryManager delegates to this store instead of
//! the file-based StateStore.

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;

use super::cache;
use super::database::MemoryDatabase;
use super::search::{self, RankedMemory};
use crate::memory::types::{MemoryEntry, MemoryTier, MemoryType, content_hash, dedup_by_id};

/// A learning pattern row from the `patterns` table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PatternRow {
    /// Unique pattern ID.
    pub id: String,
    /// Strategy name (e.g., "sona").
    pub strategy: String,
    /// Optional domain.
    pub domain: Option<String>,
    /// Quality score (0.0–1.0).
    pub quality: f32,
    /// Number of times this pattern was used.
    pub use_count: u32,
    /// Success rate (0.0–1.0).
    pub success_rate: f32,
    /// Whether this pattern is long-term.
    pub is_long_term: bool,
    /// Pattern data as JSON.
    pub data: String,
    /// When created.
    pub created_at: String,
    /// When last updated.
    pub updated_at: String,
}

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
            // INSERT OR REPLACE deletes then re-inserts the row, which assigns a
            // NEW rowid. The vec0 table (`memory_vectors`) is keyed by rowid and
            // is NOT covered by the FTS sync triggers, so the vector for the old
            // rowid would otherwise linger forever and grow the KNN index with
            // dead entries. Delete it before the replace (inline, since we
            // already hold the connection lock and parking_lot::Mutex is
            // non-reentrant).
            if let Ok(old_rowid) = conn.query_row::<i64, _, _>(
                "SELECT rowid FROM memories WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            ) {
                let _ = conn.execute(
                    "DELETE FROM memory_vectors WHERE rowid = ?1",
                    rusqlite::params![old_rowid],
                );
            }
            conn.execute(
                "INSERT OR REPLACE INTO memories
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
            )
            .unwrap_or(0)
        }; // conn dropped here, before any .await

        // Compute dense embedding (non-fatal: a transient API error must not
        // lose the memory entry — text + FTS5 are already inserted above;
        // we skip the vector and continue, matching mnemopi's graceful
        // degradation. See Phase 2b in the design doc.)
        let f32_vec = match self.embedding.embed(&entry.content).await {
            Ok(v) => v.to_f32_dense(),
            Err(e) => {
                tracing::warn!(
                    id = %id,
                    error = %e,
                    "Embedding API failed; memory stored without vector"
                );
                None
            }
        };
        if let Some(f32_vec) = f32_vec {
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

        let deleted =
            conn.execute("DELETE FROM memories WHERE id = ?1", rusqlite::params![id])? > 0;

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
                    source, session_id, tags, access_count, pinned,
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
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
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

        let results = search::search(&self.db, query_vec.as_deref(), query, memory_type, limit)?;

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
        let relevant = self
            .search(query, None, max_recall)
            .await
            .unwrap_or_default();

        // 4. Combine and deduplicate
        let mut combined = recent;
        combined.extend(sessions);
        combined.extend(relevant);
        dedup_by_id(&mut combined);
        combined.truncate(max_recall);
        Ok(combined)
    }

    /// Recall with Flash Attention re-ranking (Phase 6).
    ///
    /// First does standard recall, then re-ranks results using
    /// Flash Attention to compute context-aware relevance scores.
    ///
    /// The query and memory embeddings form the Q/K/V of the attention
    /// mechanism. The output attention weights determine final ranking.
    pub async fn recall_with_rerank(
        &self,
        query: &str,
        max_recall: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let candidates = self.recall(query, max_recall * 3).await?;
        if candidates.len() <= max_recall {
            return Ok(candidates);
        }

        // Get query embedding
        let query_vec = match self.get_query_vector(query).await? {
            Some(v) => v,
            None => return Ok(candidates.into_iter().take(max_recall).collect()),
        };

        // Get candidate embeddings
        let mut candidate_vecs: Vec<(MemoryEntry, Vec<f32>)> = Vec::new();
        for entry in &candidates {
            if let Ok(Some(vec)) = self.get_query_vector(&entry.content).await {
                candidate_vecs.push((entry.clone(), vec));
            }
        }

        if candidate_vecs.is_empty() {
            return Ok(candidates.into_iter().take(max_recall).collect());
        }

        // Flash Attention re-ranking
        let fa = crate::memory::flash_attention::FlashAttention::with_dimensions(query_vec.len());

        let queries = vec![query_vec.clone()];
        let keys: Vec<Vec<f32>> = candidate_vecs.iter().map(|(_, v)| v.clone()).collect();
        let values = keys.clone(); // K=V for self-supervised re-ranking

        let attention_output = fa.attention(&queries, &keys, &values);
        let output = match attention_output.first() {
            Some(o) => o,
            None => return Ok(candidates.into_iter().take(max_recall).collect()),
        };

        // Score candidates by similarity to attention output
        let mut scored: Vec<(MemoryEntry, f32)> = candidate_vecs
            .into_iter()
            .zip(keys.iter())
            .map(|((entry, _), key_vec)| {
                let similarity = cosine_similarity(output, key_vec);
                (entry, similarity)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_recall);

        Ok(scored.into_iter().map(|(e, _)| e).collect())
    }

    /// Count total entries in the database.
    pub fn total_entries(&self) -> usize {
        let conn = self.db.conn();
        conn.query_row("SELECT COUNT(*) FROM memories", [], |row| {
            row.get::<_, i64>(0)
        })
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
            )
            .is_ok()
        }; // conn dropped

        if exists {
            return true;
        }

        // Then check semantic similarity
        if let Ok(vec) = self.embedding.embed(content).await
            && let Some(f32_vec) = vec.to_f32_dense()
            && let Ok(hits) = super::search::vector::search_vector(&self.db, &f32_vec, 5)
        {
            for hit in hits {
                if hit.distance < 0.05 {
                    return true;
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

    // ── Phase 2: MemoryGraph / PageRank ────────────────────────────────

    /// Build a co-access graph from memory session history.
    ///
    /// Groups memories by `session_id` and links all co-accessed pairs.
    /// Returns a `MemoryGraph` ready for PageRank computation.
    pub fn build_co_access_graph(&self) -> crate::memory::graph::MemoryGraph {
        let conn = self.db.conn();

        // Collect session_id -> [rowid] mappings
        let mut sessions: std::collections::HashMap<String, Vec<u64>> =
            std::collections::HashMap::new();

        let mut stmt = match conn
            .prepare("SELECT rowid, session_id FROM memories WHERE session_id IS NOT NULL")
        {
            Ok(s) => s,
            Err(_) => return crate::memory::graph::MemoryGraph::new(),
        };

        let rows: Vec<(i64, String)> = match stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        }) {
            Ok(mapped) => mapped
                .filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                        None
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        drop(stmt);
        drop(conn);

        for (rowid, session_id) in rows {
            sessions.entry(session_id).or_default().push(rowid as u64);
        }

        let session_vecs: Vec<Vec<u64>> = sessions.into_values().collect();
        crate::memory::graph::MemoryGraph::from_co_access(&session_vecs)
    }

    /// Compute PageRank-based importance scores for all memories.
    ///
    /// Returns a map of memory rowid -> PageRank score.
    /// Higher scores indicate memories that are more "central" in the
    /// co-access graph — they bridge topics and appear in many sessions.
    pub fn compute_pagerank(
        &self,
        damping: f64,
        iterations: usize,
        initial_scores: Option<&std::collections::HashMap<u64, f64>>,
    ) -> std::collections::HashMap<u64, f64> {
        let graph = self.build_co_access_graph();
        graph.pagerank(damping, iterations, initial_scores)
    }

    /// Apply PageRank scores as importance boosts.
    ///
    /// For each memory, the importance is updated as:
    /// `new_importance = old_importance * (1 + pagerank_boost * pagerank_score)`
    ///
    /// Returns the number of entries updated.
    pub fn apply_pagerank_boost(
        &self,
        pagerank_scores: &std::collections::HashMap<u64, f64>,
        boost_factor: f32,
    ) -> usize {
        let conn = self.db.conn();
        let mut updated = 0;

        for (&rowid, &score) in pagerank_scores {
            // Get current importance
            let importance: Option<f32> = conn
                .query_row(
                    "SELECT importance FROM memories WHERE rowid = ?1",
                    rusqlite::params![rowid as i64],
                    |row| row.get(0),
                )
                .ok();

            if let Some(old_importance) = importance {
                let new_importance =
                    (old_importance * (1.0 + boost_factor * score as f32)).clamp(0.0, 1.0);

                if conn
                    .execute(
                        "UPDATE memories SET importance = ?1 WHERE rowid = ?2",
                        rusqlite::params![new_importance, rowid as i64],
                    )
                    .is_ok()
                {
                    updated += 1;
                }
            }
        }

        updated
    }

    /// List memories by tier.
    pub fn list_by_tier(&self, tier: MemoryTier, limit: usize) -> Result<Vec<MemoryEntry>> {
        let tier_label = match tier {
            MemoryTier::Hot => "hot",
            MemoryTier::Warm => "warm",
            MemoryTier::Cold => "cold",
        };

        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, memory_type, content, importance, tier, protection,
                    source, session_id, tags, access_count, pinned,
                    auto_classified, session_appearances, decay_score, content_hash,
                    created_at, updated_at, accessed_at
             FROM memories
             WHERE tier = ?1
             ORDER BY importance DESC
             LIMIT ?2",
        )?;

        let entries: Vec<MemoryEntry> = stmt
            .query_map(rusqlite::params![tier_label, limit], |row| {
                Ok(search::row_to_memory_entry(row))
            })?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
            .collect();

        Ok(entries)
    }

    /// Update a memory entry in-place.
    pub fn update_entry(&self, entry: &MemoryEntry) -> Result<()> {
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

        let conn = self.db.conn();
        conn.execute(
            "UPDATE memories SET
                memory_type = ?2, content = ?3, importance = ?4, tier = ?5,
                protection = ?6, source = ?7, session_id = ?8,
                tags = ?9, access_count = ?10, pinned = ?11, auto_classified = ?12,
                session_appearances = ?13, decay_score = ?14, compaction_level = ?15,
                content_hash = ?16, updated_at = ?17, accessed_at = ?18
             WHERE id = ?1",
            rusqlite::params![
                entry.id,
                entry.memory_type.label(),
                entry.content,
                entry.importance,
                tier_label,
                protection_label,
                entry.source,
                entry.session_id,
                serde_json::to_string(&entry.tags)?,
                entry.access_count as i64,
                entry.pinned as i64,
                entry.auto_classified as i64,
                entry.session_appearances as i64,
                entry.decay_score,
                entry.compaction_level as i64,
                entry.content_hash as i64,
                entry.modified_at.to_rfc3339(),
                entry.accessed_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    // ── Phase 4: Learning Patterns (SONA) ──────────────

    /// Store a learning pattern.
    pub fn save_pattern(
        &self,
        id: &str,
        strategy: &str,
        domain: Option<&str>,
        quality: f32,
        data: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.db.conn();
        conn.execute(
            "INSERT OR REPLACE INTO patterns
             (id, strategy, domain, quality, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, strategy, domain, quality, data, now, now],
        )?;
        Ok(())
    }

    /// Load all learning patterns.
    pub fn load_patterns(&self) -> Result<Vec<PatternRow>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, strategy, domain, quality, use_count, success_rate,
                    is_long_term, data, created_at, updated_at
             FROM patterns
             ORDER BY quality DESC",
        )?;

        let rows: Vec<PatternRow> = stmt
            .query_map([], |row| {
                Ok(PatternRow {
                    id: row.get(0)?,
                    strategy: row.get(1)?,
                    domain: row.get(2)?,
                    quality: row.get(3)?,
                    use_count: row.get::<_, i64>(4)? as u32,
                    success_rate: row.get(5)?,
                    is_long_term: row.get::<_, i64>(6)? != 0,
                    data: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
                    None
                }
            })
            .collect();

        Ok(rows)
    }

    /// Record a pattern usage.
    pub fn record_pattern_usage(&self, id: &str, success: bool) -> Result<()> {
        let conn = self.db.conn();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE patterns SET
                use_count = use_count + 1,
                success_rate = CASE WHEN use_count = 0 THEN ?1
                    ELSE (success_rate * use_count + ?1) / (use_count + 1) END,
                updated_at = ?2
             WHERE id = ?3",
            rusqlite::params![success as i32 as f32, now, id],
        )?;
        Ok(())
    }

    /// Auto-promote high-quality patterns to long-term storage.
    ///
    /// Patterns with quality >= `min_quality` and use_count >= `min_usage`
    /// are marked as long-term.
    pub fn auto_promote_patterns(&self, min_quality: f32, min_usage: u32) -> usize {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE patterns SET is_long_term = 1
             WHERE quality >= ?1 AND use_count >= ?2 AND is_long_term = 0",
            rusqlite::params![min_quality, min_usage as i64],
        )
        .unwrap_or(0)
    }

    // ── Private helpers ─────────────────────────────────────────────

    /// Get or compute a query embedding, using the cache.
    pub async fn get_query_vector(&self, query: &str) -> Result<Option<Vec<f32>>> {
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

    /// Backfill dense vectors for memory rows that don't yet have one.
    ///
    /// Called on first boot with a dense embedding provider (Phase 2c in the
    /// design doc). Iterates `memories` rows whose `rowid` is missing from
    /// the `memory_vectors` vec0 table, computes their embedding, and
    /// inserts the resulting f32 vector. Sparse (TF-IDF) providers skip
    /// this method since `EmbeddingVector::to_f32_dense()` returns None.
    ///
    /// Returns the number of rows successfully backfilled.
    pub async fn backfill_vectors(&self) -> Result<usize> {
        // Collect target rowids (lock dropped before any await).
        let missing: Vec<(i64, String)> = {
            let conn = self.db.conn();
            let mut stmt = conn
                .prepare(
                    "SELECT m.rowid, m.content
                     FROM memories m
                     LEFT JOIN memory_vectors_rowids v ON v.rowid = m.rowid
                     WHERE v.rowid IS NULL",
                )
                .context("prepare backfill query")?;
            stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .context("query backfill candidates")?
            .filter_map(Result::ok)
            .collect()
        };

        let total = missing.len();
        if total == 0 {
            tracing::debug!("backfill_vectors: nothing to do");
            return Ok(0);
        }
        tracing::info!(count = total, "backfill_vectors: starting");

        let mut done = 0usize;
        for (rowid, content) in missing {
            // Per-row embedding: errors are skipped (best-effort backfill).
            let embedding_vec = match self.embedding.embed(&content).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(rowid, error = %e, "backfill embedding failed; skipping row");
                    continue;
                }
            };
            let Some(f32_vec) = embedding_vec.to_f32_dense() else {
                tracing::debug!(
                    rowid,
                    "backfill: provider returned non-dense vector; skipping"
                );
                continue;
            };
            if memory_insert_vector(&self.db, rowid, &f32_vec).is_ok() {
                done += 1;
            }
        }
        tracing::info!(done, total, "backfill_vectors: complete");
        Ok(done)
    }

    /// Detect embedding-dimension mismatch with the stored vec0 table.
    ///
    /// On mismatch (e.g. user switched from `text-embedding-3-small` 1536-dim
    /// to `text-embedding-3-large` 3072-dim), wipe the `memory_vectors`
    /// table so subsequent writes use the new dimension. The text rows are
    /// unaffected — vectors will be rebuilt via `backfill_vectors()`.
    ///
    /// Mirrors mnemopi's `reconcileEmbeddingModel()` (`memory.ts:435`).
    pub fn reconcile_vector_dimension(&self, new_dim: usize) -> Result<()> {
        let conn = self.db.conn();
        let stored_len: Option<i64> = conn
            .query_row(
                "SELECT length(embedding) FROM memory_vectors LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        if let Some(bytes) = stored_len {
            // f32 stored as 4 bytes each, so dim = bytes / 4.
            let stored_dim = (bytes as usize) / 4;
            if stored_dim != new_dim {
                tracing::warn!(
                    stored_dim,
                    new_dim,
                    "reconcile_vector_dimension: dimension mismatch; wiping memory_vectors",
                );
                conn.execute("DELETE FROM memory_vectors", [])
                    .context("wipe memory_vectors on dimension mismatch")?;
            }
        }
        Ok(())
    }
}

// ── MemoryBackend trait impl ──────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::memory::backend::MemoryBackend for SqliteMemoryStore {
    async fn remember(&self, entry: &MemoryEntry) -> anyhow::Result<String> {
        SqliteMemoryStore::remember(self, entry).await
    }

    fn get(&self, id: &str, memory_type: MemoryType) -> anyhow::Result<Option<MemoryEntry>> {
        SqliteMemoryStore::get(self, id, memory_type)
    }

    fn get_by_id(&self, id: &str) -> anyhow::Result<Option<MemoryEntry>> {
        SqliteMemoryStore::get_by_id(self, id)
    }

    fn forget(&self, id: &str, memory_type: MemoryType) -> anyhow::Result<bool> {
        SqliteMemoryStore::forget(self, id, memory_type)
    }

    fn list(&self, memory_type: MemoryType, limit: usize) -> anyhow::Result<Vec<MemoryEntry>> {
        SqliteMemoryStore::list(self, memory_type, limit)
    }

    async fn search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        SqliteMemoryStore::search(self, query, memory_type, limit).await
    }

    async fn recall(&self, query: &str, max_recall: usize) -> anyhow::Result<Vec<MemoryEntry>> {
        SqliteMemoryStore::recall(self, query, max_recall).await
    }

    async fn recall_with_rerank(
        &self,
        query: &str,
        max_recall: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        SqliteMemoryStore::recall_with_rerank(self, query, max_recall).await
    }

    fn blend_into_prompt(&self, memories: &[MemoryEntry], system_prompt: &str) -> String {
        SqliteMemoryStore::blend_into_prompt(self, memories, system_prompt)
    }

    async fn is_duplicate(&self, content: &str) -> bool {
        SqliteMemoryStore::is_duplicate(self, content).await
    }

    async fn remember_unique(&self, entry: &MemoryEntry) -> anyhow::Result<Option<String>> {
        SqliteMemoryStore::remember_unique(self, entry).await
    }

    fn list_by_tier(&self, tier: MemoryTier, limit: usize) -> anyhow::Result<Vec<MemoryEntry>> {
        SqliteMemoryStore::list_by_tier(self, tier, limit)
    }
}

// ---------------------------------------------------------------------------
// Re-export search helper functions from sub-modules
// ---------------------------------------------------------------------------

use crate::memory::embedding::EmbeddingProvider;

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

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
    use crate::memory::embedding::TfIdfEmbeddingProvider;
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

        let entry = make_test_entry(
            "sqlite-test-1",
            MemoryType::Fact,
            "Rust is a systems language",
        );
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
        assert!(
            store
                .get("forget-test-1", MemoryType::Fact)
                .unwrap()
                .is_some()
        );

        let deleted = store.forget("forget-test-1", MemoryType::Fact).unwrap();
        assert!(deleted);
        assert!(
            store
                .get("forget-test-1", MemoryType::Fact)
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_list() {
        let store = make_store();

        store
            .remember(&make_test_entry("list-1", MemoryType::Fact, "fact 1"))
            .await
            .unwrap();
        store
            .remember(&make_test_entry("list-2", MemoryType::Fact, "fact 2"))
            .await
            .unwrap();
        store
            .remember(&make_test_entry("list-3", MemoryType::Episode, "episode 1"))
            .await
            .unwrap();

        let facts = store.list(MemoryType::Fact, 10).unwrap();
        assert_eq!(facts.len(), 2);

        let episodes = store.list(MemoryType::Episode, 10).unwrap();
        assert_eq!(episodes.len(), 1);
    }

    #[tokio::test]
    async fn test_search_bm25() {
        let store = make_store();

        store
            .remember(&make_test_entry(
                "s-1",
                MemoryType::Fact,
                "Rust programming language safety",
            ))
            .await
            .unwrap();
        store
            .remember(&make_test_entry(
                "s-2",
                MemoryType::Fact,
                "Python data science machine learning",
            ))
            .await
            .unwrap();

        let results = store.search("Rust programming", None, 10).await.unwrap();
        assert!(!results.is_empty(), "BM25 search should find results");
        assert_eq!(results[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_search_with_type_filter() {
        let store = make_store();

        store
            .remember(&make_test_entry(
                "tf-1",
                MemoryType::Fact,
                "test content fact",
            ))
            .await
            .unwrap();
        store
            .remember(&make_test_entry(
                "tf-2",
                MemoryType::Episode,
                "test content episode",
            ))
            .await
            .unwrap();

        let results = store
            .search("test", Some(MemoryType::Fact), 10)
            .await
            .unwrap();
        assert!(results.iter().all(|r| r.memory_type == MemoryType::Fact));
    }

    #[tokio::test]
    async fn test_recall() {
        let store = make_store();

        store
            .remember(&make_test_entry(
                "rc-1",
                MemoryType::Fact,
                "Rust memory safety",
            ))
            .await
            .unwrap();
        store
            .remember(&make_test_entry(
                "rc-2",
                MemoryType::Conversation,
                "User asked about Rust",
            ))
            .await
            .unwrap();

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

        store
            .remember(&make_test_entry("cnt-1", MemoryType::Fact, "one"))
            .await
            .unwrap();
        store
            .remember(&make_test_entry("cnt-2", MemoryType::Episode, "two"))
            .await
            .unwrap();
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

        store
            .remember(&make_test_entry(
                "dup-1",
                MemoryType::Fact,
                "unique content here",
            ))
            .await
            .unwrap();

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
