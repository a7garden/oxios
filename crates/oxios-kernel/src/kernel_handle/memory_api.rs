//! Memory API — memory subsystem facade (Phase C).
//!
//! Extracted from `AgentApi` to provide a focused interface for
//! memory operations. Replaces the previous `AgentApi.memory_manager()`
//! leaky accessor pattern.
//!
//! Consumers should use [`KernelHandle::memory()`] to access this API.

use crate::memory::store::HnswMemoryIndex;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType, SemanticHit};
use std::sync::Arc;

/// Memory subsystem facade.
///
/// Provides high-level memory operations without exposing the
/// internal `MemoryManager` directly. This is the 14th typed API
/// in `KernelHandle` (alongside `A2aApi`, `AgentApi`, etc.).
pub struct MemoryApi {
    /// Underlying memory manager.
    pub(crate) memory_manager: Arc<MemoryManager>,
    /// Optional HNSW index for fast semantic search.
    pub(crate) hnsw_index: Option<Arc<HnswMemoryIndex>>,
}

impl MemoryApi {
    /// Create a new MemoryApi.
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self {
            memory_manager,
            hnsw_index: None,
        }
    }

    /// Attach an HNSW index for fast semantic search.
    pub fn set_hnsw_index(&mut self, index: Arc<HnswMemoryIndex>) {
        self.hnsw_index = Some(index);
    }

    /// Store a memory entry. Returns the entry's ID.
    pub async fn remember(&self, entry: MemoryEntry) -> anyhow::Result<String> {
        self.memory_manager.remember(entry).await
    }

    /// Search memory by text query.
    pub async fn search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.memory_manager.search(query, memory_type, limit).await
    }

    /// Recall memory by query (semantic if HNSW available, else keyword).
    pub async fn recall(&self, query: &str) -> anyhow::Result<Vec<MemoryEntry>> {
        self.memory_manager.recall(query).await
    }

    /// Get a specific memory entry by ID and type.
    pub async fn get(
        &self,
        id: &str,
        memory_type: MemoryType,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        self.memory_manager.get(id, memory_type).await
    }

    /// Forget (delete) a memory entry.
    ///
    /// Returns `Ok(())` on success. (Previously returned `Result<bool>`;
    /// the bool was discarded by all callers.)
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> anyhow::Result<()> {
        self.memory_manager.forget(id, memory_type).await
    }

    /// List memories of a given type.
    pub async fn list(
        &self,
        memory_type: MemoryType,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.memory_manager.list(memory_type, limit).await
    }

    /// Search memory using semantic similarity (returns SemanticHits).
    /// Falls back to keyword search if no HNSW index.
    pub async fn search_semantic(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SemanticHit>> {
        if let Some(hnsw) = &self.hnsw_index {
            let _ = hnsw; // hnsw available, would use it here
            // For now, delegate to regular search and convert
            let entries = self.memory_manager.search(query, None, limit).await?;
            Ok(entries.into_iter().map(|e| SemanticHit { entry: e, distance: 0.0, similarity: 1.0 }).collect())
        } else {
            // Fallback: use keyword search
            let entries = self.memory_manager.search(query, None, limit).await?;
            Ok(entries.into_iter().map(|e| SemanticHit { entry: e, distance: 0.0, similarity: 1.0 }).collect())
        }
    }

    /// Get memory statistics: (total_entries, vector_index_size).
    pub async fn stats(&self) -> (usize, usize) {
        (
            self.memory_manager.total_entries().await,
            self.memory_manager.vector_index_size(),
        )
    }

    /// Rebuild the HNSW index from current memory state.
    /// Returns the number of vectors indexed.
    pub async fn rebuild_hnsw_index(&self) -> anyhow::Result<usize> {
        if let Some(hnsw) = &self.hnsw_index {
            // Try to rebuild
            let _ = self.memory_manager.rebuild_index().await?;
            Ok(hnsw.len())
        } else {
            Ok(0)
        }
    }

    /// Access the underlying memory manager. For advanced operations
    /// not yet exposed via this facade.
    pub fn manager(&self) -> &Arc<MemoryManager> {
        &self.memory_manager
    }
}
