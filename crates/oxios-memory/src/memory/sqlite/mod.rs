//! SQLite-backed memory storage (RFC-012).
//!
//! Feature-gated under `sqlite-memory`. Provides:
//! - `MemoryDatabase` — schema, connections, schema initialization
//! - `SqliteMemoryStore` — CRUD operations
//! - `search` — BM25 + vector + RRF hybrid search
//! - `cache` — embedding cache
//! - `migration` — JSON → SQLite one-time migration
//! - `hyperbolic_persist` — HyperbolicEmbedding SQLite adapter

pub mod cache;
pub mod database;
pub mod hyperbolic_persist;
pub mod migration;
pub mod search;
pub mod store;

pub use database::{bytes_to_f32_slice, f32_slice_to_bytes, MemoryDatabase};
pub use store::SqliteMemoryStore;
