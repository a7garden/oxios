//! Embedding cache for reducing API calls.
//!
//! Provides LRU cache with TTL for embedding vectors.
//!
//! # Example
//!
//! ```
//! use oxios_kernel::memory::EmbeddingCache;
//!
//! let cache = EmbeddingCache::new(3600, 10000);  // 1 hour TTL, 10k max
//! cache.insert("hello", vec![1.0, 2.0, 3.0]);
//! let embedded = cache.get("hello");
//! assert!(embedded.is_some());
//! ```

use lru::LruCache;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// Cache entry with TTL tracking.
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    ttl: Duration,
}

impl<V> CacheEntry<V> {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Content-addressable embedding cache with TTL and LRU eviction.
pub struct EmbeddingCache {
    inner: RwLock<LruCache<u64, CacheEntry<Vec<f32>>>>,
    ttl: Duration,
    max_entries: usize,
    hits: RwLock<u64>,
    misses: RwLock<u64>,
}

/// Cache statistics for monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Hit rate as a fraction (0.0 to 1.0).
    pub hit_rate: f64,
    /// Current number of entries in cache.
    pub size: usize,
    /// Maximum capacity of cache.
    pub capacity: usize,
}

impl EmbeddingCache {
    /// Create a new cache with TTL and capacity.
    ///
    /// # Arguments
    /// * `ttl_secs` - Time-to-live for cached entries in seconds
    /// * `max_entries` - Maximum number of entries to cache
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            inner: RwLock::new(LruCache::new(
                std::num::NonZeroUsize::new(max_entries).unwrap_or(std::num::NonZeroUsize::MIN),
            )),
            ttl: Duration::from_secs(ttl_secs),
            max_entries,
            hits: RwLock::new(0),
            misses: RwLock::new(0),
        }
    }

    /// Hash content to cache key.
    pub fn content_hash(content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Get cached embedding if exists and not expired.
    pub fn get(&self, content: &str) -> Option<Vec<f32>> {
        let key = Self::content_hash(content);
        let mut inner = self.inner.write();

        match inner.get(&key) {
            Some(entry) if !entry.is_expired() => {
                *self.hits.write() += 1;
                Some(entry.value.clone())
            }
            Some(_) => {
                // Expired — remove
                inner.pop(&key);
                *self.misses.write() += 1;
                None
            }
            None => {
                *self.misses.write() += 1;
                None
            }
        }
    }

    /// Cache an embedding.
    pub fn insert(&self, content: &str, embedding: Vec<f32>) {
        let key = Self::content_hash(content);
        let mut inner = self.inner.write();

        inner.push(
            key,
            CacheEntry {
                value: embedding,
                created_at: Instant::now(),
                ttl: self.ttl,
            },
        );
    }

    /// Evict expired entries.
    ///
    /// Returns the number of entries evicted.
    pub fn evict_expired(&self) -> usize {
        let mut inner = self.inner.write();
        let mut evicted = 0;

        let keys: Vec<_> = inner
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(k, _)| *k)
            .collect();

        for key in keys {
            inner.pop(&key);
            evicted += 1;
        }

        evicted
    }

    /// Evict least recently used entries to free space.
    ///
    /// Returns the number of entries evicted.
    pub fn evict_lru(&self, target_size: usize) -> usize {
        let mut inner = self.inner.write();
        let mut evicted = 0;

        while inner.len() > target_size {
            if inner.pop_lru().is_none() {
                break;
            }
            evicted += 1;
        }

        evicted
    }

    /// Cache statistics.
    pub fn stats(&self) -> CacheStats {
        let hits = *self.hits.read();
        let misses = *self.misses.read();
        let total = hits + misses;

        CacheStats {
            hits,
            misses,
            hit_rate: if total > 0 {
                hits as f64 / total as f64
            } else {
                0.0
            },
            size: self.inner.read().len(),
            capacity: self.max_entries,
        }
    }

    /// Clear the cache.
    pub fn clear(&self) {
        self.inner.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_cache_basic() {
        let cache = EmbeddingCache::new(60, 100);

        // Insert
        cache.insert("hello", vec![1.0, 2.0, 3.0]);

        // Get
        let result = cache.get("hello");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![1.0, 2.0, 3.0]);

        // Stats
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_cache_miss() {
        let cache = EmbeddingCache::new(60, 100);

        let result = cache.get("nonexistent");
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_cache_ttl() {
        let cache = EmbeddingCache::new(1, 100); // 1 second TTL

        cache.insert("test", vec![1.0]);
        assert!(cache.get("test").is_some());

        // Wait for expiration
        thread::sleep(Duration::from_secs(2));

        // Should be expired
        assert!(cache.get("test").is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = EmbeddingCache::new(60, 2);

        cache.insert("a", vec![1.0]);
        cache.insert("b", vec![2.0]);
        cache.insert("c", vec![3.0]); // Should evict oldest

        // a should be evicted
        assert!(cache.get("a").is_none());

        // b and c should exist
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
    }
}
