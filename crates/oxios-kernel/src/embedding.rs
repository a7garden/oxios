//! Embedding abstraction — re-exported from `oxios-memory`.
//!
//! The actual implementations live in `oxios-memory::memory::embedding`.
//! This module re-exports them for back-compat.

pub use oxios_memory::memory::embedding::{
    EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider,
};

#[cfg(feature = "embedding-gguf")]
pub mod gguf;
