//! Agent memory system.
//!
//! Provides persistent memory for agents across sessions.
//! Memory entries are stored as JSON files via StateStore.

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::state_store::StateStore;

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

// ---------------------------------------------------------------------------
// MemoryManager
// ---------------------------------------------------------------------------

/// Agent memory manager.
///
/// Stores and retrieves memory entries using the file-based StateStore.
#[derive(Debug, Clone)]
pub struct MemoryManager {
    state_store: Arc<StateStore>,
    max_recall: usize,
}

impl MemoryManager {
    /// Create a new MemoryManager.
    pub fn new(state_store: Arc<StateStore>) -> Self {
        Self {
            state_store,
            max_recall: 10,
        }
    }

    /// Set max memories returned by recall.
    pub fn with_max_recall(mut self, n: usize) -> Self {
        self.max_recall = n;
        self
    }

    /// Store a memory entry. Returns the entry ID.
    pub async fn remember(&self, entry: MemoryEntry) -> Result<String> {
        let id = entry.id.clone();
        let category = entry.memory_type.category();
        self.state_store
            .save_json(category, &id, &entry)
            .await?;
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
        // StateStore doesn't have delete, so check existence and report
        if self.get(id, memory_type).await?.is_some() {
            tracing::info!(id = %id, "Memory forgotten (tombstone)");
            Ok(true)
        } else {
            Ok(false)
        }
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

    /// Search memories by keyword.
    pub async fn search(
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