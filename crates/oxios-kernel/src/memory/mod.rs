//! Agent memory system.
//!
//! Provides persistent memory for agents across sessions.
//! Memory entries are stored as JSON files via StateStore.
//! Supports embedding-based vector search using TF-IDF + cosine similarity.

use std::collections::HashMap;
use std::path::PathBuf;
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
pub use store::HnswMemoryIndex;

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

/// Memory entry type — expanded from 5 to 9 types.
/// Existing Knowledge is preserved for backward compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    // Existing types (unchanged — backward compat)
    /// Conversation compaction summary (auto-generated).
    Conversation,
    /// Session-end summary (auto-generated).
    Session,
    /// A factual statement (e.g., "API uses port 3000").
    Fact,
    /// An event or experience (e.g., "deployed v0.2.0").
    Episode,
    /// Static knowledge (user/program-provided, knowledge-base synced).
    /// Preserved from knowledge_lens.rs. **Do not remove.**
    Knowledge,

    // New types (from SOAR/ACT-R cognitive model)
    /// A learned procedure or pattern (e.g., "run cargo test before commit").
    Skill,
    /// A user preference (e.g., "use Korean for user-facing messages").
    Preference,
    /// A recorded decision with rationale (e.g., "chose HNSW over FAISS").
    Decision,
    /// User identity and expertise profile.
    UserProfile,
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
            MemoryType::Skill => "memory/skills",
            MemoryType::Preference => "memory/preferences",
            MemoryType::Decision => "memory/decisions",
            MemoryType::UserProfile => "memory/profiles",
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
            MemoryType::Skill => "skill",
            MemoryType::Preference => "preference",
            MemoryType::Decision => "decision",
            MemoryType::UserProfile => "user_profile",
        }
    }

    /// Base importance for each type.
    pub fn base_importance(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.95,
            MemoryType::Preference => 0.90,
            MemoryType::Decision => 0.80,
            MemoryType::Knowledge => 0.75,
            MemoryType::Skill => 0.75,
            MemoryType::Fact => 0.60,
            MemoryType::Episode => 0.50,
            MemoryType::Session => 0.40,
            MemoryType::Conversation => 0.35,
        }
    }

    /// Base decay rate for each type.
    pub fn base_decay_rate(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.001,
            MemoryType::Preference => 0.002,
            MemoryType::Decision => 0.005,
            MemoryType::Knowledge => 0.006,
            MemoryType::Skill => 0.008,
            MemoryType::Fact => 0.015,
            MemoryType::Episode => 0.025,
            MemoryType::Session => 0.040,
            MemoryType::Conversation => 0.060,
        }
    }

    /// Initial tier for new entries of this type.
    pub fn initial_tier(&self) -> MemoryTier {
        match self {
            // Hot: immediately needed in context
            MemoryType::UserProfile
            | MemoryType::Preference
            | MemoryType::Decision
            | MemoryType::Fact => MemoryTier::Hot,
            // Warm: on-demand access
            MemoryType::Knowledge
            | MemoryType::Skill
            | MemoryType::Episode
            | MemoryType::Session
            | MemoryType::Conversation => MemoryTier::Warm,
        }
    }

    /// Whether this type is automatically protected from deletion.
    pub fn is_auto_protected(&self) -> bool {
        matches!(self, MemoryType::UserProfile | MemoryType::Preference)
    }

    /// Whether this type is stored globally (cross-Space).
    pub fn is_global(&self) -> bool {
        matches!(self, MemoryType::UserProfile | MemoryType::Preference)
    }

    /// All memory type variants.
    pub fn all() -> &'static [MemoryType] {
        &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
            MemoryType::Skill,
            MemoryType::Preference,
            MemoryType::Decision,
            MemoryType::UserProfile,
        ]
    }
}

// ---------------------------------------------------------------------------
// MemoryTier
// ---------------------------------------------------------------------------

/// Memory tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// Always loaded into agent context (~3K tokens).
    Hot,
    /// Loaded on demand (recent sessions, knowledge).
    Warm,
    /// Compressed archive (long-term storage).
    Cold,
}

impl MemoryTier {
    /// Maximum entries per tier (configurable).
    pub fn default_max_entries(&self) -> usize {
        match self {
            MemoryTier::Hot => 50,
            MemoryTier::Warm => 500,
            MemoryTier::Cold => 10_000,
        }
    }

    /// Maximum token budget per tier.
    pub fn default_token_budget(&self) -> usize {
        match self {
            MemoryTier::Hot => 3_000,
            MemoryTier::Warm => 50_000,
            MemoryTier::Cold => usize::MAX,
        }
    }
}

