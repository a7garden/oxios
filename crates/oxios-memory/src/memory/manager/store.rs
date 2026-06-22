//! Memory store operations: save/load, index management, search.
//!
//! Integrates HNSW index (usearch) for fast approximate nearest neighbor search
//! alongside the abstract storage backend for persistence.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::memory::auto_protect::AutoProtector;
use crate::memory::embedding::EmbeddingVector;
use crate::memory::storage::MemoryStorageExt;
#[cfg(feature = "sqlite-memory")]
use crate::memory::types::MemoryTier;
use crate::memory::types::{MemoryEntry, MemoryType, content_hash, dedup_by_id, extract_keywords};

use super::MemoryManager;

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
    pub async fn rebuild_index(&self) -> anyhow::Result<()> {
        // Collect all entries outside the lock
        let mut entries_to_index: Vec<(String, EmbeddingVector)> = Vec::new();

        for mt in MemoryType::all() {
            if let Ok(names) = self.storage.list_category(mt.category()).await {
                for name in names {
                    if let Ok(Some(entry)) = self
                        .storage
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
    pub async fn save_index_snapshot(&self) -> anyhow::Result<()> {
        let snapshot = {
            let index = self.vector_index.read();
            VectorIndexSnapshot {
                created_at: chrono::Utc::now(),
                entry_count: index.len(),
                entries: index.clone(),
            }
        };

        self.storage
            .save_json("memory", "vector_index_snapshot", &snapshot)
            .await?;

        self.git_commit("memory/vector_index_snapshot.json", "memory: snapshot save")
            .await;

        tracing::debug!(
            entries = snapshot.entry_count,
            "Vector index snapshot saved"
        );
        Ok(())
    }

    /// Load a previously saved vector index snapshot from disk.
    pub async fn load_index_snapshot(&self) -> anyhow::Result<usize> {
        let snapshot: Option<VectorIndexSnapshot> = self
            .storage
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
    pub async fn remember(&self, entry: MemoryEntry) -> anyhow::Result<String> {
        // ── SQLite fast path (RFC-012) ──
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.remember(&entry).await;
        }

        // ── Legacy JSON path ──
        let id = entry.id.clone();
        let vector = self.embedding.embed(&entry.content).await?;
        let category = entry.memory_type.category();
        self.storage.save_json(category, &id, &entry).await?;

        self.git_commit(
            &format!("{category}/{id}.json"),
            &format!("memory: store {id}"),
        )
        .await;

        // Update vector index
        {
            let mut index = self.vector_index.write();
            index.insert(id.clone(), vector.clone());
        }

        // Update HNSW index if attached
        if let Some(f32_vec) = vector.to_f32_dense() {
            let hnsw = self.hnsw_index.read();
            if let Some(ref hnsw) = *hnsw
                && let Err(e) = hnsw.add_entry(&id, &f32_vec)
            {
                tracing::warn!(id = %id, error = %e, "Failed to update HNSW index on remember");
            }
        }

        tracing::debug!(id = %id, ty = entry.memory_type.label(), "Memory stored");
        Ok(id)
    }

    /// Retrieve a single memory by ID.
    ///
    /// This is a pure read — it does NOT mutate access counters. Bumping
    /// `access_count`/`accessed_at`/`seen_in_sessions` here polluted entries
    /// returned to callers (notably `get_by_id`, which scans all 9 types) with
    /// access-tracking side-effects that were never persisted anyway. Access
    /// recording belongs to explicit recall/touch paths, not to reads.
    pub async fn get(
        &self,
        id: &str,
        memory_type: MemoryType,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.get(id, memory_type);
        }
        self.storage.load_json(memory_type.category(), id).await
    }

    /// Delete a memory entry.
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> anyhow::Result<bool> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.forget(id, memory_type);
        }
        let result = self.storage.delete_file(memory_type.category(), id).await?;

        // Remove from HNSW index if attached
        {
            let hnsw = self.hnsw_index.read();
            if let Some(ref hnsw) = *hnsw
                && let Err(e) = hnsw.remove_entry(id)
            {
                tracing::warn!(id = %id, error = %e, "Failed to remove from HNSW index on forget");
            }
        }

        Ok(result)
    }

    /// List memories of a given type, most recent first.
    pub async fn list(
        &self,
        memory_type: MemoryType,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.list(memory_type, limit);
        }
        let category = memory_type.category();
        let names = self.storage.list_category(category).await?;
        let mut entries = Vec::new();
        for name in names.into_iter().take(limit.saturating_mul(2)) {
            if let Ok(Some(entry)) = self.storage.load_json::<MemoryEntry>(category, &name).await {
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
    ) -> anyhow::Result<Vec<MemoryEntry>> {
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

        // Load entries from storage (no lock held)
        let mut results = Vec::new();
        for (id, score) in scored {
            for mt in types {
                if let Ok(Some(mut entry)) = self
                    .storage
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
    pub(crate) async fn keyword_search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
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
    pub async fn recall(&self, query: &str) -> anyhow::Result<Vec<MemoryEntry>> {
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
    #[cfg(feature = "sqlite-memory")]
    pub async fn recall_with_rerank(&self, query: &str) -> anyhow::Result<Vec<MemoryEntry>> {
        if let Some(ref sqlite) = self.sqlite_store {
            return sqlite.recall_with_rerank(query, self.max_recall).await;
        }
        // Fallback to standard recall
        self.recall(query).await
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
    pub async fn remember_unique(&self, entry: MemoryEntry) -> anyhow::Result<Option<String>> {
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

    /// Recall with proactive enhancement.
    ///
    /// Extends the standard `recall()` with proactive memory injection
    /// based on `RecallTiming` triggers.
    pub async fn recall_with_proactive(
        &self,
        query: &str,
        recall_timing: &mut Option<crate::memory::proactive::RecallTiming>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // Step 1: Standard recall
        let mut combined = self.recall(query).await?;

        // Step 2: Proactive enhancement based on timing triggers
        let should_recall = recall_timing
            .as_mut()
            .map(|t| t.should_recall(query))
            .unwrap_or(true);

        if should_recall && combined.len() < self.max_recall {
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

            #[cfg(not(feature = "sqlite-memory"))]
            {
                let proactive = crate::memory::proactive::ProactiveRecall::new(5, 0.6);
                let extra = proactive.recall(self, query, &combined).await?;
                combined.extend(extra);
                dedup_by_id(&mut combined);
                combined.truncate(self.max_recall);
            }

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
}
