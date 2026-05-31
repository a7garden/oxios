//! Compaction tree — 5-level memory compression hierarchy.
//!
//! Raw → Daily → Weekly → Monthly → Root
//!
//! Older memories are progressively compressed into higher-level summaries,
//! preserving key information while reducing storage and context size.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CompactionLevel
// ---------------------------------------------------------------------------

/// Compaction level in the compression hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum CompactionLevel {
    /// Raw session data (uncompressed).
    Raw = 0,
    /// Daily summary (compressed from raw).
    Daily = 1,
    /// Weekly summary (compressed from daily).
    Weekly = 2,
    /// Monthly summary (compressed from weekly).
    Monthly = 3,
    /// Root index entry (top-level summary).
    Root = 4,
}

impl CompactionLevel {
    /// Line count threshold for triggering compaction at this level.
    #[allow(dead_code)]
    pub fn threshold(&self) -> usize {
        match self {
            CompactionLevel::Raw => 200,
            CompactionLevel::Daily => 300,
            CompactionLevel::Weekly => 500,
            CompactionLevel::Monthly => usize::MAX,
            CompactionLevel::Root => usize::MAX,
        }
    }

    /// Storage subdirectory name for this level.
    #[allow(dead_code)]
    pub fn dir_name(&self) -> &'static str {
        match self {
            CompactionLevel::Raw => "raw",
            CompactionLevel::Daily => "daily",
            CompactionLevel::Weekly => "weekly",
            CompactionLevel::Monthly => "monthly",
            CompactionLevel::Root => "root",
        }
    }

    /// All levels in order.
    #[allow(dead_code)]
    pub fn all() -> &'static [CompactionLevel] {
        &[
            CompactionLevel::Raw,
            CompactionLevel::Daily,
            CompactionLevel::Weekly,
            CompactionLevel::Monthly,
            CompactionLevel::Root,
        ]
    }

    /// Get the next higher compaction level.
    #[allow(dead_code)]
    pub fn next(&self) -> Option<CompactionLevel> {
        match self {
            CompactionLevel::Raw => Some(CompactionLevel::Daily),
            CompactionLevel::Daily => Some(CompactionLevel::Weekly),
            CompactionLevel::Weekly => Some(CompactionLevel::Monthly),
            CompactionLevel::Monthly => Some(CompactionLevel::Root),
            CompactionLevel::Root => None,
        }
    }
}

// ---------------------------------------------------------------------------
// CompactionTree
// ---------------------------------------------------------------------------

/// Compaction tree manager.
///
/// Manages the 5-level compression hierarchy. Dream calls `compact()` to
/// promote entries up the tree when they exceed size thresholds.
pub struct CompactionTree {
    /// Line count threshold for triggering compaction.
    pub line_threshold: usize,
}

impl CompactionTree {
    /// Create a new compaction tree with the given line threshold.
    pub fn new(line_threshold: usize) -> Self {
        Self { line_threshold }
    }

    /// Create with default threshold.
    pub fn default_tree() -> Self {
        Self::new(200)
    }

    /// Check if content should be compacted based on line count.
    pub fn should_compact(&self, content: &str) -> bool {
        content.lines().count() >= self.line_threshold
    }

    /// Simple rule-based compaction: preserve first/last sentences of each
    /// paragraph, discard middle.
    ///
    /// This is the fallback when LLM compaction is not available.
    pub fn rule_based_compact(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < 5 {
            return content.to_string();
        }

        let mut compacted: Vec<String> = Vec::new();

        // Preserve first 2 lines
        compacted.extend(lines.iter().take(2).map(|l| l.to_string()));

        // If long, add a summary indicator
        if lines.len() > 10 {
            compacted.push(format!("... ({} lines omitted) ...", lines.len() - 4));
        }

        // Preserve last 2 lines
        let tail: Vec<String> = lines.iter().rev().take(2).map(|l| l.to_string()).collect();
        compacted.extend(tail.into_iter().rev());

        compacted.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_level_next() {
        assert_eq!(CompactionLevel::Raw.next(), Some(CompactionLevel::Daily));
        assert_eq!(CompactionLevel::Daily.next(), Some(CompactionLevel::Weekly));
        assert_eq!(
            CompactionLevel::Weekly.next(),
            Some(CompactionLevel::Monthly)
        );
        assert_eq!(CompactionLevel::Monthly.next(), Some(CompactionLevel::Root));
        assert_eq!(CompactionLevel::Root.next(), None);
    }

    #[test]
    fn test_should_compact_short() {
        let tree = CompactionTree::new(10);
        let content = "line 1\nline 2\nline 3";
        assert!(!tree.should_compact(content));
    }

    #[test]
    fn test_should_compact_long() {
        let tree = CompactionTree::new(5);
        let content = (0..10)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(tree.should_compact(&content));
    }

    #[test]
    fn test_rule_based_compact_short() {
        let tree = CompactionTree::new(10);
        let content = "line 1\nline 2\nline 3";
        let result = tree.rule_based_compact(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_rule_based_compact_long() {
        let tree = CompactionTree::new(10);
        let content = (0..20)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = tree.rule_based_compact(&content);
        assert!(result.lines().count() < 20, "Should be compacted");
        assert!(result.contains("line 0"), "Should preserve first line");
        assert!(result.contains("line 19"), "Should preserve last line");
        assert!(
            result.contains("omitted"),
            "Should indicate omitted content"
        );
    }
}
