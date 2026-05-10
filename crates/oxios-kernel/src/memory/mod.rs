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

use crate::embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
use crate::git_layer::GitLayer;
use crate::state_store::StateStore;

// Re-export budget types so external `use crate::memory::X` paths still work.
pub use budget::{CurationCandidate, CurationReport, MemoryBudget};

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

    /// Returns a reference to the term-frequency map.
    pub fn tf_map(&self) -> &HashMap<String, f64> {
        &self.tf
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
    /// Vector index for semantic search (id → EmbeddingVector).
    vector_index: RwLock<HashMap<String, EmbeddingVector>>,
    /// Embedding provider for generating vectors.
    embedding: Arc<dyn EmbeddingProvider>,
    /// Optional git layer for version-controlled memory.
    git_layer: Option<Arc<GitLayer>>,
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
            embedding: Arc::new(TfIdfEmbeddingProvider),
            git_layer: None,
        }
    }

    /// Attach a git layer for version-controlled saves.
    pub fn set_git_layer(&mut self, gl: Arc<GitLayer>) {
        self.git_layer = Some(gl);
    }

    /// Commit a file to git if git_layer is available.
    fn git_commit(&self, rel_path: &str, message: &str) {
        if let Some(ref gl) = self.git_layer {
            if gl.is_enabled() {
                let _ = gl.commit_file(rel_path, message);
            }
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
pub(crate) fn extract_keywords(query: &str) -> Vec<String> {
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
pub(crate) fn dedup_by_id(entries: &mut Vec<MemoryEntry>) {
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.id.clone()));
}

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

mod budget;
mod store;

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
