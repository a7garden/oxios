//! Proactive recall — automatically inject relevant memories into context.
//!
//! Implements 3-step selective recall:
//! 1. ROOT index triage (O(1) topic lookup)
//! 2. Manifest-based selection (keyword matching)
//! 3. HNSW vector search (semantic similarity)
//!
//! Only triggered at session start and topic transitions to avoid context bloat.

use std::collections::HashSet;

use anyhow::Result;

use super::{MemoryEntry, MemoryManager};

// ---------------------------------------------------------------------------
// RecallTiming
// ---------------------------------------------------------------------------

/// Tracks when proactive recall should be triggered.
#[derive(Debug, Clone, Default)]
pub struct RecallTiming {
    /// Last topic that triggered a recall.
    pub last_recall_topic: Option<String>,
    /// Messages since last recall.
    pub message_count_since_recall: usize,
}

impl RecallTiming {
    /// Create a new timing tracker.
    pub fn new() -> Self {
        Self {
            last_recall_topic: None,
            message_count_since_recall: 0,
        }
    }

    /// Check if proactive recall should fire for the given query.
    ///
    /// Triggers on:
    /// - Session first message (count == 0)
    /// - Topic change (after at least 3 messages)
    /// - Periodic (every 10 messages)
    pub fn should_recall(&mut self, query: &str) -> bool {
        let topic_changed = self
            .last_recall_topic
            .as_ref()
            .is_none_or(|prev| !topics_similar(prev, query));

        let should = self.message_count_since_recall == 0 // First message
            || (topic_changed && self.message_count_since_recall >= 3) // Topic change
            || self.message_count_since_recall >= 10; // Periodic

        if should {
            self.last_recall_topic = Some(query.to_string());
            self.message_count_since_recall = 0;
        } else {
            self.message_count_since_recall += 1;
        }
        should
    }
}

/// Simple topic similarity check (keyword overlap).
fn topics_similar(a: &str, b: &str) -> bool {
    let a_words: HashSet<String> = a
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .map(|w| w.to_string())
        .collect();
    let b_words: HashSet<String> = b
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .map(|w| w.to_string())
        .collect();

    if a_words.is_empty() || b_words.is_empty() {
        return false;
    }

    let overlap = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    // Jaccard similarity > 0.3
    overlap as f32 / union as f32 > 0.3
}

// ---------------------------------------------------------------------------
// ProactiveRecall
// ---------------------------------------------------------------------------

/// Proactive recall engine.
///
/// Combines ROOT index triage, manifest-based selection, and HNSW
/// semantic search to find relevant memories for the current context.
pub struct ProactiveRecall {
    /// Maximum results to return.
    pub limit: usize,
    /// Minimum effective importance threshold.
    pub threshold: f32,
}

impl ProactiveRecall {
    /// Create with the given limit and threshold.
    pub fn new(limit: usize, threshold: f32) -> Self {
        Self { limit, threshold }
    }

    /// Execute 3-step proactive recall.
    ///
    /// Steps:
    /// 1. HOT tier memories (always injected for context)
    /// 2. ROOT index triage (O(1) topic lookup)
    /// 3. SQLite semantic + BM25 search (vector + keyword fusion)
    pub async fn recall(
        &self,
        mgr: &MemoryManager,
        query: &str,
        current_context: &[MemoryEntry],
    ) -> Result<Vec<MemoryEntry>> {
        let mut results = Vec::new();
        let mut seen_ids: HashSet<String> = current_context.iter().map(|e| e.id.clone()).collect();

        // Step 1: HOT tier memories (always included)
        if let Ok(hot_entries) = mgr
            .list_by_tier(crate::memory::MemoryTier::Hot, self.limit)
            .await
        {
            for entry in hot_entries {
                if !seen_ids.contains(&entry.id) {
                    seen_ids.insert(entry.id.clone());
                    results.push(entry);
                }
            }
        }

        // Step 2: SQLite semantic + BM25 search
        if results.len() < self.limit {
            let remaining = self.limit - results.len();
            let search_results = mgr
                .search(query, None, remaining * 2)
                .await
                .unwrap_or_default();
            for entry in search_results {
                if !seen_ids.contains(&entry.id) {
                    seen_ids.insert(entry.id.clone());
                    results.push(entry);
                }
                if results.len() >= self.limit {
                    break;
                }
            }
        }

        // Filter by importance threshold
        results.retain(|e| {
            crate::memory::decay::DecayEngine::effective_importance(e) >= self.threshold
        });

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recall_timing_first_message() {
        let mut timing = RecallTiming::new();
        assert!(timing.should_recall("hello"));
    }

    #[test]
    fn test_recall_timing_topic_change() {
        let mut timing = RecallTiming::new();
        timing.should_recall("rust programming");
        timing.message_count_since_recall = 5;
        assert!(timing.should_recall("python deployment"));
    }

    #[test]
    fn test_recall_timing_same_topic() {
        let mut timing = RecallTiming::new();
        timing.should_recall("rust async runtime");
        timing.message_count_since_recall = 1;
        assert!(!timing.should_recall("rust async tokio"));
    }

    #[test]
    fn test_recall_timing_periodic() {
        let mut timing = RecallTiming::new();
        timing.should_recall("rust");
        timing.message_count_since_recall = 10;
        assert!(timing.should_recall("rust continued"));
    }

    #[test]
    fn test_topics_similar_same() {
        assert!(topics_similar("rust async runtime", "rust async runtime"));
    }

    #[test]
    fn test_topics_similar_overlap() {
        assert!(topics_similar(
            "rust async runtime tokio",
            "rust async runtime futures"
        ));
    }

    #[test]
    fn test_topics_different() {
        assert!(!topics_similar("rust async runtime", "python data science"));
    }
}
