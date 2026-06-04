//! Memory subsystem modules — extracted from `oxios-kernel` per RFC-018.
//!
//! - `chunking`     — Text splitting utilities (b.1)
//! - `embedding`    — Embedding providers (TfIdf, GGUF) (b.2)
//! - `hyperbolic`   — Poincaré ball model embeddings (b.1)
//! - `normalizer`   — L2 normalization, cosine similarity (b.1)
//! - `quota`        — Curation budgets and reports (b.3)
//! - `root_index`   — ROOT index for O(1) topic lookup (b.3)
//! - `storage`      — Storage abstraction traits (`MemoryStorage`, `MemoryGit`) (b.0)
//! - `text_vector`  — TF-IDF text vector (b.2, supports embedding)
//! - `types`        — Core data types (MemoryType, MemoryEntry, etc.) (b.3)

pub mod chunking;
pub mod embedding;
pub mod hyperbolic;
pub mod normalizer;
pub mod quota;
pub mod root_index;
pub mod storage;
pub mod text_vector;
pub mod types;

pub use chunking::{chunk_fixed, chunk_paragraphs, ChunkConfig, TextChunk};
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
