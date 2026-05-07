//! Agent memory system.
//!
//! Provides persistent memory for agents across sessions.
//! Memory entries are stored as JSON files via StateStore.
//! Supports embedding-based vector search using TF-IDF + cosine similarity.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::state_store::StateStore;

// ---------------------------------------------------------------------------
// Content hashing
// ---------------------------------------------------------------------------

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Compute a stable hash of content for deduplication.
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// TextVector (TF-IDF vector for semantic similarity)
// ---------------------------------------------------------------------------

/// Simple TF-IDF vector for text similarity.
///
/// Tokenizes text into terms, computes normalized term frequency,
/// and supports cosine similarity comparison. No external embedding
/// model needed — works for any language including Korean.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextVector {
    /// Term frequencies (normalized).
    tf: HashMap<String, f64>,
}

impl TextVector {
    /// Create a text vector from input text.
    pub fn from_text(text: &str) -> Self {
        let mut tf: HashMap<String, f64> = HashMap::new();
        let terms = Self::tokenize(text);
        let total = terms.len() as f64;

        for term in terms {
            *tf.entry(term).or_insert(0.0) += 1.0;
        }

        // Normalize by total term count
        if total > 0.0 {
            for v in tf.values_mut() {
                *v /= total;
            }
        }

        Self { tf }
    }

    /// Tokenize text into terms (language-agnostic).
    /// Splits on whitespace and punctuation, lowercases.
    /// Preserves Korean Hangul syllables (U+AC00–U+D7A3) within tokens.
    pub fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && !('\u{AC00}'..='\u{D7A3}').contains(&c))
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(&self, other: &TextVector) -> f64 {
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for (term, &a) in &self.tf {
            norm_a += a * a;
            if let Some(&b) = other.tf.get(term) {
                dot += a * b;
            }
        }
        for &b in other.tf.values() {
            norm_b += b * b;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Memory entry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Conversation compaction summary (auto-generated).
    Conversation,
    /// Session-end summary (auto-generated).
    Session,
    /// Agent-stored fact.
    Fact,
    /// Episode memory (event/experience).
    Episode,
    /// Static knowledge (user/program-provided).
    Knowledge,
}

impl MemoryType {
    /// Category name used in StateStore.
    pub fn category(&self) -> &'static str {
        match self {
            MemoryType::Conversation => "memory/conversations",
            MemoryType::Session => "memory/sessions",
            MemoryType::Fact => "memory/facts",
            MemoryType::Episode => "memory/episodes",
            MemoryType::Knowledge => "memory/knowledge",
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            MemoryType::Conversation => "conversation",
            MemoryType::Session => "session",
            MemoryType::Fact => "fact",
            MemoryType::Episode => "episode",
            MemoryType::Knowledge => "knowledge",
        }
    }
}

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique ID.
    pub id: String,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Content (Markdown).
    pub content: String,
    /// Creator (agent name, "compaction", "system", etc.).
    pub source: String,
    /// Related session ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Tags for search.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Importance (0.0 – 1.0).
    #[serde(default = "default_importance")]
    pub importance: f32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last access timestamp.
    pub accessed_at: DateTime<Utc>,
    /// Access count.
    #[serde(default)]
    pub access_count: u32,
}

fn default_importance() -> f32 {
    0.5
}

/// Budget for memory curation — limits per type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBudget {
    /// Maximum entries per memory type.
    pub max_per_type: usize,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self { max_per_type: 100 }
    }
}

/// A single candidate for removal during curation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationCandidate {
    /// Memory entry ID.
    pub id: String,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Effective importance score (lower = more likely removed).
    pub effective_importance: f32,
}

/// Report from a curation run.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CurationReport {
    /// Total entries before curation.
    pub total_before: usize,
    /// Total entries after curation.
    pub total_after: usize,
    /// Number of entries actually removed.
    pub removed: usize,
    /// Candidates identified for removal.
    pub candidates_for_removal: Vec<CurationCandidate>,
}

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
    /// Map of entry ID to text vector.
    entries: HashMap<String, TextVector>,
}

// ---------------------------------------------------------------------------
// MemoryManager
// ---------------------------------------------------------------------------

/// Agent memory manager.
///
/// Stores and retrieves memory entries using the file-based StateStore.
/// Supports embedding-based vector search via an in-memory TF-IDF index
/// that is rebuilt on startup.
pub struct MemoryManager {
    state_store: Arc<StateStore>,
    max_recall: usize,
    /// Vector index for semantic search (id → TextVector).
    vector_index: RwLock<HashMap<String, TextVector>>,
}

impl std::fmt::Debug for MemoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryManager")
            .field("max_recall", &self.max_recall)
            .field("index_size", &self.vector_index.read().len())
            .finish()
    }
}

impl MemoryManager {
    /// Create a new MemoryManager.
    pub fn new(state_store: Arc<StateStore>) -> Self {
        Self {
            state_store,
            max_recall: 10,
            vector_index: RwLock::new(HashMap::new()),
        }
    }

