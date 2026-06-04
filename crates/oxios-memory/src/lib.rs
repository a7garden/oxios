//! Oxios Memory — *facade* crate for the memory subsystem.
//!
//! ## Status (2026-06-04)
//!
//! This crate is a *facade* that:
//! - **Re-exports** memory types from `oxios-kernel` (Phase B pending)
//! - **Defines** storage abstraction traits (`MemoryStorage`, `MemoryGit`)
//!   that `oxios-kernel` *will* implement when extraction is complete
//!
//! The actual code still lives in `oxios-kernel`. Progressive extraction
//! is tracked in [RFC-016].
//!
//! [RFC-016]: https://github.com/a7garden/oxios/blob/main/docs/rfc-016-kernel-boundary-cleanup.md
//!
//! ## Usage
//!
//! Code that wants to use memory should depend on this crate rather
//! than `oxios-kernel` directly:
//!
//! ```rust,ignore
//! use oxios_memory::MemoryManager;  // re-exported from oxios-kernel
//! use oxios_memory::MemoryStorage;   // defined here
//! ```

#![warn(missing_docs)]

// ─── Storage abstraction traits (canonical, in this crate) ──
pub mod memory;

// ─── Memory core types (re-exported from oxios-kernel) ───────
pub use oxios_kernel::memory::{
    AutoClassifier, ChunkConfig, CompactionTree, CurationCandidate, CurationReport,
    DecayEngine, DreamCheckpoint, DreamProcess, DreamReport, HistoricalPeriod,
    HnswIndex, HnswMemoryIndex, MemoryEntry, MemoryGraph, MemoryManager, MemoryTier,
    MemoryType, ProactiveRecall, ProtectionLevel, RootEntry, RootIndex, SemanticHit,
    TextChunk, TextVector, TopicEntry,
    chunk_fixed, chunk_paragraphs, content_hash, cosine_similarity_f32,
    l2_normalize_f32, l2_normalize_f64,
};

pub use oxios_kernel::memory::MemoryBudget;

// Re-export storage traits from this crate's memory module
pub use crate::memory::{MemoryGit, MemoryStorage};

// ─── Embedding providers (re-exported) ────────────────────────
pub use oxios_kernel::embedding::{
    EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider,
};

// ─── Configuration types (re-exported) ───────────────────────
pub use oxios_kernel::config::{
    ConsolidationConfig, MemoryBridgeConfig, MemoryConfig, SqliteMemoryConfig,
};
