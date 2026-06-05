#![allow(missing_docs)]
//! Unified search: sqlite-vec KNN + FTS5 BM25 → Reciprocal Rank Fusion.
//!
//! This is the primary search API for the SQLite-backed memory system.
//! It runs both vector (semantic) and keyword (BM25) search in parallel
//! and fuses results using RRF for optimal recall.

pub(super) mod bm25;
mod rrf;
pub(super) mod vector;

use anyhow::Result;

use super::database::MemoryDatabase;
use crate::memory::types::{MemoryEntry, MemoryTier, MemoryType, ProtectionLevel};

// Re-export for external use
pub use bm25::Bm25Hit;
pub use rrf::reciprocal_rank_fusion;
pub use vector::VectorHit;

/// A ranked memory result from unified search.
#[derive(Debug, Clone)]
pub struct RankedMemory {
    /// The memory entry.
    pub entry: MemoryEntry,
    /// Combined RRF score (higher = more relevant).
    pub score: f64,
}

/// Execute unified search over memories.
///
/// Combines:
/// 1. **Vector KNN** (sqlite-vec) — semantic similarity via dense embeddings
/// 2. **BM25** (FTS5) — keyword relevance
/// 3. **RRF** — fusion of both result sets
///
/// # Arguments
/// * `db` — SQLite memory database.
/// * `query_vector` — Optional dense embedding for semantic search.
/// * `query_text` — Text query for BM25 keyword search.
/// * `memory_type` — Optional filter by memory type.
/// * `limit` — Maximum results to return.
///
/// # Returns
/// Fused results sorted by combined RRF score.
pub fn search(
    db: &MemoryDatabase,
    query_vector: Option<&[f32]>,
    query_text: &str,
    memory_type: Option<MemoryType>,
    limit: usize,
) -> Result<Vec<RankedMemory>> {
    let fetch_limit = limit * 2; // Over-fetch before type filtering

    let mut tier_results: Vec<Vec<(i64, f64)>> = Vec::new();

    // ── Tier 1: sqlite-vec Dense KNN ──
    if let Some(query_vec) = query_vector {
        match vector::search_vector(db, query_vec, fetch_limit) {
            Ok(hits) => {
                let tier: Vec<(i64, f64)> =
                    hits.into_iter().map(|h| (h.rowid, h.distance)).collect();
                if !tier.is_empty() {
                    tier_results.push(tier);
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "Vector search failed, skipping tier");
            }
        }
    }

    // ── Tier 2: FTS5 BM25 ──
    match bm25::search_bm25(db, query_text, fetch_limit) {
        Ok(hits) => {
            let tier: Vec<(i64, f64)> = hits.into_iter().map(|h| (h.rowid, h.score)).collect();
            if !tier.is_empty() {
                tier_results.push(tier);
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "BM25 search failed, skipping tier");
        }
    }

    // ── RRF Fusion ──
    let fused = reciprocal_rank_fusion(tier_results, 60.0);

    // ── Load memory entries by rowid ──
    let mut results = Vec::new();
    for (rowid, score) in fused.into_iter().take(limit) {
        if let Some(entry) = load_memory_by_rowid(db, rowid)? {
            // Apply type filter if specified
            if let Some(ref mt) = memory_type {
                if entry.memory_type != *mt {
                    continue;
                }
            }
            results.push(RankedMemory { entry, score });
        }
    }

    Ok(results)
}

/// Load a memory entry by its SQLite rowid.
fn load_memory_by_rowid(db: &MemoryDatabase, rowid: i64) -> Result<Option<MemoryEntry>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, memory_type, content, importance, tier, protection,
                source, session_id, tags, access_count, pinned,
                auto_classified, session_appearances, decay_score, content_hash,
                created_at, updated_at, accessed_at
         FROM memories WHERE rowid = ?1",
    )?;

    let mut rows = stmt.query(rusqlite::params![rowid])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_memory_entry(row))),
        None => Ok(None),
    }
}

/// Load a memory entry by its string ID.
pub fn load_memory_by_id(db: &MemoryDatabase, id: &str) -> Result<Option<MemoryEntry>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, memory_type, content, importance, tier, protection,
                source, session_id, tags, access_count, pinned,
                auto_classified, session_appearances, decay_score, content_hash,
                created_at, updated_at, accessed_at
         FROM memories WHERE id = ?1",
    )?;

    let mut rows = stmt.query(rusqlite::params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_memory_entry(row))),
        None => Ok(None),
    }
}

