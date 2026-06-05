//! Agent memory system.
//!
//! Core types and leaf modules live in [`oxios_memory`]. This module
//! re-exports them for back-compat and keeps kernel-coupled modules here.
//!
//! ## Structure
//!
//! ```text
//! oxios-memory (crate)       oxios-kernel/memory (this module)
//! ─────────────────────      ──────────────────────────────────
//! types.rs                   memory_manager.rs   — MemoryManager struct
//! embedding.rs               store.rs            — CRUD operations
//! chunking.rs                memory_ops.rs       — advanced search & tier ops
//! normalizer.rs              hnsw_memory_index.rs — HNSW index wrapper
//! hyperbolic.rs              dream.rs            — Dream consolidation
//! auto_classify.rs           auto_memory_bridge.rs — external memory sync
//! auto_protect.rs            proactive.rs        — proactive recall
//! compaction.rs              sona.rs             — learning engine
//! decay.rs                   sqlite_store.rs     — SQLite backend
//! graph.rs                   database.rs         — SQLite schema
//! flash_attention.rs         cache.rs            — embedding cache (SQLite)
//! embedding_cache.rs         search/             — BM25 + vector + RRF
//! embedding_viz.rs           migration.rs        — JSON → SQLite migration
//! hnsw.rs                    hyperbolic_persist.rs
//! root_index.rs
//! quota.rs
//! storage.rs (traits)
//! ```

// Re-export all types from oxios-memory (back-compat)
pub use oxios_memory::memory::types::{
    content_hash, dedup_by_id, extract_keywords, MemoryEntry, MemoryTier, MemoryType,
    ProtectionLevel, TextVector,
};

// Re-export leaf modules from oxios-memory
pub use oxios_memory::l2_normalize_f32;
pub use oxios_memory::memory::embedding_viz::{compute_pca_2d, compute_top_neighbors};
pub use oxios_memory::HnswIndex;
pub use oxios_memory::{
    AutoClassifier, AutoProtector, CacheStats, CompactionTree, CurationCandidate, CurationReport,
    DecayEngine, EmbeddingCache, FlashAttention, FlashAttentionConfig, HistoricalPeriod,
    MemoryBudget, MemoryEstimate, MemoryGraph, MemoryMapEntry, MemoryNeighbor, RootEntry,
    RootIndex, TopicEntry,
};

// Re-export from kernel sub-modules
pub use hnsw_memory_index::{HnswMemoryIndex, SemanticHit};
pub use memory_manager::MemoryManager;

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

/// HNSW index wrapper + semantic search hit type.
pub mod hnsw_memory_index;
/// Core struct + constructor + curation.
mod memory_manager;
/// Advanced operations (semantic search, HNSW rebuild, tier shift, pin).
mod memory_ops;
/// CRUD operations (remember, forget, list, search, recall, blend).
pub(crate) mod store;

// Kernel-coupled sub-modules
pub mod auto_memory_bridge;
pub mod dream;
pub mod proactive;
pub mod sona;

#[cfg(feature = "sqlite-memory")]
pub mod cache;
#[cfg(feature = "sqlite-memory")]
pub mod database;
#[cfg(feature = "sqlite-memory")]
pub mod hyperbolic_persist;
#[cfg(feature = "sqlite-memory")]
pub mod migration;
#[cfg(feature = "sqlite-memory")]
pub mod search;
#[cfg(feature = "sqlite-memory")]
pub mod sqlite_store;

// Re-export dream types
pub use dream::{DreamCheckpoint, DreamProcess, DreamReport};
pub use proactive::{ProactiveRecall, RecallTiming};