fn default_tier() -> MemoryTier {
    MemoryTier::Warm
}

// ---------------------------------------------------------------------------
// ProtectionLevel
// ---------------------------------------------------------------------------

/// Auto-protection level. Users never need to know about this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionLevel {
    /// No protection. Normal decay + deletion.
    None = 0,
    /// Slow decay, deletion possible.
    /// Trigger: 2+ accesses.
    Low = 1,
    /// Very slow decay. Deletion only after retention_days × 2.
    /// Trigger: 3+ accesses or 2+ session appearances.
    Medium = 2,
    /// Near-permanent. Preserved in LLM compaction.
    /// Trigger: 5+ accesses, 3+ sessions, or user "remember this".
    High = 3,
    /// Absolute protection. Never deleted or compressed.
    /// Trigger: UserProfile/Preference type, or explicit user pin.
    Permanent = 4,
}

impl ProtectionLevel {
    /// Decay multiplier based on protection level.
    pub fn decay_multiplier(&self) -> f32 {
        match self {
            ProtectionLevel::None => 1.0,
            ProtectionLevel::Low => 0.5,
            ProtectionLevel::Medium => 0.2,
            ProtectionLevel::High => 0.05,
            ProtectionLevel::Permanent => 0.0,
        }
    }
}

impl Default for ProtectionLevel {
    fn default() -> Self {
        ProtectionLevel::None
    }
}

/// A single memory entry — extended with lifecycle + auto-protection metadata.
/// All new fields use `#[serde(default)]` for backward compatibility with
/// existing stored JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // ── Identity ──────────────────────────────────────
    /// Unique ID.
    pub id: String,
    /// Memory type (auto-classified if not explicitly set).
    pub memory_type: MemoryType,
    /// Current tier (auto-managed by Dream).
    #[serde(default = "default_tier")]
    pub tier: MemoryTier,

    // ── Content ───────────────────────────────────────
    /// Content (Markdown).
    pub content: String,
    /// Content hash for deduplication.
    #[serde(default)]
    pub content_hash: u64,
    /// Tags (auto-extracted from content).
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Source ────────────────────────────────────────
    /// Creator (agent name, "compaction", "system", "dream", "auto-classify").
    pub source: String,
    /// Related session ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Related space ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    // ── Importance ────────────────────────────────────
    /// Base importance (0.0–1.0), set by type or auto-computed.
    #[serde(default = "default_importance")]
    pub importance: f32,
    /// Whether user explicitly pinned (optional override).
    #[serde(default)]
    pub pinned: bool,

    // ── Auto-Protection ───────────────────────────────
    /// Auto-computed protection level. Dream recomputes each run.
    #[serde(default)]
    pub protection: ProtectionLevel,
    /// Whether the type was auto-classified (vs explicit).
    #[serde(default)]
    pub auto_classified: bool,
    /// Number of distinct sessions this entry appeared in.
    #[serde(default)]
    pub session_appearances: u32,
    /// Whether the user has corrected/contradicted this entry's topic.
    #[serde(default)]
    pub user_corrected: bool,
    /// Session IDs that have accessed this entry (for dedup of session_appearances).
    /// Max 100 entries; oldest evicted first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen_in_sessions: Vec<String>,

    // ── Lifecycle ─────────────────────────────────────
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last access timestamp.
    pub accessed_at: DateTime<Utc>,
    /// Last modification timestamp.
    #[serde(default = "default_now")]
    pub modified_at: DateTime<Utc>,
    /// Access count.
    #[serde(default)]
    pub access_count: u32,
    /// Current decay score (0.0–1.0), computed by DecayEngine.
    #[serde(default = "default_importance")]
    pub decay_score: f32,
    /// Compaction level (0 = raw, 1 = daily, 2 = weekly, 3 = monthly, 4 = root).
    #[serde(default)]
    pub compaction_level: u8,
    /// IDs of entries this was compacted from.
    #[serde(default)]
    pub compacted_from: Vec<String>,

    // ── Relationships ─────────────────────────────────
    /// IDs of related memory entries.
    #[serde(default)]
    pub related_ids: Vec<String>,
    /// Contradicts a previous entry (ID of the contradicted entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<String>,
}

fn default_importance() -> f32 {
    0.5
}

