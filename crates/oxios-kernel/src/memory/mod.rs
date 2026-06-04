//! Agent memory system.
//!
//! Provides persistent memory for agents across sessions.
//! Memory entries are stored as JSON files via StateStore.
//! Supports embedding-based vector search using TF-IDF + cosine similarity.
//!
//! ## Module Activity Status (RFC-017, 2026-05)
//!
//! 모든 모듈은 활성 경로에서 사용된다:
//!
//! | 범주 | 모듈 | 핵심 역할 |
//! |------|------|----------|
//! | **핵심** | store, sqlite_store, search | CRUD + 영속화 + 검색 |
//! | **통합** | dream | 4-phase 백그라운드 통합 |
//! | **분석** | graph, hnsw, flash_attention | PageRank, ANN, re-ranking |
//! | **생명주기** | decay, auto_protect, auto_classify, compaction | 감쇠/보호/분류/압축 |
//! | **인프라** | cache, embedding_cache, database, migration, migrate | 캐시/스키마/마이그레이션 |
//! | **유틸** | budget, normalizer, chunking | 예산/정규화/청킹 |
//! | **학습** | sona, proactive | ⚠️ 구현됨, RFC-020에서 활성화 예정 |
//!
//! 삭제된 모듈 (git history에 보존):
//! - `reasoning_bank` (RFC-017): Ouroboros가 동일 역할 담당
//! - `rvf_store` (RFC-017): LLM 에이전트에 부적합한 RL/EWC 개념

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
#[cfg(test)]
use chrono::Utc;
use parking_lot::RwLock;

use crate::git_layer::GitLayer;
use crate::state_store::StateStore;
use oxios_memory::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};

// Re-export budget types so external `use crate::memory::X` paths still work.
// (`quota` and `root_index` moved to oxios-memory in RFC-018 b.3.)
pub use oxios_memory::memory::{
    CurationCandidate, CurationReport, HistoricalPeriod, MemoryBudget, MemoryEntry, MemoryTier,
    MemoryType, ProtectionLevel, RootEntry, RootIndex, TopicEntry,
};
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
// Core types (MemoryType, MemoryTier, ProtectionLevel, MemoryEntry) — moved
// to oxios-memory in RFC-018 b.3. Re-exported from `oxios_kernel::memory::*`
// for back-compat (existing `use crate::memory::MemoryType;` paths still work).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// MemoryManager
// ---------------------------------------------------------------------------

/// Agent memory manager.
///
/// Stores and retrieves memory entries using a [`oxios_memory::MemoryStorage`]
/// backend (concretely, `oxios-kernel::StateStore`). Supports embedding-based
/// vector search via an in-memory TF-IDF index that is rebuilt on startup.
pub struct MemoryManager {
    state_store: Arc<dyn oxios_memory::MemoryStorage>,
    max_recall: usize,
    /// Vector index for semantic search (id → EmbeddingVector).
    vector_index: RwLock<HashMap<String, EmbeddingVector>>,
    /// Embedding provider for generating vectors.
    embedding: Arc<dyn EmbeddingProvider>,
    /// Optional git layer for version-controlled memory.
    git_layer: Option<Arc<dyn oxios_memory::MemoryGit>>,
    /// Optional HNSW index for fast ANN search.
    hnsw_index: RwLock<Option<Arc<HnswMemoryIndex>>>,
    /// Optional SONA learning engine (RFC-020 Phase 2).
    /// Shared via Arc so DreamProcess and AgentRuntime can access concurrently.
    sona_engine: Option<Arc<sona::SonaEngine>>,
    /// Optional SQLite-backed store (RFC-012). When present, remember/search
    /// operations delegate here instead of StateStore.
    #[cfg(feature = "sqlite-memory")]
    sqlite_store: Option<Arc<crate::memory::sqlite_store::SqliteMemoryStore>>,
}

