//! Memory manager — the central orchestrator for agent memory.
//!
//! `MemoryManager` coordinates CRUD, indexing, search, and lifecycle
//! operations for memory entries. It wraps an abstract `MemoryStorage`
//! backend and optional SQLite store, HNSW index, git layer, and SONA engine.
//!
//! All kernel-coupled types (`StateStore`, `GitLayer`, `MemoryConfig`) are
//! accessed through traits defined in [`crate::storage`]. The kernel
//! implements those traits and injects concrete instances.

mod ops;
mod store;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;

use crate::memory::embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
use crate::memory::hnsw_memory_index::HnswMemoryIndex;
use crate::memory::sona::SonaEngine;
use crate::memory::storage::{MemoryGit, MemoryStorage};
use crate::memory::types::{MemoryEntry, MemoryType};

use super::{CurationCandidate, CurationReport, MemoryBudget};

// ---------------------------------------------------------------------------
// MemoryManager
// ---------------------------------------------------------------------------

/// Agent memory manager.
///
/// Stores and retrieves memory entries using a pluggable storage backend.
/// Supports embedding-based vector search via an in-memory TF-IDF index
/// that is rebuilt on startup.
pub struct MemoryManager {
    /// Storage backend (typically `StateStore` from kernel).
    pub(crate) storage: Arc<dyn MemoryStorage>,
    /// Maximum memories returned by recall.
    pub(crate) max_recall: usize,
    /// Vector index for semantic search (id → EmbeddingVector).
    pub(crate) vector_index: RwLock<HashMap<String, EmbeddingVector>>,
    /// Embedding provider for generating vectors.
    pub(crate) embedding: Arc<dyn EmbeddingProvider>,
    /// Optional git layer for version-controlled memory.
    pub(crate) git: Option<Arc<dyn MemoryGit>>,
    /// Optional HNSW index for fast ANN search.
    pub(crate) hnsw_index: RwLock<Option<Arc<HnswMemoryIndex>>>,
    /// Optional SONA learning engine (RFC-020 Phase 2).
    pub(crate) sona_engine: Option<Arc<SonaEngine>>,
    /// Optional SQLite-backed store (RFC-012).
    #[cfg(feature = "sqlite-memory")]
    pub(crate) sqlite_store: Option<Arc<crate::memory::sqlite::SqliteMemoryStore>>,
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
    /// Create a new MemoryManager with a storage backend.
    pub fn new(storage: Arc<dyn MemoryStorage>) -> Self {
        Self {
            storage,
            max_recall: 10,
            vector_index: RwLock::new(HashMap::new()),
            embedding: Arc::new(TfIdfEmbeddingProvider),
            git: None,
            hnsw_index: RwLock::new(None),
            sona_engine: None,
            #[cfg(feature = "sqlite-memory")]
            sqlite_store: None,
        }
    }

    /// Attach a git layer for version-controlled saves.
    pub fn set_git_layer(&mut self, gl: Arc<dyn MemoryGit>) {
        self.git = Some(gl);
    }

    /// Attach a SQLite-backed memory store (RFC-012).
    #[cfg(feature = "sqlite-memory")]
    pub fn set_sqlite_store(&mut self, store: Arc<crate::memory::sqlite::SqliteMemoryStore>) {
        self.sqlite_store = Some(store);
    }

    /// Get a reference to the SQLite store (if configured).
    #[cfg(feature = "sqlite-memory")]
    pub fn sqlite_store(&self) -> &Option<Arc<crate::memory::sqlite::SqliteMemoryStore>> {
        &self.sqlite_store
    }

    /// Attach a SONA learning engine (RFC-020 Phase 2).
    pub fn set_sona_engine(&mut self, engine: Arc<SonaEngine>) {
        self.sona_engine = Some(engine);
    }

    /// Get a reference to the SONA engine (if configured).
    pub fn sona_engine(&self) -> Option<&Arc<SonaEngine>> {
        self.sona_engine.as_ref()
    }

    /// Attach an HNSW index for fast semantic search.
    pub fn set_hnsw_index(&self, index: Arc<HnswMemoryIndex>) {
        *self.hnsw_index.write() = Some(index);
    }

    /// Set max memories returned by recall.
    pub fn with_max_recall(mut self, n: usize) -> Self {
        self.max_recall = n;
        self
    }

    /// Set max_recall in-place.
    pub fn set_max_recall(&mut self, n: usize) {
        self.max_recall = n;
    }

    /// Returns the number of entries in the vector index.
    pub fn vector_index_size(&self) -> usize {
        self.vector_index.read().len()
    }

    /// Commit a file to git if git_layer is available.
    pub(crate) async fn git_commit(&self, rel_path: &str, message: &str) {
        if let Some(ref gl) = self.git
            && gl.is_enabled()
        {
            let _ = gl.commit_file(rel_path, message).await;
        }
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