fn default_now() -> DateTime<Utc> {
    Utc::now()
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
    /// Optional HNSW index for fast ANN search.
    hnsw_index: RwLock<Option<Arc<HnswMemoryIndex>>>,
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
            hnsw_index: RwLock::new(None),
        }
    }

    /// Attach a git layer for version-controlled saves.
    pub fn set_git_layer(&mut self, gl: Arc<GitLayer>) {
        self.git_layer = Some(gl);
    }

    /// Create a Space-scoped MemoryManager.
    ///
    /// Each Space gets its own StateStore under the given directory,
    /// providing natural memory isolation between Spaces.
    pub fn for_space(space_dir: PathBuf) -> Self {
        let memory_dir = space_dir.join("memory");
        let state_store = Arc::new(StateStore::new(memory_dir).unwrap_or_else(|_| {
            // Fallback: create in temp dir
            StateStore::new(std::env::temp_dir().join("oxios-memory")).unwrap()
        }));
        Self::new(state_store)
    }

    /// Attach an HNSW index for fast semantic search.
    ///
    /// Once attached, `remember()` and `forget()` automatically keep
    /// the HNSW index in sync with the state store.
    pub fn set_hnsw_index(&self, index: Arc<HnswMemoryIndex>) {
        *self.hnsw_index.write() = Some(index);
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
            if self
                .forget(&candidate.id, candidate.memory_type)
                .await
                .is_ok()
            {
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
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "can", "shall",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into", "through",
        "during", "before", "after", "above", "below", "between", "out", "off", "over", "under",
        "again", "further", "then", "once", "and", "but", "or", "nor", "not", "so", "yet", "both",
        "either", "neither", "each", "every", "all", "any", "few", "more", "most", "other", "some",
        "such", "no", "only", "own", "same", "than", "too", "very", "just", "because", "if",
        "when", "where", "how", "what", "which", "who", "whom", "this", "that", "these", "those",
        "i", "me", "my", "we", "our", "you", "your", "he", "him", "his", "she", "her", "it", "its",
        "they", "them", "their",
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

pub mod auto_classify;
pub mod auto_memory_bridge;
mod budget;
mod chunking;
mod compaction;
mod decay;
mod dream;
pub mod embedding_cache;
pub mod flash_attention;
mod graph;
mod hnsw;
pub mod hyperbolic;
mod normalizer;
mod proactive;
mod root_index;
pub mod sona;
pub(crate) mod store;

pub use auto_classify::AutoClassifier;
pub use compaction::CompactionTree;
pub use decay::DecayEngine;
pub use dream::{DreamCheckpoint, DreamProcess, DreamReport};
pub use proactive::ProactiveRecall;
pub use root_index::{HistoricalPeriod, RootEntry, RootIndex, TopicEntry};

pub use embedding_cache::{CacheStats, EmbeddingCache};
pub use store::SemanticHit;

// Re-export key types from sub-modules.
pub use chunking::{chunk_fixed, chunk_paragraphs, ChunkConfig, TextChunk};
pub use graph::MemoryGraph;
pub use hnsw::HnswIndex;
pub use normalizer::{cosine_similarity_f32, l2_normalize_f32, l2_normalize_f64};

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
        let mgr = MemoryManager::new(Arc::new(
            StateStore::new(std::env::temp_dir().join("test")).unwrap(),
        ));
        let result = mgr.blend_into_prompt(&[], "You are an agent.");
        assert_eq!(result, "You are an agent.");
    }

    #[test]
    fn test_blend_into_prompt_with_memories() {
        let mgr = MemoryManager::new(Arc::new(
            StateStore::new(std::env::temp_dir().join("test")).unwrap(),
        ));
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

        assert!(v1.cosine_similarity(&v2) > 0.1, "Korean+code similarity");
        assert!(v1.cosine_similarity(&v3) < 0.1, "Korean different topics");
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
        let results = mgr
            .search("systems programming with rust", None, 5)
            .await
            .unwrap();
        assert!(!results.is_empty(), "Vector search should find results");
        assert_eq!(
            results[0].id, "vec-test-1",
            "Should find the Rust entry first"
        );
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
        store
            .save_json("memory/facts", "rebuild-test-1", &entry)
            .await
            .unwrap();

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
            tier: MemoryTier::Warm,
            content: format!("Test content for {}", id),
            content_hash: 0,
            tags: vec![],
            source: "test".to_string(),
            session_id: None,
            space_id: None,
            importance: 0.5,
            pinned: false,
            protection: ProtectionLevel::None,
            auto_classified: false,
            session_appearances: 0,
            user_corrected: false,
            seen_in_sessions: vec![],
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            modified_at: Utc::now(),
            access_count: 0,
            decay_score: 1.0,
            compaction_level: 0,
            compacted_from: vec![],
            related_ids: vec![],
            contradicts: None,
        }
    }
}
