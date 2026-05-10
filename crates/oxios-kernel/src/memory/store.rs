//! Memory store operations: save/load, index management, search.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::embedding::EmbeddingVector;

use super::{MemoryEntry, MemoryManager, MemoryType, content_hash, dedup_by_id, extract_keywords};

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
        for mt in [
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ] {
            if let Ok(entries) = self.list(mt, usize::MAX).await {
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

        for mt in &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ] {
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

        tracing::info!(entries = self.vector_index.read().len(), "Memory vector index rebuilt");
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

        tracing::debug!(entries = snapshot.entry_count, "Vector index snapshot saved");
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
    /// Also computes and stores the entry's text vector in the in-memory
    /// index for future semantic search.
    pub async fn remember(&self, entry: MemoryEntry) -> Result<String> {
        let id = entry.id.clone();
        let vector = self.embedding.embed(&entry.content).await?;
        let category = entry.memory_type.category();
        self.state_store
            .save_json(category, &id, &entry)
            .await?;

        self.git_commit(
            &format!("{}/{}.json", category, id),
            &format!("memory: store {}", id),
        );

        // Update vector index
        {
            let mut index = self.vector_index.write();
            index.insert(id.clone(), vector);
        }

        tracing::debug!(id = %id, ty = entry.memory_type.label(), "Memory stored");
        Ok(id)
    }

    /// Retrieve a single memory by ID.
    pub async fn get(&self, id: &str, memory_type: MemoryType) -> Result<Option<MemoryEntry>> {
        self.state_store
            .load_json(memory_type.category(), id)
            .await
    }

    /// Delete a memory entry.
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> Result<bool> {
        self.state_store.delete_file(memory_type.category(), id).await
    }

    /// List memories of a given type, most recent first.
    pub async fn list(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<MemoryEntry>> {
        let category = memory_type.category();
        let names = self.state_store.list_category(category).await?;
        let mut entries = Vec::new();
        for name in names.into_iter().take(limit * 2) {
            if let Ok(Some(entry)) = self.state_store.load_json::<MemoryEntry>(category, &name).await {
                entries.push(entry);
            }
        }
        // Sort by created_at descending (most recent first)
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
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
        let all_types: &[MemoryType] = &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ];
        let types: &[MemoryType] = match memory_type {
            Some(ref t) => std::slice::from_ref(t),
            None => all_types,
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
                    entry.access_count += 1;
                    entry.accessed_at = chrono::Utc::now();
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
            None => vec![
                MemoryType::Conversation,
                MemoryType::Fact,
                MemoryType::Episode,
                MemoryType::Knowledge,
            ],
        };

        let mut results = Vec::new();
        for ty in &types {
            let entries = self.list(*ty, limit * 2).await?;
            for entry in entries {
                let matches = keywords.iter().any(|k| {
                    let k_lower = k.to_lowercase();
                    entry.content.to_lowercase().contains(&k_lower)
                        || entry.tags.iter().any(|t| t.to_lowercase().contains(&k_lower))
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
        let limit = self.max_recall;

        // 1. Recent conversation summaries (always include)
        let recent = self.list(MemoryType::Conversation, 3).await.unwrap_or_default();

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
        if memories.is_empty() {
            return system_prompt.to_string();
        }

        let memory_block = memories
            .iter()
            .map(|m| format!("- [{}] {}", m.memory_type.label(), m.content))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "{system_prompt}\n\n## Relevant Memory\n\n{memory_block}"
        )
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
            summary_parts.push(format!("Agent: {}", truncated));
        }

        // Include metadata
        if let Some(ref seed_id) = session.active_seed_id {
            summary_parts.push(format!("Seed: {}", seed_id));
        }
        if let Some(ref persona_id) = session.active_persona_id {
            summary_parts.push(format!("Persona: {}", persona_id));
        }

        let content = summary_parts.join("\n");
        let entry = MemoryEntry {
            id: format!("session-{}-{}", session.id.0, chrono::Utc::now().timestamp()),
            memory_type: MemoryType::Session,
            content,
            source: "session_summary".to_string(),
            session_id: Some(session.id.0.clone()),
            tags: vec![],
            importance: 0.6,
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            access_count: 0,
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
            index.iter().any(|(_, vector)| query_vector.cosine_similarity(vector) > 0.95)
        };
        if similar {
            return true;
        }

        // Then check exact content hash across all types
        for mt in &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ] {
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
        if self.is_duplicate(&entry.content).await {
            tracing::debug!(id = %entry.id, "Skipping duplicate memory");
            return Ok(None);
        }
        let id = self.remember(entry).await?;
        Ok(Some(id))
    }
}
