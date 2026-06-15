//! Advanced memory operations — semantic search, HNSW rebuild, tier management.

use anyhow::Result;

use crate::memory::auto_protect::AutoProtector;
use crate::memory::hnsw_memory_index::{HnswMemoryIndex, SemanticHit};
use crate::memory::storage::MemoryStorageExt;
use crate::memory::types::{MemoryEntry, MemoryTier, MemoryType};

use super::MemoryManager;

impl MemoryManager {
    /// Semantic search using HNSW index.
    ///
    /// Unlike `search()` which uses brute-force cosine similarity over the
    /// in-memory HashMap, `semantic_search()` uses the HNSW approximate
    /// nearest neighbor index for sub-linear time complexity.
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
                    .storage
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
            if let Ok(names) = self.storage.list_category(mt.category()).await {
                for name in names {
                    if let Ok(Some(entry)) = self
                        .storage
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
    pub async fn list_by_tier(&self, tier: MemoryTier, limit: usize) -> Result<Vec<MemoryEntry>> {
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
        if let Some((cat, name)) = reference.split_once('/')
            && let Ok(Some(entry)) = self.storage.load_json::<MemoryEntry>(cat, name).await
        {
            return Ok(Some(entry));
        }
        Ok(None)
    }

    /// Select memories by manifest (keyword matching against content).
    pub async fn select_by_manifest(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        self.keyword_search(query, None, limit).await
    }

    /// Build the Hot tier context for agent prompt injection.
    pub async fn build_hot_context(&self, token_budget: usize) -> Result<String> {
        let hot_entries = self.list_by_tier(MemoryTier::Hot, 50).await?;

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
    pub async fn shift_tier(&self, id: &str, from: MemoryTier, to: MemoryTier) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await
            && entry.tier == from
        {
            entry.tier = to;
            self.remember(entry).await?;
        }
        Ok(())
    }

    /// Pin a memory (set permanent protection).
    pub async fn pin(&self, id: &str) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await {
            entry.pinned = true;
            entry.protection = crate::memory::types::ProtectionLevel::Permanent;
            self.remember(entry).await?;
        }
        Ok(())
    }

    /// Unpin a memory (revert to auto-computed protection).
    pub async fn unpin(&self, id: &str) -> Result<()> {
        if let Ok(Some(mut entry)) = self.get_by_id(id).await {
            entry.pinned = false;
            // Recompute protection
            let protector = crate::memory::auto_protect::AutoProtector::default_protector();
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
        let engine = crate::memory::decay::DecayEngine::new(multiplier);
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
    pub async fn immediate_hot_overflow(&self, hot_max: usize) -> Result<usize> {
        let hot_entries = self.list_by_tier(MemoryTier::Hot, hot_max * 2).await?;
        if hot_entries.len() <= hot_max {
            return Ok(0);
        }

        let overflow = hot_entries.len() - hot_max;
        let mut candidates: Vec<MemoryEntry> = hot_entries
            .into_iter()
            .filter(|e| e.protection < crate::memory::types::ProtectionLevel::High && !e.pinned)
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
            self.shift_tier(&entry.id, MemoryTier::Hot, MemoryTier::Warm)
                .await?;
            demoted += 1;
        }

        Ok(demoted)
    }
}
