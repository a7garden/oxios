//! Memory subsystem modules — extracted from `oxios-kernel` per RFC-018.
//!
//! - `auto_classify`   — Auto memory type classification (b.4)
//! - `auto_protect`    — Auto protection level computation (b.4)
//! - `chunking`        — Text splitting utilities (b.1)
//! - `compaction`      — 5-level memory compression tree (b.5)
//! - `decay`           — Ebbinghaus-inspired decay engine (b.4)
//! - `embedding`       — Embedding providers (TfIdf, GGUF) (b.2)
//! - `flash_attention` — Block-wise attention algorithm (b.5)
//! - `graph`           — PageRank memory link graph (b.5)
//! - `hyperbolic`      — Poincaré ball model embeddings (b.1)
//! - `normalizer`      — L2 normalization, cosine similarity (b.1)
//! - `quota`           — Curation budgets and reports (b.3)
//! - `root_index`      — ROOT index for O(1) topic lookup (b.3)
//! - `storage`         — Storage abstraction traits (`MemoryStorage`, `MemoryGit`) (b.0)
//! - `text_vector`     — TF-IDF text vector (b.2, supports embedding)
//! - `types`           — Core data types (MemoryType, MemoryEntry, etc.) (b.3)

pub mod auto_classify;
pub mod auto_protect;
pub mod chunking;
pub mod compaction;
pub mod decay;
pub mod embedding;
pub mod flash_attention;
pub mod graph;
pub mod hyperbolic;
pub mod normalizer;
pub mod quota;
pub mod root_index;
pub mod storage;
pub mod storage_ext;
pub mod text_vector;
pub mod types;

pub use auto_classify::AutoClassifier;
pub use auto_protect::AutoProtector;
pub use chunking::{chunk_fixed, chunk_paragraphs, ChunkConfig, TextChunk};
pub use compaction::CompactionTree;
pub use decay::DecayEngine;
pub use embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
#[cfg(feature = "embedding-gguf")]
pub use embedding::gguf::{EmbeddingDimension, GgufEmbeddingProvider, GgufModelLoader};
pub use hyperbolic::{
    batch_euclidean_to_poincare, euclidean_to_poincare, hyperbolic_distance, mobius_add,
    mobius_scalar_mul, HyperbolicConfig, HyperbolicEmbedding,
};
pub use normalizer::{
    cosine_similarity_f32, dot_product_f32, l2_norm_f32, l2_norm_f64, l2_normalize_f32,
    l2_normalize_f64,
};
pub use quota::{CurationCandidate, CurationReport, MemoryBudget};
pub use root_index::{HistoricalPeriod, RootEntry, RootIndex, TopicEntry};
pub use storage::{MemoryGit, MemoryStorage};
pub use text_vector::TextVector;
pub use types::{MemoryEntry, MemoryTier, MemoryType, ProtectionLevel};
