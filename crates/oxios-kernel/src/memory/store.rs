//! Memory store operations: save/load, index management, search.
//!
//! Integrates HNSW index (usearch) for fast approximate nearest neighbor search
//! alongside the existing file-based state store for persistence.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::embedding::EmbeddingVector;

use super::l2_normalize_f32;
use super::{
    content_hash, dedup_by_id, extract_keywords, AutoProtector, HnswIndex, MemoryEntry,
    MemoryManager, MemoryTier, MemoryType,
};

// ---------------------------------------------------------------------------
// VectorIndexSnapshot
// ---------------------------------------------------------------------------

/// Snapshot of the vector index for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorIndexSnapshot {
    /// Snapshot creation timestamp.
    created_at: DateTime<Utc>,
    /// Number of entries in the snapshot.
    entry_count: usize,
    /// Map of entry ID to embedding vector.
    entries: HashMap<String, EmbeddingVector>,
}

// ---------------------------------------------------------------------------
// Store & search operations
// ---------------------------------------------------------------------------

impl MemoryManager {
    /// Returns total entries across all memory types (from disk).
    pub async fn total_entries(&self) -> usize {
        let mut total = 0;
        for mt in MemoryType::all() {
            if let Ok(entries) = self.list(*mt, 1_000_000).await {
                total += entries.len();
            }
        }
        total
    }

    /// Rebuild the vector index from all stored memories.
    ///
    /// Call once at startup to populate the in-memory index from
    /// persisted memory entries.
    pub async fn rebuild_index(&self) -> Result<()> {
        // Collect all entries outside the lock
        let mut entries_to_index: Vec<(String, EmbeddingVector)> = Vec::new();

        for mt in MemoryType::all() {
            if let Ok(names) = self.state_store.list_category(mt.category()).await {
                for name in names {
                    if let Ok(Some(entry)) = self
                        .state_store
                        .load_json::<MemoryEntry>(mt.category(), &name)
                        .await
                    {
                        let vector = self.embedding.embed(&entry.content).await?;
                        entries_to_index.push((entry.id.clone(), vector));
                    }
                }
            }
        }

        // Now acquire the lock only for the write
        {
            let mut index = self.vector_index.write();
            index.clear();
            for (id, vector) in entries_to_index {
                index.insert(id, vector);
            }
        }

        tracing::info!(
            entries = self.vector_index.read().len(),
            "Memory vector index rebuilt"
        );
        Ok(())
    }

    /// Save the current vector index to disk as a snapshot.
    pub async fn save_index_snapshot(&self) -> Result<()> {
        let snapshot = {
            let index = self.vector_index.read();
            VectorIndexSnapshot {
                created_at: chrono::Utc::now(),
                entry_count: index.len(),
                entries: index.clone(),
            }
        };

        self.state_store
            .save_json("memory", "vector_index_snapshot", &snapshot)
            .await?;

        self.git_commit("memory/vector_index_snapshot.json", "memory: snapshot save");

        tracing::debug!(
            entries = snapshot.entry_count,
            "Vector index snapshot saved"
        );
        Ok(())
    }

    /// Load a previously saved vector index snapshot from disk.
    pub async fn load_index_snapshot(&self) -> Result<usize> {
        let snapshot: Option<VectorIndexSnapshot> = self
            .state_store
            .load_json("memory", "vector_index_snapshot")
            .await?;

        match snapshot {
            Some(snap) => {
                let count = snap.entry_count;
                let mut index = self.vector_index.write();
                *index = snap.entries;
                tracing::info!(entries = count, "Vector index snapshot loaded");
                Ok(count)
            }
            None => {
                tracing::debug!("No vector index snapshot found");
                Ok(0)
            }
        }
    }

    /// Store a memory entry. Returns the entry ID.
    ///
    /// When SQLite backend is enabled, delegates to `SqliteMemoryStore`.
    /// Otherwise computes and stores the entry's text vector in the in-memory
    /// index for future semantic search.
    pub async fn remember(&self, entry: MemoryEntry) -> Result<String> {
        // ── SQLite fast path (RFC-012) ──
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.remember(&entry).await;
        }

