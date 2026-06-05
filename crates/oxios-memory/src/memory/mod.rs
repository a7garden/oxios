//! Memory subsystem modules — extracted from `oxios-kernel` per RFC-018.
//!
//! ## Module categories
//!
//! | Category | Modules | Description |
//! |----------|---------|-------------|
//! | **Types** | `types` | Core types (MemoryEntry, MemoryType, MemoryTier, etc.) |
//! | **Text** | `chunking`, `normalizer` | Text splitting and vector math |
//! | **Geometry** | `hyperbolic` | Poincaré ball model embeddings |
//! | **Lifecycle** | `decay`, `auto_classify`, `auto_protect`, `compaction`, `quota` | Memory lifecycle |
//! | **Analysis** | `graph`, `flash_attention`, `embedding_cache`, `embedding_viz`, `hnsw` | Analysis & indexing |
//! | **Index** | `root_index` | Root memory index |
//! | **Storage** | `storage` | Storage abstraction traits |

// ─── Embedding ────────────────────────────────────────────────────────
pub mod embedding;
pub use embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};

// ─── Core types ─────────────────────────────────────────────────────
pub mod types;
pub use types::{
    content_hash, dedup_by_id, extract_keywords, MemoryEntry, MemoryTier, MemoryType,
    ProtectionLevel, TextVector,
};

// ─── Text / math utilities ──────────────────────────────────────────
pub mod chunking;
pub mod hyperbolic;
pub mod normalizer;

// ─── Memory lifecycle ───────────────────────────────────────────────
pub mod auto_classify;
pub mod auto_protect;
pub mod compaction;
pub mod decay;
pub mod quota;

// ─── Analysis & indexing ────────────────────────────────────────────
pub mod embedding_cache;
pub mod embedding_viz;
pub mod flash_attention;
pub mod graph;
pub mod hnsw;
pub mod root_index;

// ─── Storage abstraction ────────────────────────────────────────────
pub mod storage;

// ─── Re-exports (b.1 — chunking/normalizer/hyperbolic) ──────────────
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

// ─── Re-exports (lifecycle) ─────────────────────────────────────────
pub use auto_classify::AutoClassifier;
pub use auto_protect::AutoProtector;
pub use compaction::CompactionTree;
pub use decay::DecayEngine;
pub use embedding_cache::{CacheStats, EmbeddingCache};
pub use embedding_viz::{compute_pca_2d, compute_top_neighbors, MemoryMapEntry, MemoryNeighbor};
pub use flash_attention::{BenchmarkResult, FlashAttention, FlashAttentionConfig, MemoryEstimate};
pub use graph::MemoryGraph;
pub use hnsw::HnswIndex;
pub use quota::{CurationCandidate, CurationReport, MemoryBudget};
pub use root_index::{HistoricalPeriod, RootEntry, RootIndex, TopicEntry};
