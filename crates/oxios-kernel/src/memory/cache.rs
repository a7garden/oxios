//! SQLite-based embedding cache.
//!
//! Caches computed embeddings by content hash to avoid recomputation.
//! Stored in the same `memory.db` file for atomicity.

use anyhow::Result;

use super::database::{bytes_to_f32_slice, f32_slice_to_bytes, MemoryDatabase};
use crate::memory::content_hash;

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total cached entries.
    pub entries: usize,
    /// Cache hits since last stats reset.
    pub hits: u64,
    /// Cache misses since last stats reset.
    pub misses: u64,
}

/// Get a cached embedding for the given text.
///
/// Returns `Some(vector)` if a cached embedding exists, `None` otherwise.
pub fn get_cached(db: &MemoryDatabase, text: &str) -> Result<Option<Vec<f32>>> {
    let hash = format!("{:016x}", content_hash(text));
    let conn = db.conn();

    let result = conn
        .query_row(
            "SELECT embedding FROM embedding_cache WHERE content_hash = ?1",
            rusqlite::params![hash],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .ok();

    match result {
        Some(bytes) => Ok(Some(bytes_to_f32_slice(&bytes))),
        None => Ok(None),
    }
}

/// Store an embedding in the cache.
pub fn put_cached(db: &MemoryDatabase, text: &str, vector: &[f32]) -> Result<()> {
    let hash = format!("{:016x}", content_hash(text));
    let bytes = f32_slice_to_bytes(vector);
    let conn = db.conn();

    conn.execute(
        "INSERT OR REPLACE INTO embedding_cache (content_hash, embedding, created_at)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![hash, bytes, chrono::Utc::now().to_rfc3339()],
    )?;

    Ok(())
}

/// Get or compute an embedding, using the cache.
///
/// If the embedding is cached, returns it directly.
/// Otherwise, calls `compute_fn`, caches the result, and returns it.
pub async fn get_or_compute<F, Fut>(
    db: &MemoryDatabase,
    text: &str,
    compute_fn: F,
) -> Result<Vec<f32>>
where
    F: FnOnce(&str) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<f32>>>,
{
    // Check cache first
    if let Some(cached) = get_cached(db, text)? {
        tracing::debug!(len = text.len(), "Embedding cache hit");
        return Ok(cached);
    }

    // Compute and cache
    let vector = compute_fn(text).await?;
    if let Err(e) = put_cached(db, text, &vector) {
        tracing::debug!(error = %e, "Failed to cache embedding (non-fatal)");
    }

    Ok(vector)
}

/// Get cache statistics.
pub fn stats(db: &MemoryDatabase) -> Result<CacheStats> {
    let conn = db.conn();
    let entries: i64 = conn.query_row(
        "SELECT COUNT(*) FROM embedding_cache",
        [],
        |row| row.get(0),
    )?;
    Ok(CacheStats {
        entries: entries as usize,
        ..Default::default()
    })
}

/// Clear all cached embeddings.
pub fn clear(db: &MemoryDatabase) -> Result<usize> {
    let conn = db.conn();
    let count = conn.execute("DELETE FROM embedding_cache", [])?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_and_get() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        let vector = vec![0.1, 0.2, 0.3, 0.4];
        put_cached(&db, "test text", &vector).unwrap();

        let cached = get_cached(&db, "test text").unwrap();
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.len(), 4);
        for (a, b) in vector.iter().zip(cached.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_cache_miss() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let result = get_cached(&db, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_overwrite() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        put_cached(&db, "same text", &[0.1, 0.2]).unwrap();
        put_cached(&db, "same text", &[0.3, 0.4]).unwrap();

        let cached = get_cached(&db, "same text").unwrap().unwrap();
        assert!((cached[0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn test_cache_stats() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        assert_eq!(stats(&db).unwrap().entries, 0);
        put_cached(&db, "text1", &[0.1]).unwrap();
        put_cached(&db, "text2", &[0.2]).unwrap();
        assert_eq!(stats(&db).unwrap().entries, 2);
    }

    #[test]
    fn test_cache_clear() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        put_cached(&db, "text1", &[0.1]).unwrap();
        assert_eq!(stats(&db).unwrap().entries, 1);

        let cleared = clear(&db).unwrap();
        assert_eq!(cleared, 1);
        assert_eq!(stats(&db).unwrap().entries, 0);
    }

    #[tokio::test]
    async fn test_get_or_compute() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let result = get_or_compute(&db, "test input", move |_text| {
            let c = count_clone.clone();
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(vec![0.5, 0.6, 10.0])
            }
        })
        .await
        .unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Second call should hit cache
        let count_clone2 = call_count.clone();
        let result2 = get_or_compute(&db, "test input", move |_text| {
            let c = count_clone2.clone();
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(vec![0.5, 0.6, 10.0])
            }
        })
        .await
        .unwrap();

        assert_eq!(result2, result);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1, "Should have hit cache");
    }
}