        // ── Legacy JSON path ──
        let id = entry.id.clone();
        let vector = self.embedding.embed(&entry.content).await?;
        let category = entry.memory_type.category();
        self.state_store.save_json(category, &id, &entry).await?;

        self.git_commit(
            &format!("{category}/{id}.json"),
            &format!("memory: store {id}"),
        );

        // Update vector index
        {
            let mut index = self.vector_index.write();
            index.insert(id.clone(), vector.clone());
        }

        // Update HNSW index if attached
        if let Some(f32_vec) = vector.to_f32_dense() {
            let hnsw = self.hnsw_index.read();
            if let Some(ref hnsw) = *hnsw {
                if let Err(e) = hnsw.add_entry(&id, &f32_vec) {
                    tracing::warn!(id = %id, error = %e, "Failed to update HNSW index on remember");
                }
            }
        }

        tracing::debug!(id = %id, ty = entry.memory_type.label(), "Memory stored");
        Ok(id)
    }

    /// Retrieve a single memory by ID.
    ///
    /// Records access for auto-protection tracking.
    pub async fn get(&self, id: &str, memory_type: MemoryType) -> Result<Option<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.get(id, memory_type);
        }
        let result: Option<MemoryEntry> = self
            .state_store
            .load_json(memory_type.category(), id)
            .await?;
        if let Some(mut entry) = result {
            AutoProtector::record_access(&mut entry, "");
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    /// Delete a memory entry.
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> Result<bool> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.forget(id, memory_type);
        }
        let result = self
            .state_store
            .delete_file(memory_type.category(), id)
            .await?;

        // Remove from HNSW index if attached
        {
            let hnsw = self.hnsw_index.read();
            if let Some(ref hnsw) = *hnsw {
                if let Err(e) = hnsw.remove_entry(id) {
                    tracing::warn!(id = %id, error = %e, "Failed to remove from HNSW index on forget");
                }
            }
        }

        Ok(result)
    }

    /// List memories of a given type, most recent first.
    pub async fn list(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.list(memory_type, limit);
        }
        let category = memory_type.category();
        let names = self.state_store.list_category(category).await?;
        let mut entries = Vec::new();
        for name in names.into_iter().take(limit.saturating_mul(2)) {
            if let Ok(Some(entry)) = self
                .state_store
                .load_json::<MemoryEntry>(category, &name)
                .await
            {
                entries.push(entry);
            }
        }
        // Sort by created_at descending (most recent first)
        entries.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        entries.truncate(limit);
        Ok(entries)
    }

    /// Search memories by semantic similarity (vector search).
    ///
    /// Falls back to keyword search when the vector index is empty or
    /// yields no results above the similarity threshold.
    pub async fn search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.search(query, memory_type, limit).await;
        }
        let query_vector = self.embedding.embed(query).await?;

        // Scope the read lock: compute scores, then drop before any await.
        let scored: Vec<(String, f64)> = {
            let index = self.vector_index.read();
            let mut scored: Vec<(String, f64)> = index
                .iter()
                .map(|(id, vector)| {
                    let score = query_vector.cosine_similarity(vector);
                    (id.clone(), score)
                })
                .filter(|(_, score)| *score > 0.1)
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            scored
        }; // lock dropped here, before any .await

        // If index was empty, scored will be empty — fall back immediately
        if scored.is_empty() {
            return self.keyword_search(query, memory_type, limit).await;
        }

        // Determine which memory types to search
        let types: &[MemoryType] = match memory_type {
            Some(ref t) => std::slice::from_ref(t),
            None => MemoryType::all(),
        };

        // Load entries from state store (no lock held)
        let mut results = Vec::new();
        for (id, score) in scored {
            for mt in types {
                if let Ok(Some(mut entry)) = self
                    .state_store
                    .load_json::<MemoryEntry>(mt.category(), &id)
                    .await
                {
                    AutoProtector::record_access(&mut entry, "");
                    tracing::debug!(id = %id, score, "Vector search hit");
                    results.push(entry);
                    break;
                }
            }
        }

        // Fall back to keyword search if no results
        if results.is_empty() {
            return self.keyword_search(query, memory_type, limit).await;
        }

        Ok(results)
    }

    /// Keyword-based search (original algorithm, used as fallback).
    async fn keyword_search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let keywords = extract_keywords(query);
        let types = match memory_type {
            Some(t) => vec![t],
            None => MemoryType::all().to_vec(),
        };

        let mut results = Vec::new();
        for ty in &types {
            let entries = self.list(*ty, limit * 2).await?;
            for entry in entries {
                let matches = keywords.iter().any(|k| {
                    let k_lower = k.to_lowercase();
                    entry.content.to_lowercase().contains(&k_lower)
                        || entry
                            .tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&k_lower))
                });
                if matches {
                    results.push(entry);
                }
            }
        }

        results.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// Recall relevant memories for a new session.
    ///
    /// Combines recent conversation summaries, session summaries,
    /// and keyword-matched facts/episodes.
    pub async fn recall(&self, query: &str) -> Result<Vec<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.recall(query, self.max_recall).await;
        }
        let limit = self.max_recall;

        // 1. Recent conversation summaries (always include)
        let recent = self
            .list(MemoryType::Conversation, 3)
            .await
            .unwrap_or_default();

        // 2. Recent session summaries
        let sessions = self.list(MemoryType::Session, 2).await.unwrap_or_default();

        // 3. Keyword-matched facts and episodes
        let relevant = self.search(query, None, limit).await.unwrap_or_default();

        // 4. Combine and deduplicate
        let mut combined = recent;
        combined.extend(sessions);
        combined.extend(relevant);
        dedup_by_id(&mut combined);
        combined.truncate(limit);
        Ok(combined)
    }

    /// Blend recalled memories into the system prompt.
    pub fn blend_into_prompt(&self, memories: &[MemoryEntry], system_prompt: &str) -> String {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.blend_into_prompt(memories, system_prompt);
        }

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

    /// Recall with Flash Attention re-ranking (Phase 6).
    ///
    /// First does standard recall, then re-ranks using attention-based
    /// scoring for context-aware ordering.
    #[cfg(feature = "sqlite-memory")]
    pub async fn recall_with_rerank(&self, query: &str) -> Result<Vec<MemoryEntry>> {
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.recall_with_rerank(query, self.max_recall).await;
        }
        // Fallback to standard recall
        self.recall(query).await
    }

    /// Recall with proactive enhancement (RFC-020 Phase 1).
    ///
    /// Extends the standard `recall()` with proactive memory injection
    /// based on `RecallTiming` triggers:
    /// - Session first message (count == 0)
    /// - Topic change (Jaccard < 0.3 after 3+ messages)
    /// - Periodic (every 10 messages)
    ///
    /// Avoids duplication by passing existing recalled entries to proactive
    /// recall's `current_context` parameter.
    ///
    /// For SQLite backend, `recall()` already performs BM25+vector hybrid
    /// search, so proactive recall only supplements from Warm tier listings.
    pub async fn recall_with_proactive(
        &self,
        query: &str,
        recall_timing: &mut Option<crate::memory::proactive::RecallTiming>,
    ) -> Result<Vec<MemoryEntry>> {
        // Step 1: Standard recall (Hot + search)
        let mut combined = self.recall(query).await?;

        // Step 2: Proactive enhancement based on timing triggers
        let should_recall = recall_timing
            .as_mut()
            .map(|t| t.should_recall(query))
            .unwrap_or(true);

        if should_recall && combined.len() < self.max_recall {
            // SQLite backend: recall() already did BM25+vector hybrid search.
            // Only supplement with Warm tier entries not already found.
            #[cfg(feature = "sqlite-memory")]
            if self.sqlite_store.is_some() {
                let remaining = self.max_recall - combined.len();
                let warm = self.list_by_tier(MemoryTier::Warm, remaining).await?;
                let mut seen_ids: std::collections::HashSet<String> =
                    combined.iter().map(|e| e.id.clone()).collect();
                for entry in warm {
                    if seen_ids.insert(entry.id.clone()) && combined.len() < self.max_recall {
                        combined.push(entry);
                    }
                }
            }

            // JSON backend (or SQLite without store): proactive recall adds
            // semantic search beyond recall().
            // When sqlite-memory feature is disabled, this is the only path.
            #[cfg(not(feature = "sqlite-memory"))]
            {
                let proactive = crate::memory::proactive::ProactiveRecall::new(5, 0.6);
                let extra = proactive.recall(self, query, &combined).await?;
                combined.extend(extra);
                dedup_by_id(&mut combined);
                combined.truncate(self.max_recall);
            }

            // When sqlite-memory IS enabled but no store configured,
            // fall through to proactive recall (JSON path).
            #[cfg(feature = "sqlite-memory")]
            if self.sqlite_store.is_none() {
                let proactive = crate::memory::proactive::ProactiveRecall::new(5, 0.6);
                let extra = proactive.recall(self, query, &combined).await?;
                combined.extend(extra);
                dedup_by_id(&mut combined);
                combined.truncate(self.max_recall);
            }
        }

        Ok(combined)
    }

    /// Create a session summary memory entry from a completed session.
    ///
    /// This does NOT use LLM — it records key metadata from the session
    /// as a structured memory entry for future reference.
    pub async fn summarize_session(
        &self,
        session: &crate::state_store::Session,
    ) -> Result<Option<String>> {
        if session.user_messages.is_empty() {
            return Ok(None);
        }

        // Build a summary from the session metadata
        let mut summary_parts = Vec::new();

        // Include the first user message as context
        if let Some(first_msg) = session.user_messages.first() {
            summary_parts.push(format!("User: {}", first_msg.content));
        }

        // Include the last agent response
        if let Some(last_response) = session.agent_responses.last() {
            let truncated = if last_response.content.len() > 500 {
                format!("{}...", &last_response.content[..500])
            } else {
                last_response.content.clone()
            };
            summary_parts.push(format!("Agent: {truncated}"));
        }

        // Include metadata
        if let Some(ref seed_id) = session.active_seed_id {
            summary_parts.push(format!("Seed: {seed_id}"));
        }
        if let Some(ref persona_id) = session.active_persona_id {
            summary_parts.push(format!("Persona: {persona_id}"));
        }

        let content = summary_parts.join("\n");
        let entry = MemoryEntry {
            id: format!(
                "session-{}-{}",
                session.id.0,
                chrono::Utc::now().timestamp()
            ),
            memory_type: MemoryType::Session,
            tier: super::MemoryTier::Warm,
            content,
            content_hash: 0,
            source: "session_summary".to_string(),
            session_id: Some(session.id.0.clone()),
            tags: vec![],
            importance: 0.6,
            pinned: false,
            protection: super::ProtectionLevel::None,
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
        };

        let id = self.remember(entry).await?;
        Ok(Some(id))
    }

    /// Check if a memory entry with identical content already exists.
    ///
    /// Uses a fast hash comparison against the in-memory vector index.
    pub async fn is_duplicate(&self, content: &str) -> bool {
        let hash = content_hash(content);

        // Check semantic similarity via vector index first (fast)
        let query_vector = match self.embedding.embed(content).await {
            Ok(v) => v,
            Err(_) => return false,
        };
        let similar = {
            let index = self.vector_index.read();
            index
                .iter()
                .any(|(_, vector)| query_vector.cosine_similarity(vector) > 0.95)
        };
        if similar {
            return true;
        }

        // Then check exact content hash across all types
        for mt in MemoryType::all() {
            if let Ok(entries) = self.list(*mt, 1000).await {
                for entry in entries {
                    if content_hash(&entry.content) == hash {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Store a memory entry only if no duplicate content exists.
    ///
    /// Returns the entry ID if stored, or `None` if duplicate.
    pub async fn remember_unique(&self, entry: MemoryEntry) -> Result<Option<String>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.remember_unique(&entry).await;
        }
        if self.is_duplicate(&entry.content).await {
            tracing::debug!(id = %entry.id, "Skipping duplicate memory");
            return Ok(None);
        }
        let id = self.remember(entry).await?;
        Ok(Some(id))
    }
}

// ---------------------------------------------------------------------------
// HNSW-augmented operations
// ---------------------------------------------------------------------------

/// Result of a semantic search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHit {
    /// Memory entry.
    pub entry: MemoryEntry,
    /// Cosine distance (0.0 = identical).
    pub distance: f32,
    /// Cosine similarity (1.0 = identical).
    pub similarity: f32,
}

/// HNSW index manager for memory entries.
///
/// Maintains a mapping from u64 keys to String IDs, and the HNSW index
/// itself. Thread-safe via `RwLock`.
pub struct HnswMemoryIndex {
    /// The HNSW index.
    index: RwLock<HnswIndex>,
    /// Map: u64 key → String memory ID.
    key_to_id: RwLock<HashMap<u64, String>>,
    /// Map: String memory ID → u64 key.
    id_to_key: RwLock<HashMap<String, u64>>,
    /// Next key counter.
    next_key: AtomicU64,
    /// Base path for index persistence.
    persist_path: Option<PathBuf>,
}

impl std::fmt::Debug for HnswMemoryIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HnswMemoryIndex")
            .field("size", &self.len())
            .field("dimensions", &self.index.read().dimensions())
            .finish()
    }
}

