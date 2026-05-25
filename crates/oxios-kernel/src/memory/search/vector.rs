//! sqlite-vec vector KNN search over memory embeddings.
//!
//! Uses brute-force KNN via the `vec0` virtual table for dense vector
//! similarity search. Adequate for Oxios memory scale (~10K entries).

use anyhow::Result;

use super::super::database::{bytes_to_f32_slice, f32_slice_to_bytes, MemoryDatabase};

/// A single vector search hit.
#[derive(Debug, Clone)]
pub struct VectorHit {
    /// Memory rowid (SQLite internal).
    pub rowid: i64,
    /// Cosine distance (lower = more similar).
    pub distance: f64,
}

/// Execute a KNN vector search against the sqlite-vec index.
///
/// # Arguments
/// * `db` — Memory database.
/// * `query_vector` — Dense f32 query vector.
/// * `limit` — Maximum results.
///
/// # Returns
/// Hits sorted by distance ascending (most similar first).
pub fn search_vector(
    db: &MemoryDatabase,
    query_vector: &[f32],
    limit: usize,
) -> Result<Vec<VectorHit>> {
    if query_vector.is_empty() {
        return Ok(Vec::new());
    }

    let conn = db.conn();
    let query_bytes = f32_slice_to_bytes(query_vector);

    let sql = "SELECT rowid, distance
               FROM memory_vectors
               WHERE embedding MATCH ?1
               ORDER BY distance
               LIMIT ?2";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params![query_bytes, limit], |row| {
        Ok(VectorHit {
            rowid: row.get(0)?,
            distance: row.get(1)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }

    Ok(results)
}

/// Insert a vector embedding for a memory rowid.
pub fn insert_vector(db: &MemoryDatabase, rowid: i64, vector: &[f32]) -> Result<()> {
    let conn = db.conn();
    let vec_bytes = f32_slice_to_bytes(vector);
    conn.execute(
        "INSERT INTO memory_vectors (rowid, embedding) VALUES (?1, ?2)",
        rusqlite::params![rowid, vec_bytes],
    )?;
    Ok(())
}

/// Delete a vector embedding by rowid.
pub fn delete_vector(db: &MemoryDatabase, rowid: i64) -> Result<()> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM memory_vectors WHERE rowid = ?1",
        rusqlite::params![rowid],
    )?;
    Ok(())
}

/// Count vectors in the index.
pub fn vector_count(db: &MemoryDatabase) -> Result<usize> {
    let conn = db.conn();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_vectors",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_vector(dim: usize, seed: f32) -> Vec<f32> {
        (0..dim).map(|i| (seed + i as f32 * 0.01).sin()).collect()
    }

    #[test]
    fn test_vector_insert_and_search() {
        let db = MemoryDatabase::open_in_memory(8).unwrap();
        let conn = db.conn();

        // Insert a memory to get a rowid
        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('vec-1', 'fact', 'test', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        let rowid: i64 = conn.last_insert_rowid();

        let v = make_test_vector(8, 1.0);
        insert_vector(&db, rowid, &v).unwrap();
        assert_eq!(vector_count(&db).unwrap(), 1);

        // Search with the same vector → should find it with distance ~0
        let results = search_vector(&db, &v, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].rowid, rowid);
        assert!(results[0].distance < 0.01, "Distance should be near 0, got {}", results[0].distance);
    }

    #[test]
    fn test_vector_knn_ordering() {
        let db = MemoryDatabase::open_in_memory(4).unwrap();
        let conn = db.conn();

        // Insert 3 memories with different vectors
        let v1 = vec![1.0, 0.0, 0.0, 0.0f32];
        let v2 = vec![0.9, 0.1, 0.0, 0.0f32];
        let v3 = vec![0.0, 0.0, 1.0, 0.0f32];

        for (i, v) in [&v1, &v2, &v3].iter().enumerate() {
            conn.execute(
                &format!(
                    "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                     VALUES ('knn-{}', 'fact', 'test', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    i
                ),
                [],
            ).unwrap();
            let rowid = conn.last_insert_rowid();
            insert_vector(&db, rowid, v).unwrap();
        }

        // Query with v1 → should return v1 first, then v2 (close), then v3 (far)
        let results = search_vector(&db, &v1, 10).unwrap();
        assert_eq!(results.len(), 3);
        // First should be the v1 entry (distance ~0)
        assert!(results[0].distance < results[1].distance);
        assert!(results[1].distance < results[2].distance);
    }

    #[test]
    fn test_vector_delete() {
        let db = MemoryDatabase::open_in_memory(4).unwrap();
        let conn = db.conn();

        conn.execute(
            "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
             VALUES ('del-1', 'fact', 'test', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        let rowid = conn.last_insert_rowid();

        let v = vec![1.0, 0.0, 0.0, 0.0f32];
        insert_vector(&db, rowid, &v).unwrap();
        assert_eq!(vector_count(&db).unwrap(), 1);

        delete_vector(&db, rowid).unwrap();
        assert_eq!(vector_count(&db).unwrap(), 0);
    }

    #[test]
    fn test_vector_empty_search() {
        let db = MemoryDatabase::open_in_memory(4).unwrap();
        let v = vec![1.0, 0.0, 0.0, 0.0f32];
        let results = search_vector(&db, &v, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_f32_bytes_roundtrip() {
        let original: Vec<f32> = vec![0.1, -0.5, 42.0, 0.0, 1.5];
        let bytes = f32_slice_to_bytes(&original);
        let restored = bytes_to_f32_slice(&bytes);
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }
}