    /// Set max memories returned by recall.
    pub fn with_max_recall(mut self, n: usize) -> Self {
        self.max_recall = n;
        self
    }

    /// Apply MemoryConfig settings.
    pub fn with_config(mut self, config: &crate::config::MemoryConfig) -> Self {
        self.max_recall = config.max_recall;
        self
    }

    /// Returns the number of entries in the vector index.
    pub fn vector_index_size(&self) -> usize {
        self.vector_index.read().len()
    }

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
        let mut entries_to_index: Vec<(String, TextVector)> = Vec::new();

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
                        let vector = TextVector::from_text(&entry.content);
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
        let vector = TextVector::from_text(&entry.content);
        let category = entry.memory_type.category();
        self.state_store
            .save_json(category, &id, &entry)
            .await?;

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
        let query_vector = TextVector::from_text(query);

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
        let query_vector = TextVector::from_text(content);
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

    /// Compute effective importance of a memory entry.
    ///
    /// Effective importance = base_importance * (1 + log(1 + access_count))
    /// Memories accessed frequently get a boost.
    pub fn effective_importance(entry: &MemoryEntry) -> f32 {
        let access_boost = (1.0_f32 + entry.access_count as f32).ln();
        entry.importance * (1.0 + access_boost)
    }

    /// Curate memories: identify candidates for removal based on budget.
    ///
    /// Returns a report of how many entries would be pruned per type.
    pub async fn curate(&self, budget: &MemoryBudget) -> Result<CurationReport> {
        let mut report = CurationReport::default();

        for mt in &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ] {
            let entries = self.list(*mt, budget.max_per_type * 2).await?;
            if entries.len() <= budget.max_per_type {
                continue;
            }

            // Sort by effective importance ascending (least important first)
            let total_count = entries.len();
            let mut scored: Vec<_> = entries
                .into_iter()
                .map(|e| (e.id.clone(), e.memory_type, Self::effective_importance(&e)))
                .collect();
            scored.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

            let to_remove = scored.len() - budget.max_per_type;
            for (id, memory_type, score) in scored.into_iter().take(to_remove) {
                report.candidates_for_removal.push(CurationCandidate {
                    id,
                    memory_type,
                    effective_importance: score,
                });
            }
            report.total_before += total_count;
        }

        // Actually remove candidates
        for candidate in &report.candidates_for_removal {
            if self.forget(&candidate.id, candidate.memory_type).await.is_ok() {
                report.removed += 1;
            }
        }

