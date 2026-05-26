//! FTS5 BM25 full-text search over memory entries.
//!
//! Uses SQLite's built-in FTS5 with `unicode61` tokenizer for
//! language-agnostic text search, including CJK/Korean support.

use anyhow::Result;

use super::super::database::MemoryDatabase;

/// A single BM25 search hit.
#[derive(Debug, Clone)]
pub struct Bm25Hit {
    /// Memory rowid (SQLite internal).
    pub rowid: i64,
    /// Memory entry ID.
    pub id: String,
    /// BM25 relevance score (higher = more relevant).
    pub score: f64,
}

/// Execute a BM25 search against the FTS5 index.
///
/// # Arguments
/// * `db` — Memory database.
/// * `query` — FTS5 query string. Words are matched with OR logic by default.
///   Use quotes for exact phrases: `"exact phrase"`.
/// * `limit` — Maximum results.
///
/// # Returns
/// Hits sorted by BM25 score descending.
pub fn search_bm25(db: &MemoryDatabase, query: &str, limit: usize) -> Result<Vec<Bm25Hit>> {
    let conn = db.conn();

    // Sanitize query: FTS5 expects specific syntax
    // For safety, split on spaces and join with OR
    let fts_query = if query.contains('"') || query.contains("AND") || query.contains("OR") || query.contains("NOT") {
        // Advanced query — pass through
        query.to_string()
    } else {
        // Simple query — split into words, join with OR
        // CJK characters: split each character as a token
        // since unicode61 doesn't segment Korean/Chinese/Japanese
        let mut tokens = Vec::new();
        for word in query.split_whitespace() {
            if word.chars().all(|c| c.is_ascii_alphanumeric()) {
                if word.len() >= 2 {
                    tokens.push(word.to_string());
                }
            } else {
        // CJK or mixed: FTS5 unicode61 can't segment these,
        // so we just use the full word. If it's all CJK,
        // add each char separately for partial matching.
        let has_cjk = word.chars().any(|c| !c.is_ascii());
        if has_cjk {
            // Add individual CJK chars for char-by-char matching
            for ch in word.chars() {
                if !ch.is_ascii() && ch.is_alphabetic() {
                    tokens.push(ch.to_string());
                }
            }
        }
        if word.len() >= 2 {
            tokens.push(word.to_string());
        }
            }
        }
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        tokens.join(" OR ")
    };

    let sql = format!(
        "SELECT rowid, id, -bm25(memories_fts) as score
         FROM memories_fts
         WHERE memories_fts MATCH ?1
         ORDER BY score DESC
         LIMIT ?2"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(query = %fts_query, error = %e, "FTS5 query parse error");
            return Ok(Vec::new());
        }
    };
    let rows = match stmt.query_map(rusqlite::params![fts_query, limit], |row| {
        Ok(Bm25Hit {
            rowid: row.get(0)?,
            id: row.get(1)?,
            score: row.get(2)?,
        })
    }) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(query = %fts_query, error = %e, "FTS5 query execution error");
            return Ok(Vec::new());
        }
    };

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bm25_basic_search() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();

        // Insert test data (in a block to release the MutexGuard)
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('bm-test-1', 'fact', 'Rust is a systems programming language', 0.6, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('bm-test-2', 'fact', 'Python is great for data science', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let results = search_bm25(&db, "Rust programming", 10).unwrap();
        assert!(!results.is_empty(), "BM25 should find results");
        assert_eq!(results[0].id, "bm-test-1");
    }

    #[test]
    fn test_bm25_no_results() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let results = search_bm25(&db, "nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_korean() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('kr-bm-1', 'fact', '한국어 메모리 테스트입니다', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let results = search_bm25(&db, "한국어", 10).unwrap();
        assert!(!results.is_empty(), "Korean BM25 should find results");
    }

    #[test]
    fn test_bm25_limit() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        {
            let conn = db.conn();
            for i in 0..20 {
                conn.execute(
                    &format!(
                        "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                         VALUES ('limit-{}', 'fact', 'test content number {}', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                        i, i
                    ),
                    [],
                ).unwrap();
            }
        }

        let results = search_bm25(&db, "test content", 5).unwrap();
        assert!(results.len() <= 5, "Should respect limit");
    }
}
