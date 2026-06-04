//! Oxios Memory — tiered agent memory extracted from `oxios-kernel`.
//!
//! ## Status (RFC-018 b.1)
//!
//! This crate holds the memory subsystem modules that have been extracted
//! from `oxios-kernel`:
//!
//! - **b.1 (current)**: `chunking`, `normalizer`, `hyperbolic` (math/text utilities)
//! - **b.2+**: `embedding`, `root_index`, `quota`, `decay`, etc.
//!
//! `oxios-kernel` depends on this crate (not the other way around) for
//! the extracted modules. Remaining memory types (MemoryManager, etc.)
//! stay in `oxios-kernel` until b.7.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use oxios_memory::chunk_fixed;         // moved from oxios_kernel
//! use oxios_memory::HyperbolicEmbedding; // moved from oxios_kernel
//! use oxios_memory::cosine_similarity_f32; // moved from oxios_kernel
//! ```

#![warn(missing_docs)]

// ─── Memory subsystem modules (extracted from oxios-kernel) ──
pub mod memory;

// Re-export storage traits from this crate's memory module
pub use crate::memory::{MemoryGit, MemoryStorage};

// Re-export extracted types (RFC-018 b.1)
pub use crate::memory::chunking::{chunk_fixed, chunk_paragraphs, ChunkConfig, TextChunk};
pub use crate::memory::normalizer::{
    cosine_similarity_f32, dot_product_f32, l2_norm_f32, l2_norm_f64, l2_normalize_f32,
    l2_normalize_f64,
};