impl HnswMemoryIndex {
    /// Create a new HNSW memory index.
    ///
    /// # Arguments
    /// * `dimensions` — Embedding vector dimensions.
    /// * `capacity` — Initial capacity hint.
    /// * `persist_path` — Optional directory for index file persistence.
    pub fn new(dimensions: usize, capacity: usize, persist_path: Option<PathBuf>) -> Result<Self> {
        let index = HnswIndex::new(dimensions, capacity)?;
        Ok(Self {
            index: RwLock::new(index),
            key_to_id: RwLock::new(HashMap::new()),
            id_to_key: RwLock::new(HashMap::new()),
            next_key: AtomicU64::new(1), // 0 is used by usearch as sentinel
            persist_path,
        })
    }

    /// Try to restore from disk, fall back to new index.
    pub fn restore_or_new(
        dimensions: usize,
        capacity: usize,
        persist_path: Option<PathBuf>,
    ) -> Result<Self> {
        if let Some(ref path) = persist_path {
            let index_path = path.join("memory.usearch");
            let mapping_path = path.join("key_map.json");

            if index_path.exists() && mapping_path.exists() {
                tracing::info!(path = %index_path.display(), "Restoring HNSW index from disk");

                if let Ok(index) = HnswIndex::load(&index_path) {
                    if let Ok(data) = std::fs::read_to_string(&mapping_path) {
                        if let Ok((k2i, i2k)) = serde_json::from_str::<(
                            HashMap<u64, String>,
                            HashMap<String, u64>,
                        )>(&data)
                        {
                            let max_key = k2i.keys().max().copied().unwrap_or(0);
                            return Ok(Self {
                                index: RwLock::new(index),
                                key_to_id: RwLock::new(k2i),
                                id_to_key: RwLock::new(i2k),
                                next_key: AtomicU64::new(max_key + 1),
                                persist_path,
                            });
                        }
                    }
                }

                tracing::warn!("Failed to restore HNSW index, creating new one");
            }
        }

        Self::new(dimensions, capacity, persist_path)
    }

