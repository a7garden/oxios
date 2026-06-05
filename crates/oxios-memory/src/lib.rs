//! Oxios Memory — tiered agent memory extracted from `oxios-kernel`.
//!
//! ## Status (RFC-018)
//!
//! This crate holds the memory subsystem extracted from `oxios-kernel`:
//!
//! - **b.1**: `chunking`, `normalizer`, `hyperbolic` (math/text utilities)
//! - **b.2**: `embedding` (TF-IDF + GGUF dense vectors)
//! - **b.3**: `root_index`, `quota`
//! - **b.4**: `decay`, `auto_classify`, `auto_protect`
//! - **b.5**: `compaction`, `flash_attention`, `graph`, `embedding_cache`, `embedding_viz`
//! - **b.6**: `MemoryStorage` trait + `StateStore` impl
//! - **b.7**: `MemoryManager` move
//! - **b.8**: SQLite backend
//! - **b.9**: `migrate`, `dream`, `auto_memory_bridge`
//!
//! `oxios-kernel` depends on this crate (not the other way around) for
//! all memory types and modules.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use oxios_memory::MemoryEntry;
//! use oxios_memory::MemoryType;
//! use oxios_memory::chunk_fixed;
//! use oxios_memory::HyperbolicEmbedding;
//! use oxios_memory::cosine_similarity_f32;
//! ```

#![warn(missing_docs)]

// ─── Memory subsystem modules (extracted from oxios-kernel) ──
pub mod memory;

// Re-export storage traits
pub use crate::memory::{MemoryGit, MemoryStorage};

// Re-export core types (RFC-018)
pub use crate::memory::types::{
    content_hash, dedup_by_id, extract_keywords, MemoryEntry, MemoryTier, MemoryType,
    ProtectionLevel, TextVector,
};

// Re-export extracted modules (b.1 — chunking/normalizer/hyperbolic)
pub use crate::memory::{
    chunk_fixed, chunk_paragraphs, cosine_similarity_f32, l2_normalize_f32, l2_normalize_f64,
    ChunkConfig, HyperbolicConfig, HyperbolicEmbedding, TextChunk,
};

// Re-export lifecycle modules (b.3-b.5)
pub use crate::memory::{
    AutoClassifier, AutoProtector, CacheStats, CompactionTree, CurationCandidate, CurationReport,
    DecayEngine, EmbeddingCache, FlashAttention, FlashAttentionConfig, HistoricalPeriod,
    MemoryBudget, MemoryEstimate, MemoryGraph, MemoryMapEntry, MemoryNeighbor, RootEntry,
    RootIndex, TopicEntry,
};

// Re-export HNSW
pub use crate::memory::hnsw::HnswIndex;