        report.total_after = report.total_before - report.removed;
        Ok(report)
    }

    /// Spawn a background curation task.
    ///
    /// Returns immediately; curation runs asynchronously.
    pub fn spawn_curation_task(self: &Arc<Self>, budget: MemoryBudget) {
        let mgr = Arc::clone(self);
        tokio::spawn(async move {
            match mgr.curate(&budget).await {
                Ok(report) => {
                    if report.removed > 0 {
                        tracing::info!(
                            removed = report.removed,
                            candidates = report.candidates_for_removal.len(),
                            "Memory curation complete"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Memory curation failed");
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract search keywords from a query string.
///
/// Simple implementation: split on whitespace, lowercase, filter stop words.
fn extract_keywords(query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "shall", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "before", "after", "above", "below", "between", "out", "off", "over",
        "under", "again", "further", "then", "once", "and", "but", "or", "nor",
        "not", "so", "yet", "both", "either", "neither", "each", "every",
        "all", "any", "few", "more", "most", "other", "some", "such", "no",
        "only", "own", "same", "than", "too", "very", "just", "because",
        "if", "when", "where", "how", "what", "which", "who", "whom", "this",
        "that", "these", "those", "i", "me", "my", "we", "our", "you", "your",
        "he", "him", "his", "she", "her", "it", "its", "they", "them", "their",
    ];

    query
        .split_whitespace()
        .map(|w| {
            // Strip trailing punctuation
            let w = w.trim_end_matches(|c: char| c.is_ascii_punctuation());
            w.to_lowercase()
        })
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Remove duplicate entries by ID, keeping the first occurrence.
fn dedup_by_id(entries: &mut Vec<MemoryEntry>) {
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.id.clone()));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_category() {
        assert_eq!(MemoryType::Conversation.category(), "memory/conversations");
        assert_eq!(MemoryType::Fact.category(), "memory/facts");
        assert_eq!(MemoryType::Knowledge.category(), "memory/knowledge");
    }

    #[test]
    fn test_extract_keywords() {
        let kw = extract_keywords("How do I implement a Rust agent system?");
        assert!(kw.contains(&"implement".to_string()));
        assert!(kw.contains(&"rust".to_string()));
        assert!(kw.contains(&"agent".to_string()));
        assert!(kw.contains(&"system".to_string()));
        // stop words filtered
        assert!(!kw.contains(&"how".to_string()));
        assert!(!kw.contains(&"do".to_string()));
    }

    #[test]
    fn test_dedup_by_id() {
        let mut entries = vec![
            make_entry("a", MemoryType::Fact),
            make_entry("b", MemoryType::Fact),
            make_entry("a", MemoryType::Episode), // duplicate id
        ];
        dedup_by_id(&mut entries);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_blend_into_prompt_empty() {
        let mgr = MemoryManager::new(Arc::new(StateStore::new(std::env::temp_dir().join("test")).unwrap()));
        let result = mgr.blend_into_prompt(&[], "You are an agent.");
        assert_eq!(result, "You are an agent.");
    }

    #[test]
    fn test_blend_into_prompt_with_memories() {
        let mgr = MemoryManager::new(Arc::new(StateStore::new(std::env::temp_dir().join("test")).unwrap()));
        let memories = vec![make_entry("test", MemoryType::Fact)];
        let result = mgr.blend_into_prompt(&memories, "You are an agent.");
        assert!(result.contains("## Relevant Memory"));
        assert!(result.contains("[fact]"));
    }

    // ---- Vector search tests ----

    #[test]
    fn test_text_vector_cosine_similarity() {
        let v1 = TextVector::from_text("fix the null pointer error in main.rs");
        let v2 = TextVector::from_text("null pointer error found in rust code");
        let v3 = TextVector::from_text("update the documentation for deployment");

        // Similar texts should have high similarity
        assert!(
            v1.cosine_similarity(&v2) > 0.3,
            "Similar texts should have > 0.3 similarity"
        );

        // Different texts should have low similarity
        assert!(
            v1.cosine_similarity(&v3) < 0.2,
            "Different texts should have < 0.2 similarity"
        );
    }

    #[test]
    fn test_text_vector_korean() {
        let v1 = TextVector::from_text("main.rs 파일의 null pointer 에러 수정");
        let v2 = TextVector::from_text("null pointer 오류를 수정했습니다");
        let v3 = TextVector::from_text("문서 업데이트 배포 가이드");

        assert!(
            v1.cosine_similarity(&v2) > 0.1,
            "Korean+code similarity"
        );
        assert!(
            v1.cosine_similarity(&v3) < 0.1,
            "Korean different topics"
        );
    }

    #[test]
    fn test_text_vector_empty() {
        let v1 = TextVector::from_text("");
        let v2 = TextVector::from_text("hello");
        assert_eq!(v1.cosine_similarity(&v2), 0.0);
    }

    #[test]
    fn test_text_vector_identical() {
        let v1 = TextVector::from_text("rust programming language");
        let v2 = TextVector::from_text("rust programming language");
        let sim = v1.cosine_similarity(&v2);
        assert!(
            (sim - 1.0).abs() < 1e-9,
            "Identical texts should have similarity ~1.0, got {}",
            sim
        );
    }

    #[test]
    fn test_tokenize_korean() {
        let terms = TextVector::tokenize("main.rs 파일의 버그를 수정");
        // Should contain at least some meaningful tokens
        assert!(!terms.is_empty(), "Korean text should produce tokens");
    }

    #[tokio::test]
    async fn test_vector_search_over_keyword_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store.clone());

        // Store some memories
        let entry1 = MemoryEntry {
            id: "vec-test-1".to_string(),
            memory_type: MemoryType::Fact,
            content: "Rust is a systems programming language focused on safety".to_string(),
            source: "test".to_string(),
            session_id: None,
            tags: vec![],
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        let entry2 = MemoryEntry {
            id: "vec-test-2".to_string(),
            memory_type: MemoryType::Fact,
            content: "Python is great for machine learning and data science".to_string(),
            source: "test".to_string(),
            session_id: None,
            tags: vec![],
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };

        mgr.remember(entry1).await.unwrap();
        mgr.remember(entry2).await.unwrap();

        // Vector search should find the Rust entry for a Rust-related query
        let results = mgr.search("systems programming with rust", None, 5).await.unwrap();
        assert!(!results.is_empty(), "Vector search should find results");
        assert_eq!(results[0].id, "vec-test-1", "Should find the Rust entry first");
    }

    #[tokio::test]
    async fn test_rebuild_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store.clone());

        // Store a memory directly via state_store (bypassing remember to test rebuild)
        let entry = MemoryEntry {
            id: "rebuild-test-1".to_string(),
            memory_type: MemoryType::Fact,
            content: "memory for rebuild test".to_string(),
            source: "test".to_string(),
            session_id: None,
            tags: vec![],
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        store.save_json("memory/facts", "rebuild-test-1", &entry).await.unwrap();

        // Index should be empty before rebuild
        assert_eq!(mgr.vector_index.read().len(), 0);

        // Rebuild
        mgr.rebuild_index().await.unwrap();

        // Index should now contain the entry
        assert_eq!(mgr.vector_index.read().len(), 1);
        assert!(mgr.vector_index.read().contains_key("rebuild-test-1"));
    }

    fn make_entry(id: &str, ty: MemoryType) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            memory_type: ty,
            content: format!("Test content for {}", id),
            source: "test".to_string(),
            session_id: None,
            tags: vec![],
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        }
    }
}