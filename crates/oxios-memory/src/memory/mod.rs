//! Memory subsystem modules — extracted from `oxios-kernel` per RFC-018.
//!
//! - `auto_classify`   — Auto memory type classification (b.4)
//! - `auto_protect`    — Auto protection level computation (b.4)
//! - `cache`           — In-memory entry cache (b.7, sqlite feature)
//! - `chunking`        — Text splitting utilities (b.1)
//! - `compaction`      — 5-level memory compression tree (b.5)
//! - `database`        — SQLite memory database wrapper (b.7, sqlite feature)
//! - `decay`           — Ebbinghaus-inspired decay engine (b.4)
//! - `embedding`       — Embedding providers (TfIdf, GGUF) (b.2)
//! - `embedding_cache` — Embedding vector cache (b.7)
//! - `embedding_viz`    — Embedding visualization utilities (b.7)
//! - `flash_attention` — Block-wise attention algorithm (b.5)
//! - `graph`           — PageRank memory link graph (b.5)
//! - `hnsw`            — HNSW ANN index (b.7)
//! - `hyperbolic`      — Poincaré ball model embeddings (b.1)
//! - `migration`       — JSON→SQLite migration (b.7, sqlite feature)
//! - `normalizer`      — L2 normalization, cosine similarity (b.1)
//! - `proactive`       — Proactive recall (b.7)
//! - `quota`           — Curation budgets and reports (b.3)
//! - `root_index`      — ROOT index for O(1) topic lookup (b.3)
//! - `search`          — Hybrid search (BM25 + vector + RRF) (b.7, sqlite feature)
//! - `sona`            — Self-Optimizing Neural Architecture (b.7)
//! - `sqlite_store`    — SQLite memory backend (b.7, sqlite feature)
//! - `storage`         — Storage abstraction traits (`MemoryStorage`, `MemoryGit`) (b.0)
//! - `store`           — `MemoryManager` runtime (b.7)
//! - `text_vector`     — TF-IDF text vector (b.2, supports embedding)
//! - `types`           — Core data types (MemoryType, MemoryEntry, etc.) (b.3)

pub mod auto_classify;
pub mod auto_protect;
#[cfg(feature = "sqlite-memory")]
pub mod cache;
pub mod chunking;
pub mod compaction;
#[cfg(feature = "sqlite-memory")]
pub mod database;
pub mod decay;
pub mod embedding;
pub mod embedding_cache;
pub mod embedding_viz;
pub mod flash_attention;
pub mod graph;
pub mod helpers;
pub mod hnsw;
pub mod hyperbolic;
#[cfg(feature = "sqlite-memory")]
pub mod migration;
pub mod normalizer;
pub mod proactive;
pub mod quota;
pub mod root_index;
#[cfg(feature = "sqlite-memory")]
pub mod search;
pub mod sona;
#[cfg(feature = "sqlite-memory")]
pub mod sqlite_store;
pub mod storage;
pub mod storage_ext;
pub mod store;
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
pub use hnsw::HnswIndex;
pub use hyperbolic::{
    batch_euclidean_to_poincare, euclidean_to_poincare, hyperbolic_distance, mobius_add,
    mobius_scalar_mul, HyperbolicConfig, HyperbolicEmbedding,
};
pub use normalizer::{
    cosine_similarity_f32, dot_product_f32, l2_norm_f32, l2_norm_f64, l2_normalize_f32,
    l2_normalize_f64,
};
pub use proactive::{ProactiveRecall, RecallTiming};
pub use quota::{CurationCandidate, CurationReport, MemoryBudget};
pub use root_index::{HistoricalPeriod, RootEntry, RootIndex, TopicEntry};
pub use sona::SonaEngine;
pub use storage::{MemoryGit, MemoryStorage};
pub use store::{HnswMemoryIndex, MemoryManager, SemanticHit};
pub use text_vector::TextVector;
pub use types::{MemoryEntry, MemoryTier, MemoryType, ProtectionLevel};