    /// Get or create a u64 key for a String ID.
    fn get_or_create_key(&self, id: &str) -> u64 {
        // Fast path: check read lock
        {
            let i2k = self.id_to_key.read();
            if let Some(&key) = i2k.get(id) {
                return key;
            }
        }

        // Slow path: write lock
        let mut i2k = self.id_to_key.write();
        let mut k2i = self.key_to_id.write();

        // Double-check after acquiring write lock
        if let Some(&key) = i2k.get(id) {
            return key;
        }

        let key = self.next_key.fetch_add(1, Ordering::Relaxed);
        i2k.insert(id.to_string(), key);
        k2i.insert(key, id.to_string());
        key
    }

    /// Add an entry to the HNSW index.
    pub fn add_entry(&self, id: &str, vector: &[f32]) -> Result<()> {
        let key = self.get_or_create_key(id);
        let mut normalized = vector.to_vec();
        l2_normalize_f32(&mut normalized);
        self.index.write().add(key, &normalized)?;
        Ok(())
    }

    /// Remove an entry from the index.
    pub fn remove_entry(&self, id: &str) -> Result<()> {
        let key = {
            let i2k = self.id_to_key.read();
            i2k.get(id).copied()
        };
        if let Some(key) = key {
            self.index.write().remove(key)?;
            let mut k2i = self.key_to_id.write();
            let mut i2k = self.id_to_key.write();
            k2i.remove(&key);
            i2k.remove(id);
        }
        Ok(())
    }