impl std::fmt::Debug for MemoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryManager")
            .field("max_recall", &self.max_recall)
            .field("index_size", &self.vector_index.read().len())
            .field("sona_enabled", &self.sona_engine.is_some())
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
            sona_engine: None,
            #[cfg(feature = "sqlite-memory")]
            sqlite_store: None,
        }
    }

    /// Attach a git layer for version-controlled saves.
    pub fn set_git_layer(&mut self, gl: Arc<GitLayer>) {
        self.git_layer = Some(gl);
    }

    /// Attach a SQLite-backed memory store (RFC-012).
    ///
    /// When present, `remember()`, `search()`, `recall()`, and other
    /// operations will delegate to the SQLite store instead of the
    /// file-based StateStore.
    #[cfg(feature = "sqlite-memory")]
    pub fn set_sqlite_store(&mut self, store: Arc<crate::memory::sqlite_store::SqliteMemoryStore>) {
        self.sqlite_store = Some(store);
    }

    /// Get a reference to the SQLite store (if configured).
    #[cfg(feature = "sqlite-memory")]
    pub fn sqlite_store(&self) -> &Option<Arc<crate::memory::sqlite_store::SqliteMemoryStore>> {
        &self.sqlite_store
    }

    /// Attach a SONA learning engine (RFC-020 Phase 2).
    ///
    /// Once attached, `sona_engine()` returns the engine for
    /// trajectory recording, pattern distillation, and adaptation.
    pub fn set_sona_engine(&mut self, engine: Arc<sona::SonaEngine>) {
        self.sona_engine = Some(engine);
    }

    /// Get a reference to the SONA engine (if configured).
    pub fn sona_engine(&self) -> Option<&Arc<sona::SonaEngine>> {
        self.sona_engine.as_ref()
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
                // Fire-and-forget: the commit happens in a background task.
                // Before RFC-018 b.6 this was a sync call; now `commit_file`
                // returns a future (via the MemoryGit trait) so we spawn it
                // to keep the surrounding MemoryManager methods sync.
                let gl = gl.clone();
                let rel_path = rel_path.to_string();
                let message = message.to_string();
                tokio::spawn(async move {
                    if let Err(e) = gl.commit_file(&rel_path, &message).await {
                        tracing::warn!(error = %e, path = %rel_path, "git commit failed (non-fatal)");
                    }
                });
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
//
// `auto_classify`, `auto_protect`, `decay` moved to oxios-memory in
// RFC-018 b.4. Re-exported below for back-compat.

pub mod auto_memory_bridge;
#[cfg(feature = "sqlite-memory")]
pub mod cache;
#[cfg(feature = "sqlite-memory")]
pub mod database;
pub mod dream;
pub mod embedding_cache;
pub mod embedding_viz;
mod hnsw;
#[cfg(feature = "sqlite-memory")]
pub mod migration;
mod proactive;
#[cfg(feature = "sqlite-memory")]
pub mod search;
pub mod sona;
#[cfg(feature = "sqlite-memory")]
pub mod sqlite_store;
pub(crate) mod store;

pub use dream::{DreamCheckpoint, DreamProcess, DreamReport};
pub use proactive::ProactiveRecall;
pub use proactive::RecallTiming;

pub use embedding_cache::{CacheStats, EmbeddingCache};
pub use embedding_viz::{compute_pca_2d, compute_top_neighbors, MemoryMapEntry, MemoryNeighbor};
pub use store::SemanticHit;

// Re-export key types from sub-modules.
pub use hnsw::HnswIndex;

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
    // Note: `TextVector` itself moved to `oxios-memory` in RFC-018 b.2.
    // Its own tests live in `oxios-memory/src/memory/text_vector.rs`.

    #[tokio::test]
    async fn test_vector_search_over_keyword_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store.clone());

        // Store some memories
        let entry1 = make_entry_with_content(
            "vec-test-1",
            MemoryType::Fact,
            "Rust is a systems programming language focused on safety",
        );
        let entry2 = make_entry_with_content(
            "vec-test-2",
            MemoryType::Fact,
            "Python is great for machine learning and data science",
        );

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
        let entry = make_entry_with_content(
            "rebuild-test-1",
            MemoryType::Fact,
            "memory for rebuild test",
        );
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
        make_entry_with_content(id, ty, &format!("Test content for {}", id))
    }

    fn make_entry_with_content(id: &str, ty: MemoryType, content: &str) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            memory_type: ty,
            tier: MemoryTier::Warm,
            content: content.to_string(),
            content_hash: 0,
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
