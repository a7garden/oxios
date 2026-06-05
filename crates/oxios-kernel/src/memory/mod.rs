//! Agent memory system.
//!
//! Provides persistent memory for agents across sessions.
//! Memory entries are stored as JSON files via StateStore.
//! Supports embedding-based vector search using TF-IDF + cosine similarity.
//!
//! ## Module Activity Status (RFC-018)
//!
//! Core types and leaf modules live in `oxios-memory`. This module
//! re-exports them for back-compat and keeps kernel-coupled modules
//! (store, dream, sqlite, etc.) here.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
#[cfg(test)]
use chrono::Utc;
use parking_lot::RwLock;

use crate::embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
use crate::git_layer::GitLayer;
use crate::state_store::StateStore;

// Re-export all types from oxios-memory (RFC-018 back-compat)
pub use oxios_memory::memory::types::{
    content_hash, dedup_by_id, extract_keywords, MemoryEntry, MemoryTier, MemoryType,
    ProtectionLevel, TextVector,
};

// Re-export leaf modules from oxios-memory
pub use oxios_memory::memory::{
    auto_classify::AutoClassifier,
    auto_protect::AutoProtector,
    compaction::CompactionTree,
    decay::DecayEngine,
    embedding_cache::{CacheStats, EmbeddingCache},
    embedding_viz::{compute_pca_2d, compute_top_neighbors, MemoryMapEntry, MemoryNeighbor},
    flash_attention::{BenchmarkResult, FlashAttention, FlashAttentionConfig, MemoryEstimate},
    graph::MemoryGraph,
    hnsw::HnswIndex,
    quota::{CurationCandidate, CurationReport, MemoryBudget},
    root_index::{HistoricalPeriod, RootEntry, RootIndex, TopicEntry},
};

// Re-export from store (still in kernel)
pub use store::HnswMemoryIndex;
pub use store::SemanticHit;

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
    pub fn set_sona_engine(&mut self, engine: Arc<sona::SonaEngine>) {
        self.sona_engine = Some(engine);
    }

    /// Get a reference to the SONA engine (if configured).
    pub fn sona_engine(&self) -> Option<&Arc<sona::SonaEngine>> {
        self.sona_engine.as_ref()
    }

    /// Create a Space-scoped MemoryManager.
    pub fn for_space(space_dir: PathBuf) -> Self {
        let memory_dir = space_dir.join("memory");
        let state_store = Arc::new(StateStore::new(memory_dir).unwrap_or_else(|_| {
            StateStore::new(std::env::temp_dir().join("oxios-memory")).unwrap()
        }));
        Self::new(state_store)
    }

    /// Attach an HNSW index for fast semantic search.
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
    pub fn effective_importance(entry: &MemoryEntry) -> f32 {
        let access_boost = (1.0_f32 + entry.access_count as f32).ln();
        entry.importance * (1.0 + access_boost)
    }

    /// Curate memories: identify candidates for removal based on budget.
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
// Sub-modules (still in kernel — coupled to StateStore/KernelHandle)
// ---------------------------------------------------------------------------

// Kernel-coupled sub-modules
pub mod auto_memory_bridge;
#[cfg(feature = "sqlite-memory")]
pub mod cache;
#[cfg(feature = "sqlite-memory")]
pub mod database;
pub mod dream;
#[cfg(feature = "sqlite-memory")]
pub mod hyperbolic_persist;
#[cfg(feature = "sqlite-memory")]
pub mod migration;
pub mod proactive;
#[cfg(feature = "sqlite-memory")]
pub mod search;
pub mod sona;
#[cfg(feature = "sqlite-memory")]
pub mod sqlite_store;
pub(crate) mod store;

// Re-export dream types
pub use dream::{DreamCheckpoint, DreamProcess, DreamReport};
pub use proactive::{ProactiveRecall, RecallTiming};

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
        assert!(!kw.contains(&"how".to_string()));
        assert!(!kw.contains(&"do".to_string()));
    }

    #[test]
    fn test_dedup_by_id() {
        let mut entries = vec![
            make_entry("a", MemoryType::Fact),
            make_entry("b", MemoryType::Fact),
            make_entry("a", MemoryType::Episode),
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

    #[test]
    fn test_text_vector_cosine_similarity() {
        let v1 = TextVector::from_text("fix the null pointer error in main.rs");
        let v2 = TextVector::from_text("null pointer error found in rust code");
        let v3 = TextVector::from_text("update the documentation for deployment");

        assert!(v1.cosine_similarity(&v2) > 0.3);
        assert!(v1.cosine_similarity(&v3) < 0.2);
    }

    #[test]
    fn test_text_vector_multilingual() {
        let v1 = TextVector::from_text("main.rs 파일의 null pointer 에러 수정");
        let v2 = TextVector::from_text("null pointer 오류를 수정했습니다");
        let v3 = TextVector::from_text("문서 업데이트 배포 가이드");

        assert!(v1.cosine_similarity(&v2) > 0.1);
        assert!(v1.cosine_similarity(&v3) < 0.1);
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
        assert!((sim - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_tokenize_multilingual() {
        let terms = TextVector::tokenize("main.rs 파일의 버그를 수정");
        assert!(!terms.is_empty());
    }

    #[tokio::test]
    async fn test_vector_search_over_keyword_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store.clone());

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

        let results = mgr
            .search("systems programming with rust", None, 5)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "vec-test-1");
    }

    #[tokio::test]
    async fn test_rebuild_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let mgr = MemoryManager::new(store.clone());

        let entry = make_entry_with_content(
            "rebuild-test-1",
            MemoryType::Fact,
            "memory for rebuild test",
        );
        store
            .save_json("memory/facts", "rebuild-test-1", &entry)
            .await
            .unwrap();

        assert_eq!(mgr.vector_index.read().len(), 0);

        mgr.rebuild_index().await.unwrap();

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
