//! Memory subsystem modules — extracted from `oxios-kernel` per RFC-018 b.1.
//!
//! - `chunking` — Text splitting utilities (no internal deps)
//! - `normalizer` — L2 normalization, cosine similarity
//! - `hyperbolic` — Poincaré ball model embeddings
//! - `storage` — Storage abstraction traits (`MemoryStorage`, `MemoryGit`)

pub mod chunking;
pub mod hyperbolic;
pub mod normalizer;
pub mod storage;

pub use chunking::{chunk_fixed, chunk_paragraphs, ChunkConfig, TextChunk};
pub use hyperbolic::{
    batch_euclidean_to_poincare, euclidean_to_poincare, hyperbolic_distance, mobius_add,
    mobius_scalar_mul, HyperbolicConfig, HyperbolicEmbedding,
};
pub use normalizer::{
    cosine_similarity_f32, dot_product_f32, l2_norm_f32, l2_norm_f64, l2_normalize_f32,
    l2_normalize_f64,
};
pub use storage::{MemoryGit, MemoryStorage};
