//! HNSW-based approximate nearest neighbor index via `usearch`.
//!
//! Wraps the usearch library to provide a Rust-friendly HNSW index for
//! high-dimensional dense vector search. Supports persistence (save/load),
//! add/remove, and k-NN search.

use std::path::Path;

use anyhow::{Context, Result};
use usearch::ffi::{IndexOptions, MetricKind, ScalarKind};
use usearch::Index;

/// Default vector dimensions (OpenAI text-embedding-3-small).
const DEFAULT_DIMENSIONS: usize = 1536;

/// Default connectivity (HNSW graph edges per node).
const DEFAULT_CONNECTIVITY: usize = 16;

/// Default expansion factor for search.
const DEFAULT_EXPANSION_SEARCH: usize = 128;

/// Default expansion factor for add.
const DEFAULT_EXPANSION_ADD: usize = 128;

// ---------------------------------------------------------------------------
// HnswIndex
// ---------------------------------------------------------------------------

/// HNSW approximate nearest neighbor index.
///
/// Wraps `usearch::Index` and provides a type-safe, ergonomic interface
/// for dense vector operations. The index is not thread-safe internally —
/// callers must synchronize access (e.g., via `parking_lot::RwLock`).
pub struct HnswIndex {
    /// Underlying usearch index.
    index: Index,
    /// Vector dimensions.
    dimensions: usize,
}

impl std::fmt::Debug for HnswIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HnswIndex")
            .field("dimensions", &self.dimensions)
            .field("size", &self.len())
            .finish()
    }
}

impl HnswIndex {
    /// Create a new HNSW index.
    ///
    /// # Arguments
    /// * `dimensions` — Dimensionality of vectors (e.g., 1536 for OpenAI).
    /// * `capacity` — Initial capacity hint (pre-allocated slots).
    pub fn new(dimensions: usize, capacity: usize) -> Result<Self> {
        let options = IndexOptions {
            dimensions,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: DEFAULT_CONNECTIVITY,
            expansion_add: DEFAULT_EXPANSION_ADD,
            expansion_search: DEFAULT_EXPANSION_SEARCH,
            multi: false,
        };

        let index = Index::new(&options).context("Failed to create HNSW index")?;
        if capacity > 0 {
            index
                .reserve(capacity)
                .map_err(|e| anyhow::anyhow!("Failed to reserve HNSW capacity: {e}"))?;
        }

        Ok(Self { index, dimensions })
    }

    /// Create with default dimensions (1536).
    pub fn with_default_dims(capacity: usize) -> Result<Self> {
        Self::new(DEFAULT_DIMENSIONS, capacity)
    }

    /// Add a vector to the index with the given key.
    ///
    /// The key is a u64 identifier. Callers should maintain a mapping
    /// from u64 key to logical ID (e.g., via SQLite).
    pub fn add(&self, key: u64, vector: &[f32]) -> Result<()> {
        anyhow::ensure!(
            vector.len() == self.dimensions,
            "Vector dimension mismatch: expected {}, got {}",
            self.dimensions,
            vector.len()
        );
        self.index
            .add(key, vector)
            .map_err(|e| anyhow::anyhow!("HNSW add failed for key {key}: {e}"))?;
        Ok(())
    }

    /// Search for the k nearest neighbors of the query vector.
    ///
    /// Returns a sorted list of (key, distance) pairs.
    /// Distance is cosine distance (0.0 = identical for normalized vectors).
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>> {
        anyhow::ensure!(
            query.len() == self.dimensions,
            "Query dimension mismatch: expected {}, got {}",
            self.dimensions,
            query.len()
        );
        if k == 0 {
            return Ok(Vec::new());
        }

        let results = self
            .index
            .search(query, k)
            .map_err(|e| anyhow::anyhow!("HNSW search failed: {e}"))?;

        Ok(results
            .keys
            .into_iter()
            .zip(results.distances)
            .filter(|(k, _)| *k != 0)
            .collect())
    }

