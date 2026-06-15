//! HNSW index manager for memory entries.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::HnswIndex;
use super::MemoryEntry;
use super::l2_normalize_f32;

/// Result of a semantic search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHit {
    /// Memory entry.
    pub entry: MemoryEntry,
    /// Cosine distance (0.0 = identical).
    pub distance: f32,
    /// Cosine similarity (1.0 = identical).
    pub similarity: f32,
}

/// HNSW index manager for memory entries.
///
/// Maintains a mapping from u64 keys to String IDs, and the HNSW index
/// itself. Thread-safe via `RwLock`.
pub struct HnswMemoryIndex {
    /// The HNSW index.
    index: RwLock<HnswIndex>,
    /// Map: u64 key → String memory ID.
    key_to_id: RwLock<HashMap<u64, String>>,
    /// Map: String memory ID → u64 key.
    id_to_key: RwLock<HashMap<String, u64>>,
    /// Next key counter.
    next_key: AtomicU64,
    /// Base path for index persistence.
    persist_path: Option<PathBuf>,
}

impl std::fmt::Debug for HnswMemoryIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HnswMemoryIndex")
            .field("size", &self.len())
            .field("dimensions", &self.index.read().dimensions())
            .finish()
    }
}

impl HnswMemoryIndex {
    /// Create a new HNSW memory index.
    ///
    /// # Arguments
    /// * `dimensions` — Embedding vector dimensions.
    /// * `capacity` — Initial capacity hint.
    /// * `persist_path` — Optional directory for index file persistence.
    pub fn new(dimensions: usize, capacity: usize, persist_path: Option<PathBuf>) -> Result<Self> {
        let index = HnswIndex::new(dimensions, capacity)?;
        Ok(Self {
            index: RwLock::new(index),
            key_to_id: RwLock::new(HashMap::new()),
            id_to_key: RwLock::new(HashMap::new()),
            next_key: AtomicU64::new(1), // 0 is used by usearch as sentinel
            persist_path,
        })
    }

    /// Try to restore from disk, fall back to new index.
    pub fn restore_or_new(
        dimensions: usize,
        capacity: usize,
        persist_path: Option<PathBuf>,
    ) -> Result<Self> {
        if let Some(ref path) = persist_path {
            let index_path = path.join("memory.usearch");
            let mapping_path = path.join("key_map.json");

            if index_path.exists() && mapping_path.exists() {
                tracing::info!(path = %index_path.display(), "Restoring HNSW index from disk");

                if let Ok(index) = HnswIndex::load(&index_path)
                    && let Ok(data) = std::fs::read_to_string(&mapping_path)
                    && let Ok((k2i, i2k)) =
                        serde_json::from_str::<(HashMap<u64, String>, HashMap<String, u64>)>(&data)
                {
                    let max_key = k2i.keys().max().copied().unwrap_or(0);
                    return Ok(Self {
                        index: RwLock::new(index),
                        key_to_id: RwLock::new(k2i),
                        id_to_key: RwLock::new(i2k),
                        next_key: AtomicU64::new(max_key + 1),
                        persist_path,
                    });
                }

                tracing::warn!("Failed to restore HNSW index, creating new one");
            }
        }

        Self::new(dimensions, capacity, persist_path)
    }

    /// Get or create a u64 key for a String ID.
    fn get_or_create_key(&self, id: &str) -> u64 {
        // Fast path: check read lock
        {
            let i2k = self.id_to_key.read();
            if let Some(&key) = i2k.get(id) {
                return key;
            }
        }

        // Slow path: write lock
        let mut i2k = self.id_to_key.write();
        let mut k2i = self.key_to_id.write();

        // Double-check after acquiring write lock
        if let Some(&key) = i2k.get(id) {
            return key;
        }

        let key = self.next_key.fetch_add(1, Ordering::Relaxed);
        i2k.insert(id.to_string(), key);
        k2i.insert(key, id.to_string());
        key
    }

    /// Add an entry to the HNSW index.
    pub fn add_entry(&self, id: &str, vector: &[f32]) -> Result<()> {
        let key = self.get_or_create_key(id);
        let mut normalized = vector.to_vec();
        l2_normalize_f32(&mut normalized);
        self.index.write().add(key, &normalized)?;
        Ok(())
    }

    /// Remove an entry from the index.
    pub fn remove_entry(&self, id: &str) -> Result<()> {
        let key = {
            let i2k = self.id_to_key.read();
            i2k.get(id).copied()
        };
        if let Some(key) = key {
            self.index.write().remove(key)?;
            let mut k2i = self.key_to_id.write();
            let mut i2k = self.id_to_key.write();
            k2i.remove(&key);
            i2k.remove(id);
        }
        Ok(())
    }

    /// Search for k nearest neighbors.
    ///
    /// Returns (String ID, distance) pairs.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let mut normalized = query.to_vec();
        l2_normalize_f32(&mut normalized);

        let raw = self.index.read().search(&normalized, k)?;
        let k2i = self.key_to_id.read();

        let results = raw
            .into_iter()
            .filter_map(|(key, dist)| k2i.get(&key).map(|id| (id.clone(), dist)))
            .collect();

        Ok(results)
    }

    /// Number of entries in the index.
    pub fn len(&self) -> usize {
        self.index.read().len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.index.read().is_empty()
    }

    /// Save the index and key mappings to disk.
    pub fn persist(&self) -> Result<()> {
        if let Some(ref path) = self.persist_path {
            std::fs::create_dir_all(path)?;

            let index_path = path.join("memory.usearch");
            let mapping_path = path.join("key_map.json");

            // Save index
            self.index.read().save(&index_path)?;

            // Save key mappings
            let k2i = self.key_to_id.read();
            let i2k = self.id_to_key.read();
            let data = serde_json::to_string(&(k2i.clone(), &*i2k))?;
            std::fs::write(&mapping_path, data)?;

            tracing::debug!(path = %path.display(), entries = self.len(), "HNSW index persisted");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Semantic search on MemoryManager
// ---------------------------------------------------------------------------
