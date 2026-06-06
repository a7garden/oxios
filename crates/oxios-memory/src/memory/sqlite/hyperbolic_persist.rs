//! SQLite persistence adapter for `HyperbolicEmbedding`.
//!
//! Per RFC-018 b.1, the pure-math core of hyperbolic embeddings lives in
//! `oxios_memory::memory::hyperbolic`. The cfg-gated SQLite persistence
//! methods that depend on `SqliteMemoryStore` (a kernel type) are kept
//! here as *free functions* that take a `SqliteMemoryStore` argument.
//!
//! This is a kernel-side adapter — it bridges oxios-memory's math with
//! kernel's storage. The methods are exposed through the same names as
//! before, so callers (`dream.rs`) continue to compile with minimal
//! changes.
//!
//! ## Migration note
//!
//! Previously these were inherent methods on `HyperbolicEmbedding`:
//! ```ignore
//! HyperbolicEmbedding::build_from_sqlite(&store, config).await;
//! HyperbolicEmbedding::restore_from_sqlite(&store, config)?;
//! he.persist_to_sqlite(&store)?;
//! ```
//!
//! They are now free functions in this module:
//! ```ignore
//! use oxios_kernel::memory::hyperbolic_persist;
//! hyperbolic_persist::build_from_sqlite(&store, config).await;
//! hyperbolic_persist::restore_from_sqlite(&store, config)?;
//! hyperbolic_persist::persist_to_sqlite(&he, &store)?;
//! ```

#[cfg(feature = "sqlite-memory")]
use anyhow::Result;
#[cfg(feature = "sqlite-memory")]
use crate::memory::hyperbolic::{HyperbolicConfig, HyperbolicEmbedding};

#[cfg(feature = "sqlite-memory")]
use super::store::SqliteMemoryStore;

/// Build a `HyperbolicEmbedding` from memory entries in SQLite.
///
/// Takes all memories, generates Euclidean embeddings via the
/// embedding provider, and converts to Poincaré ball coordinates.
#[cfg(feature = "sqlite-memory")]
pub async fn build_from_sqlite(
    store: &SqliteMemoryStore,
    config: HyperbolicConfig,
) -> HyperbolicEmbedding {
    let mut he = HyperbolicEmbedding::new(config);

    // Load all memories
    for mt in crate::memory::MemoryType::all() {
        if let Ok(entries) = store.list(*mt, 10_000) {
            for entry in entries {
                // Get the dense embedding from cache or compute
                if let Ok(Some(vec)) = store.get_query_vector(&entry.content).await {
                    he.add(&entry.id, &vec);
                }
            }
        }
    }

    tracing::debug!(count = he.len(), "Built hyperbolic embedding from SQLite");
    he
}

/// Persist hyperbolic embeddings to SQLite `dream_state`.
///
/// Stores as JSON blob under key `hyperbolic_embeddings`.
#[cfg(feature = "sqlite-memory")]
pub fn persist_to_sqlite(he: &HyperbolicEmbedding, store: &SqliteMemoryStore) -> Result<()> {
    let data: Vec<(&String, &Vec<f32>)> =
        he.all_embeddings().iter().map(|(id, v)| (id, v)).collect();
    let json = serde_json::to_string(&data)?;

    let conn = store.db().conn();
    conn.execute(
        "INSERT OR REPLACE INTO dream_state (key, value) VALUES ('hyperbolic_embeddings', ?1)",
        rusqlite::params![json],
    )?;

    tracing::debug!(
        count = he.len(),
        "Hyperbolic embeddings persisted to SQLite"
    );
    Ok(())
}

/// Restore hyperbolic embeddings from SQLite `dream_state`.
#[cfg(feature = "sqlite-memory")]
pub fn restore_from_sqlite(
    store: &SqliteMemoryStore,
    config: HyperbolicConfig,
) -> Result<HyperbolicEmbedding> {
    let conn = store.db().conn();
    let json: Option<String> = conn
        .query_row(
            "SELECT value FROM dream_state WHERE key = 'hyperbolic_embeddings'",
            [],
            |row| row.get(0),
        )
        .ok();

    let he = if let Some(data) = json {
        if let Ok(pairs) = serde_json::from_str::<Vec<(String, Vec<f32>)>>(&data) {
            HyperbolicEmbedding::from_pairs(pairs)
        } else {
            HyperbolicEmbedding::new(config)
        }
    } else {
        HyperbolicEmbedding::new(config)
    };

    tracing::debug!(
        count = he.len(),
        "Hyperbolic embeddings restored from SQLite"
    );
    Ok(he)
}