    /// Remove a vector by key.
    pub fn remove(&self, key: u64) -> Result<()> {
        self.index
            .remove(key)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("HNSW remove failed for key {key}: {e}"))
    }

    /// Check if a key exists in the index.
    pub fn contains(&self, key: u64) -> bool {
        self.index.contains(key)
    }

    /// Get the vector stored for a key.
    pub fn get(&self, key: u64) -> Option<Vec<f32>> {
        let mut buffer = vec![0.0f32; self.dimensions];
        match self.index.get(key, &mut buffer) {
            Ok(count) if count > 0 => Some(buffer),
            _ => None,
        }
    }

    /// Number of vectors currently in the index.
    pub fn len(&self) -> usize {
        self.index.size()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Vector dimensions.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Save the index to a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let path_str = path.to_str().ok_or_else(|| {
            anyhow::anyhow!("HNSW save path is not valid UTF-8: {}", path.display())
        })?;
        self.index
            .save(path_str)
            .map_err(|e| anyhow::anyhow!("HNSW save failed: {e}"))?;
        Ok(())
    }

    /// Load (restore) an index from a file.
    ///
    /// Returns a new `HnswIndex` with the same dimensions as the saved index.
    pub fn load(path: &Path) -> Result<Self> {
        let path_str = path.to_str().ok_or_else(|| {
            anyhow::anyhow!("HNSW load path is not valid UTF-8: {}", path.display())
        })?;
        let index =
            Index::restore(path_str).map_err(|e| anyhow::anyhow!("HNSW load failed: {e}"))?;
        let dimensions = index.dimensions();
        Ok(Self { index, dimensions })
    }

    /// Reserve additional capacity.
    pub fn reserve(&self, capacity: usize) -> Result<()> {
        self.index
            .reserve(capacity)
            .map_err(|e| anyhow::anyhow!("HNSW reserve failed: {e}"))?;
        Ok(())
    }

    /// Rename a key (reassign vector from old key to new key).
    pub fn rename(&self, from: u64, to: u64) -> Result<()> {
        self.index
            .rename(from, to)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("HNSW rename failed: {from} -> {to}: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_hnsw_add_and_search() {
        let index = HnswIndex::new(3, 100).unwrap();

        let v1: Vec<f32> = vec![1.0, 0.0, 0.0];
        let v2: Vec<f32> = vec![0.0, 1.0, 0.0];
        let v3: Vec<f32> = vec![0.0, 0.0, 1.0];

        index.add(1, &v1).unwrap();
        index.add(2, &v2).unwrap();
        index.add(3, &v3).unwrap();

        assert_eq!(index.len(), 3);

        // Search for nearest to v1
        let results = index.search(&v1, 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
        // Cosine distance should be ~0 for identical vectors
        assert!(
            results[0].1 < 0.01,
            "Distance should be ~0, got {}",
            results[0].1
        );
    }

    #[test]
    fn test_hnsw_search_multiple() {
        let index = HnswIndex::new(4, 100).unwrap();

        // Two clusters
        index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.add(2, &[0.9, 0.1, 0.0, 0.0]).unwrap();
        index.add(3, &[0.0, 1.0, 0.0, 0.0]).unwrap();
        index.add(4, &[0.0, 0.9, 0.1, 0.0]).unwrap();

        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        // First result should be key 1 (exact match)
        assert_eq!(results[0].0, 1);
        // Second should be key 2 (nearest neighbor)
        assert_eq!(results[1].0, 2);
    }

    #[test]
    fn test_hnsw_dimension_mismatch() {
        let index = HnswIndex::new(3, 10).unwrap();
        let result = index.add(1, &[1.0, 0.0]); // wrong dim
        assert!(result.is_err());
    }

    #[test]
    fn test_hnsw_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.usearch");

        {
            let index = HnswIndex::new(3, 100).unwrap();
            index.add(1, &[1.0, 0.0, 0.0]).unwrap();
            index.add(2, &[0.0, 1.0, 0.0]).unwrap();
            index.save(&path).unwrap();
        }

        let loaded = HnswIndex::load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.dimensions(), 3);

        let results = loaded.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn test_hnsw_contains() {
        let index = HnswIndex::new(3, 10).unwrap();
        assert!(!index.contains(1));

        index.add(1, &[1.0, 0.0, 0.0]).unwrap();
        assert!(index.contains(1));
        assert!(!index.contains(2));
    }

    #[test]
    fn test_hnsw_remove() {
        let index = HnswIndex::new(3, 100).unwrap();
        index.add(1, &[1.0, 0.0, 0.0]).unwrap();
        assert_eq!(index.len(), 1);

        index.remove(1).unwrap();
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_hnsw_empty_search() {
        let index = HnswIndex::new(3, 10).unwrap();
        let results = index.search(&[1.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_hnsw_with_default_dims() {
        let index = HnswIndex::with_default_dims(100).unwrap();
        assert_eq!(index.dimensions(), 1536);
    }
}
