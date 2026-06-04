//! Oxios Memory — math/text utilities and storage traits for agent memory.
//!
//! ## Status (RFC-018 b.1)
//!
//! This crate holds memory-related modules extracted from `oxios-kernel`:
//!
//! - **b.1 (current)**: `chunking`, `normalizer`, `hyperbolic` (math/text
//!   utilities) and `storage` (storage abstraction traits).
//! - **b.2+**: `embedding`, `root_index`, `quota`, `decay`, etc.
//!
//! `oxios-kernel` depends on this crate (not the other way around) for
//! the extracted modules. Remaining memory types (`MemoryManager`, etc.)
//! stay in `oxios-kernel` until b.7.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use oxios_memory::chunk_fixed;          // moved from oxios_kernel (b.1)
//! use oxios_memory::HyperbolicEmbedding;   // moved from oxios_kernel (b.1)
//! use oxios_memory::MemoryStorage;         // defined here
//! ```

#![warn(missing_docs)]

// ─── Memory subsystem modules (extracted from oxios-kernel) ──
pub mod memory;

// ─── Memory math/text utilities (RFC-018 b.1) ────────────────
pub use crate::memory::hyperbolic::{
    batch_euclidean_to_poincare, euclidean_to_poincare, hyperbolic_distance, mobius_add,
    mobius_scalar_mul, HyperbolicConfig, HyperbolicEmbedding,
};
pub use crate::memory::{
    chunk_fixed,
    chunk_paragraphs,
    cosine_similarity_f32,
    l2_normalize_f32,
    l2_normalize_f64, // normalizer
    ChunkConfig,
    MemoryGit,
    MemoryStorage, // storage traits
    TextChunk,     // chunking
};
