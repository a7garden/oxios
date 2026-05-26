#![allow(missing_docs)]
//! ROOT index — the "table of contents" for all agent knowledge.
//!
//! Provides O(1) topic lookup so agents can quickly understand what they know.
//! Automatically maintained by the Dream process; users never interact with it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{MemoryType, ProtectionLevel};

// ---------------------------------------------------------------------------
// RootIndex
// ---------------------------------------------------------------------------

/// ROOT index — the "table of contents" for all agent knowledge.
///
/// Agents use this to understand what they know at a glance (O(1) lookup).
/// Dream automatically rebuilds this on every run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootIndex {
    /// Index version (incremented on each dream).
    pub version: u64,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Active context entries (recent ~7 days).
    pub active_context: Vec<RootEntry>,
    /// Recent patterns observed across sessions.
    pub recent_patterns: Vec<String>,
    /// Historical summary (monthly breakdowns).
    pub historical_summary: Vec<HistoricalPeriod>,
    /// Topic index — all known topics with type and freshness.
    pub topics: Vec<TopicEntry>,
}

impl Default for RootIndex {
    fn default() -> Self {
        Self {
            version: 0,
            updated_at: Utc::now(),
            active_context: Vec::new(),
            recent_patterns: Vec::new(),
            historical_summary: Vec::new(),
            topics: Vec::new(),
        }
    }
}

impl RootIndex {
    /// Create a new empty ROOT index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Estimate token count for this index (4 chars ≈ 1 token).
    pub fn estimated_tokens(&self) -> usize {
        let total_chars: usize = self
            .active_context
            .iter()
            .map(|e| e.topic.len() + e.reference.len())
            .chain(self.recent_patterns.iter().map(|p| p.len()))
            .chain(self.topics.iter().map(|t| t.name.len() + t.description.len()))
            .sum();
        total_chars / 4
    }

    /// Check if a topic matches a query string.
    pub fn topic_matches_query(&self, topic: &TopicEntry, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        topic.name.to_lowercase().contains(&query_lower)
            || topic.description.to_lowercase().contains(&query_lower)
            || topic.category.to_lowercase().contains(&query_lower)
    }
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A single entry in the ROOT index's active context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootEntry {
    /// Topic name.
    pub topic: String,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Protection level.
    pub protection: ProtectionLevel,
    /// Age in days.
    pub age_days: u32,
    /// Reference (memory entry ID or file path).
    pub reference: String,
}

/// A historical period summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalPeriod {
    /// Period label (e.g., "2026-05").
    pub period: String,
    /// Summary of activities in this period.
    pub summary: String,
}

/// A topic entry in the ROOT index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicEntry {
    /// Topic name.
    pub name: String,
    /// Category (e.g., "project", "preference", "decision").
    pub category: String,
    /// Age in days.
    pub age_days: u32,
    /// Brief description.
    pub description: String,
    /// Reference (memory entry ID).
    pub reference: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_index_default() {
        let idx = RootIndex::default();
        assert_eq!(idx.version, 0);
        assert!(idx.active_context.is_empty());
        assert!(idx.topics.is_empty());
    }

    #[test]
    fn test_estimated_tokens() {
        let mut idx = RootIndex::new();
        idx.topics.push(TopicEntry {
            name: "Rust async runtime".to_string(),
            category: "project".to_string(),
            age_days: 5,
            description: "Using Tokio for async".to_string(),
            reference: "fact-123".to_string(),
        });
        let tokens = idx.estimated_tokens();
        assert!(tokens > 0, "Should have some estimated tokens");
    }

    #[test]
    fn test_topic_matches_query() {
        let idx = RootIndex::new();
        let topic = TopicEntry {
            name: "Memory consolidation".to_string(),
            category: "architecture".to_string(),
            age_days: 3,
            description: "RFC-008 tiered memory system".to_string(),
            reference: "dec-456".to_string(),
        };
        assert!(idx.topic_matches_query(&topic, "memory"));
        assert!(idx.topic_matches_query(&topic, "consolidation"));
        assert!(idx.topic_matches_query(&topic, "architecture"));
        assert!(!idx.topic_matches_query(&topic, "deployment"));
    }
}
