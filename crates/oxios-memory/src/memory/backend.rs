//! Memory backend abstraction — the strategy pattern for storage.
//!
//! `MemoryBackend` unifies the JSON file-based path and the SQLite backend
//! behind a single trait. `MemoryManager` delegates all CRUD to whichever
//! backend is active, eliminating `#[cfg]` branching in business logic.
//!
//! The JSON path is implemented by `MemoryManager` itself (via `state_store`).
//! The SQLite path is implemented by `SqliteMemoryStore`.

use anyhow::Result;

use crate::memory::types::{MemoryEntry, MemoryTier, MemoryType};

/// Storage backend strategy for memory CRUD.
///
/// Implementors provide the core read/write operations. The trait is
/// dyn-compatible (no generics) so `MemoryManager` can hold
/// `Option<Arc<dyn MemoryBackend>>`.
#[async_trait::async_trait]
pub trait MemoryBackend: Send + Sync {
    /// Store a memory entry. Returns the entry ID.
    async fn remember(&self, entry: &MemoryEntry) -> Result<String>;

    /// Retrieve a single memory by ID and type.
    fn get(&self, id: &str, memory_type: MemoryType) -> Result<Option<MemoryEntry>>;

    /// Retrieve a single memory by ID (searches all types).
    fn get_by_id(&self, id: &str) -> Result<Option<MemoryEntry>>;

    /// Delete a memory entry. Returns true if it existed.
    fn forget(&self, id: &str, memory_type: MemoryType) -> Result<bool>;

    /// List memories of a given type, most recent first.
    fn list(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<MemoryEntry>>;

    /// Search memories by query (semantic + keyword hybrid).
    async fn search(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>>;

    /// Recall relevant memories for a query.
    async fn recall(&self, query: &str, max_recall: usize) -> Result<Vec<MemoryEntry>>;

    /// Recall with Flash Attention re-ranking.
    async fn recall_with_rerank(&self, query: &str, max_recall: usize) -> Result<Vec<MemoryEntry>>;

    /// Blend recalled memories into the system prompt.
    fn blend_into_prompt(&self, memories: &[MemoryEntry], system_prompt: &str) -> String;

    /// Check if a memory entry with identical content already exists.
    async fn is_duplicate(&self, content: &str) -> bool;

    /// Store a memory entry only if no duplicate content exists.
    async fn remember_unique(&self, entry: &MemoryEntry) -> Result<Option<String>>;

    /// List memories by tier.
    fn list_by_tier(&self, tier: MemoryTier, limit: usize) -> Result<Vec<MemoryEntry>>;
}