    /// Search for k nearest neighbors.
    ///
    /// Returns (String ID, distance) pairs.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let mut normalized = query.to_vec();
        l2_normalize_f32(&mut normalized);

        let raw = self.index.read().search(&normalized, k)?;
        let k2i = self.key_to_id.read();

        let results = raw
            .into_iter()
            .filter_map(|(key, dist)| k2i.get(&key).map(|id| (id.clone(), dist)))
            .collect();

        Ok(results)
    }

    /// Number of entries in the index.
    pub fn len(&self) -> usize {
        self.index.read().len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.index.read().is_empty()
    }

    /// Save the index and key mappings to disk.
    pub fn persist(&self) -> Result<()> {
        if let Some(ref path) = self.persist_path {
            std::fs::create_dir_all(path)?;

            let index_path = path.join("memory.usearch");
            let mapping_path = path.join("key_map.json");

            // Save index
            self.index.read().save(&index_path)?;

            // Save key mappings
            let k2i = self.key_to_id.read();
            let i2k = self.id_to_key.read();
            let data = serde_json::to_string(&(k2i.clone(), &*i2k))?;
            std::fs::write(&mapping_path, data)?;

            tracing::debug!(path = %path.display(), entries = self.len(), "HNSW index persisted");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Semantic search on MemoryManager
// ---------------------------------------------------------------------------

impl MemoryManager {
    /// Semantic search using HNSW index.
    ///
    /// Unlike `search()` which uses brute-force cosine similarity over the
    /// in-memory HashMap, `semantic_search()` uses the HNSW approximate
    /// nearest neighbor index for sub-linear time complexity.
    ///
    /// This is the preferred search method when the HNSW index is available
    /// and populated with dense vectors.
    ///
    /// # Arguments
    /// * `query` — Search query text.
    /// * `memory_type` — Optional filter by memory type.
    /// * `limit` — Maximum results to return.
    /// * `hnsw_index` — The HNSW index to search against.
    ///
    /// # Returns
    /// A list of `SemanticHit` with entry and similarity score.
    pub async fn semantic_search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
        hnsw_index: &HnswMemoryIndex,
    ) -> Result<Vec<SemanticHit>> {
        // Skip if index is empty
        if hnsw_index.is_empty() {
            tracing::debug!("HNSW index empty, falling back to keyword search");
            return self
                .keyword_search(query, memory_type, limit)
                .await
                .map(|entries| {
                    entries
                        .into_iter()
                        .map(|entry| SemanticHit {
                            entry,
                            distance: 0.0,
                            similarity: 0.0,
                        })
                        .collect()
                });
        }

        // Generate embedding for query
        let query_vector = self.embedding.embed(query).await?;
        let query_f32 = match query_vector.to_f32_dense() {
            Some(v) => v,
            None => {
                tracing::debug!("Query embedding is sparse, falling back to keyword search");
                return self
                    .keyword_search(query, memory_type, limit)
                    .await
                    .map(|entries| {
                        entries
                            .into_iter()
                            .map(|entry| SemanticHit {
                                entry,
                                distance: 0.0,
                                similarity: 0.0,
                            })
                            .collect()
                    });
            }
        };

        // Search HNSW index
        let raw_hits = hnsw_index.search(&query_f32, limit * 2)?;

        // Determine which memory types to search
        let types: &[MemoryType] = match memory_type {
            Some(ref t) => std::slice::from_ref(t),
            None => MemoryType::all(),
        };

        // Load entries and build results
        let mut results = Vec::new();
        for (id, distance) in raw_hits {
            for mt in types {
                if let Ok(Some(mut entry)) = self
                    .state_store
                    .load_json::<MemoryEntry>(mt.category(), &id)
                    .await
                {
                    AutoProtector::record_access(&mut entry, "");

                    let similarity = 1.0 - distance;
                    results.push(SemanticHit {
                        entry,
                        distance,
                        similarity,
                    });
                    break;
                }
            }
            if results.len() >= limit {
                break;
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        tracing::debug!(
            query = %query,
            hits = results.len(),
            "Semantic search complete"
        );

        // Fall back if no results
        if results.is_empty() {
            return self
                .keyword_search(query, memory_type, limit)
                .await
                .map(|entries| {
                    entries
                        .into_iter()
                        .map(|entry| SemanticHit {
                            entry,
                            distance: 0.0,
                            similarity: 0.0,
                        })
                        .collect()
                });
        }

        Ok(results)
    }

    /// Rebuild the HNSW index from all stored memories.
    ///
    /// Call this at startup or after bulk operations.
    pub async fn rebuild_hnsw_index(&self, hnsw_index: &HnswMemoryIndex) -> Result<usize> {
        let mut count = 0;

        for mt in MemoryType::all() {
            if let Ok(names) = self.state_store.list_category(mt.category()).await {
                for name in names {
                    if let Ok(Some(entry)) = self
                        .state_store
                        .load_json::<MemoryEntry>(mt.category(), &name)
                        .await
                    {
                        let vector = self.embedding.embed(&entry.content).await?;
                        if let Some(f32_vec) = vector.to_f32_dense() {
                            if let Err(e) = hnsw_index.add_entry(&entry.id, &f32_vec) {
                                tracing::warn!(
                                    id = %entry.id,
                                    error = %e,
                                    "Failed to add entry to HNSW index"
                                );
                                continue;
                            }
                            count += 1;
                        }
                    }
                }
            }
        }

        tracing::info!(entries = count, "HNSW index rebuilt");
        Ok(count)
    }

    // ------------------------------------------------------------------
    // RFC-008: Tier-aware and new memory operations
    // ------------------------------------------------------------------

    /// List memories by tier (loads all types, filters by tier field).
    pub async fn list_by_tier(
        &self,
        tier: super::MemoryTier,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.list_by_tier(tier, limit);
        }

        let mut results = Vec::new();
        for mt in MemoryType::all() {
            if let Ok(entries) = self.list(*mt, limit).await {
                for entry in entries {
                    if entry.tier == tier {
                        results.push(entry);
                    }
                }
            }
            if results.len() >= limit {
                break;
            }
        }
        results.truncate(limit);
        Ok(results)
    }

    /// Get a memory entry by ID (searches all types).
    pub async fn get_by_id(&self, id: &str) -> Result<Option<MemoryEntry>> {
        for mt in MemoryType::all() {
            if let Ok(Some(entry)) = self.get(id, *mt).await {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Load a memory entry by reference string (ID or category/id).
    pub async fn load_by_reference(&self, reference: &str) -> Result<Option<MemoryEntry>> {
        // Try as direct ID first
        if let Ok(Some(entry)) = self.get_by_id(reference).await {
            return Ok(Some(entry));
        }
        // Try as category/name format
        if let Some((cat, name)) = reference.split_once('/') {
            if let Ok(Some(entry)) = self.state_store.load_json::<MemoryEntry>(cat, name).await {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Select memories by manifest (keyword matching against content).
    ///
    /// Used by proactive recall Step 2 for cross-domain keyword matching.
    pub async fn select_by_manifest(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        self.keyword_search(query, None, limit).await
    }

    /// Build the Hot tier context for agent prompt injection.
    ///
    /// Returns a formatted string of all Hot tier memories, truncated to
    /// fit within the configured token budget (4 chars ≈ 1 token).
    pub async fn build_hot_context(&self, token_budget: usize) -> Result<String> {
        let hot_entries = self.list_by_tier(super::MemoryTier::Hot, 50).await?;

        let mut context_parts = Vec::new();
        let mut char_budget = token_budget * 4;

        for entry in &hot_entries {
            let line = format!("- [{}] {}", entry.memory_type.label(), entry.content);
            if line.len() > char_budget {
                break;
            }
            char_budget -= line.len();
            context_parts.push(line);
        }

        if context_parts.is_empty() {
            Ok(String::new())
        } else {
            Ok(format!("## Active Context\n\n{}", context_parts.join("\n")))
        }
    }

    /// Build full context: hot context + proactive recall blended into system prompt.
    pub async fn build_full_context(
        &self,
        _query: &str,
        system_prompt: &str,
        token_budget: usize,
    ) -> Result<String> {
        let hot_ctx = self.build_hot_context(token_budget).await?;

        if hot_ctx.is_empty() {
            return Ok(system_prompt.to_string());
        }

        Ok(format!("{system_prompt}\n\n{hot_ctx}"))
    }

    /// Shift a memory entry between tiers.
    pub async fn shift_tier(
        &self,
        id: &str,
        from: super::MemoryTier,
        to: super::MemoryTier,
    ) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await {
            if entry.tier == from {
                entry.tier = to;
                self.remember(entry).await?;
            }
        }
        Ok(())
    }

    /// Pin a memory (set permanent protection).
    pub async fn pin(&self, id: &str) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await {
            entry.pinned = true;
            entry.protection = super::ProtectionLevel::Permanent;
            self.remember(entry).await?;
        }
        Ok(())
    }

    /// Unpin a memory (revert to auto-computed protection).
    pub async fn unpin(&self, id: &str) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await {
            entry.pinned = false;
            // Recompute protection
            let protector = oxios_memory::memory::auto_protect::AutoProtector::default_protector();
            entry.protection = protector.compute_protection(&entry);
            self.remember(entry).await?;
        }
        Ok(())
    }

    /// Set importance for a memory entry.
    pub async fn set_importance(&self, id: &str, importance: f32) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await {
            entry.importance = importance.clamp(0.0, 1.0);
            self.remember(entry).await?;
        }
        Ok(())
    }

    /// Recompute decay scores for all entries.
    ///
    /// Returns the number of entries updated.
    pub async fn recompute_all_decay(&self, multiplier: f32) -> Result<usize> {
        let engine = oxios_memory::memory::decay::DecayEngine::new(multiplier);
        let now = chrono::Utc::now();
        let mut count = 0;

        for mt in MemoryType::all() {
            if let Ok(entries) = self.list(*mt, 1_000_000).await {
                for mut entry in entries {
                    let new_decay = engine.compute_decay(&entry, now);
                    if (entry.decay_score - new_decay).abs() > 0.001 {
                        entry.decay_score = new_decay;
                        self.remember(entry).await?;
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Immediate Hot overflow handling.
    ///
    /// Called after remember() to immediately demote entries when Hot
    /// exceeds its budget.
    pub async fn immediate_hot_overflow(&self, hot_max: usize) -> Result<usize> {
        let hot_entries = self
            .list_by_tier(super::MemoryTier::Hot, hot_max * 2)
            .await?;
        if hot_entries.len() <= hot_max {
            return Ok(0);
        }

        let overflow = hot_entries.len() - hot_max;
        let mut candidates: Vec<MemoryEntry> = hot_entries
            .into_iter()
            .filter(|e| e.protection < super::ProtectionLevel::High && !e.pinned)
            .collect();

        candidates.sort_by(|a, b| {
            a.protection.cmp(&b.protection).then(
                a.decay_score
                    .partial_cmp(&b.decay_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        let mut demoted = 0;
        for entry in candidates.into_iter().take(overflow) {
            self.shift_tier(&entry.id, super::MemoryTier::Hot, super::MemoryTier::Warm)
                .await?;
            demoted += 1;
        }

        Ok(demoted)
    }
}