/// Map a SQLite row to a MemoryEntry.
///
/// Column order must match the SELECT statement above:
///  0: id, 1: memory_type, 2: content, 3: importance, 4: tier, 5: protection,
///  6: source, 7: session_id, 8: tags, 9: access_count, 10: pinned,
/// 11: auto_classified, 12: session_appearances, 13: decay_score, 14: content_hash,
/// 15: created_at, 16: updated_at, 17: accessed_at
pub fn row_to_memory_entry(row: &rusqlite::Row<'_>) -> MemoryEntry {
    use chrono::Utc;

    let memory_type_str: String = row.get_unwrap(1);
    let tier_str: String = row.get_unwrap(4);
    let protection_str: String = row.get_unwrap(5);
    let tags_str: Option<String> = row.get_unwrap(8);
    let created_at_str: String = row.get_unwrap(15);
    let updated_at_str: String = row.get_unwrap(16);
    let accessed_at_str: Option<String> = row.get_unwrap(17);

    MemoryEntry {
        id: row.get_unwrap(0),
        memory_type: parse_memory_type(&memory_type_str),
        content: row.get_unwrap(2),
        importance: row.get_unwrap(3),
        tier: parse_tier(&tier_str),
        protection: parse_protection(&protection_str),
        source: row.get_unwrap(6),
        session_id: row.get_unwrap(7),
        tags: tags_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        content_hash: row.get::<_, i64>(14).unwrap() as u64,
        pinned: row.get::<_, i64>(10).unwrap() != 0,
        auto_classified: row.get::<_, i64>(11).unwrap() != 0,
        session_appearances: row.get::<_, i64>(12).unwrap() as u32,
        user_corrected: false,
        seen_in_sessions: vec![],
        created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
        accessed_at: accessed_at_str
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        modified_at: updated_at_str.parse().unwrap_or_else(|_| Utc::now()),
        access_count: row.get::<_, i64>(10).unwrap() as u32,
        decay_score: row.get_unwrap(14),
        compaction_level: 0,
        compacted_from: vec![],
        related_ids: vec![],
        contradicts: None,
    }
}

fn parse_memory_type(s: &str) -> MemoryType {
    match s {
        "conversation" => MemoryType::Conversation,
        "session" => MemoryType::Session,
        "fact" => MemoryType::Fact,
        "episode" => MemoryType::Episode,
        "knowledge" => MemoryType::Knowledge,
        "skill" => MemoryType::Skill,
        "preference" => MemoryType::Preference,
        "decision" => MemoryType::Decision,
        "user_profile" => MemoryType::UserProfile,
        _ => MemoryType::Fact,
    }
}

fn parse_tier(s: &str) -> MemoryTier {
    match s {
        "hot" => MemoryTier::Hot,
        "cold" => MemoryTier::Cold,
        _ => MemoryTier::Warm,
    }
}

fn parse_protection(s: &str) -> ProtectionLevel {
    match s {
        "none" => ProtectionLevel::None,
        "low" => ProtectionLevel::Low,
        "medium" => ProtectionLevel::Medium,
        "high" => ProtectionLevel::High,
        "permanent" => ProtectionLevel::Permanent,
        _ => ProtectionLevel::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::database::MemoryDatabase;

    #[test]
    fn test_search_with_bm25_only() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('search-1', 'fact', 'Rust programming language', 0.6, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('search-2', 'fact', 'Python data science', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let results = search(&db, None, "Rust", None, 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].entry.id, "search-1");
    }

    #[test]
    fn test_search_with_type_filter() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('filter-1', 'fact', 'test content', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('filter-2', 'episode', 'test content episode', 0.5, 'warm', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let results = search(&db, None, "test", Some(MemoryType::Fact), 10).unwrap();
        assert!(results
            .iter()
            .all(|r| r.entry.memory_type == MemoryType::Fact));
    }

    #[test]
    fn test_search_empty() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        let results = search(&db, None, "nothing", None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_load_memory_by_id() {
        let db = MemoryDatabase::open_in_memory(256).unwrap();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, source, created_at, updated_at)
                 VALUES ('load-test', 'fact', 'load this', 0.7, 'hot', 'test', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let entry = load_memory_by_id(&db, "load-test").unwrap();
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.id, "load-test");
        assert_eq!(entry.content, "load this");
        assert_eq!(entry.memory_type, MemoryType::Fact);
        assert_eq!(entry.tier, MemoryTier::Hot);
    }
}
