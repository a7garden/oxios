//! Compaction tree — 5-level memory compression hierarchy.
//!
//! Raw → Daily → Weekly → Monthly → Root
//!
//! Older memories are progressively compressed into higher-level summaries,
//! preserving key information while reducing storage and context size.
//!
//! ## Levels
//!
//! | Level   | Value | Threshold | Description                        |
//! |---------|-------|-----------|------------------------------------|
//! | Raw     | 0     | 200 lines | Uncompressed session data          |
//! | Daily   | 1     | 300 lines | Compressed from raw (per-day)      |
//! | Weekly  | 2     | 500 lines | Compressed from daily (per-week)   |
//! | Monthly | 3     | ∞         | Compressed from weekly (per-month) |
//! | Root    | 4     | ∞         | Top-level index entry              |

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CompactionLevel
// ---------------------------------------------------------------------------

/// Compaction level in the compression hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CompactionLevel {
    /// Raw session data (uncompressed).
    Raw = 0,
    /// Daily summary (compressed from raw).
    Daily = 1,
    /// Weekly summary (compressed from weekly).
    Weekly = 2,
    /// Monthly summary (compressed from weekly).
    Monthly = 3,
    /// Root index entry (top-level summary).
    Root = 4,
}

impl CompactionLevel {
    /// Line count threshold for triggering compaction at this level.
    ///
    /// When content at this level exceeds the threshold, it becomes a
    /// candidate for promotion to the next level.
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
    pub fn dir_name(&self) -> &'static str {
        match self {
            CompactionLevel::Raw => "raw",
            CompactionLevel::Daily => "daily",
            CompactionLevel::Weekly => "weekly",
            CompactionLevel::Monthly => "monthly",
            CompactionLevel::Root => "root",
        }
    }

    /// All levels in ascending order.
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
    ///
    /// Returns `None` for `Root` (topmost level).
    pub fn next(&self) -> Option<CompactionLevel> {
        match self {
            CompactionLevel::Raw => Some(CompactionLevel::Daily),
            CompactionLevel::Daily => Some(CompactionLevel::Weekly),
            CompactionLevel::Weekly => Some(CompactionLevel::Monthly),
            CompactionLevel::Monthly => Some(CompactionLevel::Root),
            CompactionLevel::Root => None,
        }
    }

    /// Numeric value of this level.
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Try to convert a u8 to a CompactionLevel.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(CompactionLevel::Raw),
            1 => Some(CompactionLevel::Daily),
            2 => Some(CompactionLevel::Weekly),
            3 => Some(CompactionLevel::Monthly),
            4 => Some(CompactionLevel::Root),
            _ => None,
        }
    }

    /// Compression ratio for content being promoted *from* this level.
    ///
    /// The ratio indicates what fraction of lines to keep. Lower levels
    /// compress more aggressively because the source data is more verbose.
    fn compression_ratio(&self) -> f64 {
        match self {
            CompactionLevel::Raw => 0.30,     // Keep 30% of raw lines
            CompactionLevel::Daily => 0.40,   // Keep 40% of daily lines
            CompactionLevel::Weekly => 0.50,  // Keep 50% of weekly lines
            CompactionLevel::Monthly => 0.60, // Keep 60% of monthly lines
            CompactionLevel::Root => 1.0,     // Root is never compressed further
        }
    }

    /// Target number of summary lines when promoting content at this level.
    fn target_summary_lines(&self) -> usize {
        match self {
            CompactionLevel::Raw => 15,
            CompactionLevel::Daily => 20,
            CompactionLevel::Weekly => 10,
            CompactionLevel::Monthly => 5,
            CompactionLevel::Root => 0,
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
///
/// # Usage
///
/// ```ignore
/// let tree = CompactionTree::default_tree();
///
/// // Check if content should be promoted
/// if tree.should_promote("long content...", CompactionLevel::Raw) {
///     let compacted = tree.compact_to_level("long content...", CompactionLevel::Raw, CompactionLevel::Daily);
/// }
///
/// // Promote several entries at once
/// let entries = vec!["entry 1".to_string(), "entry 2".to_string()];
/// let daily_summary = tree.promote(&entries, CompactionLevel::Raw);
/// ```
pub struct CompactionTree {
    /// Line count threshold for triggering compaction.
    pub line_threshold: usize,
}

impl CompactionTree {
    /// Create a new compaction tree with the given line threshold.
    pub fn new(line_threshold: usize) -> Self {
        Self { line_threshold }
    }

    /// Create with default threshold (200 lines).
    pub fn default_tree() -> Self {
        Self::new(200)
    }

    /// Check if content should be compacted based on line count.
    ///
    /// Uses the tree's configured `line_threshold`.
    pub fn should_compact(&self, content: &str) -> bool {
        content.lines().count() >= self.line_threshold
    }

    /// Check if content at a given level should be promoted to the next level.
    ///
    /// Promotion is indicated when the content exceeds the threshold for
    /// its current compaction level.
    pub fn should_promote(&self, content: &str, current_level: CompactionLevel) -> bool {
        let line_count = content.lines().count();
        line_count >= current_level.threshold()
    }

    /// Compact content from one level to another.
    ///
    /// This applies rule-based compression appropriate for the target level.
    /// If `to` is not strictly higher than `from`, the content is returned
    /// unchanged.
    ///
    /// # Multi-level compaction
    ///
    /// When compacting across multiple levels (e.g., Raw → Weekly), the
    /// compression is applied progressively through each intermediate level
    /// for better quality.
    pub fn compact_to_level(
        &self,
        content: &str,
        from: CompactionLevel,
        to: CompactionLevel,
    ) -> String {
        if to.as_u8() <= from.as_u8() {
            // Cannot compact to same or lower level
            return content.to_string();
        }

        // Progressive compaction through each intermediate level
        let mut current_content = content.to_string();
        let mut current_level = from;

        while current_level < to {
            let next_level = match current_level.next() {
                Some(n) => n,
                None => break,
            };
            current_content = self.compact_single_level(&current_content, current_level);
            current_level = next_level;
        }

        current_content
    }

    /// Promote multiple entries from a given level into a single summary
    /// at the next level.
    ///
    /// This is used by the dream process to consolidate several entries
    /// (e.g., multiple daily summaries) into one higher-level summary
    /// (e.g., a weekly summary).
    ///
    /// # Returns
    ///
    /// A compacted summary string combining all entries.
    pub fn promote(&self, entries: &[String], level: CompactionLevel) -> String {
        if entries.is_empty() {
            return String::new();
        }

        if entries.len() == 1 {
            // Single entry: just compact it
            return self.compact_single_level(&entries[0], level);
        }

        // Combine entries with section markers
        let combined = entries.join("\n---\n");

        // Apply compaction for the source level
        let compacted = self.compact_single_level(&combined, level);

        // Add header indicating promotion
        let header = match level.next() {
            Some(next) => format!(
                "[{} summary from {} entries]",
                next.dir_name(),
                entries.len()
            ),
            None => format!("[Root summary from {} entries]", entries.len()),
        };

        format!("{header}\n{compacted}")
    }

    /// Simple rule-based compaction: preserve first/last sentences of each
    /// paragraph, discard middle.
    ///
    /// This is the fallback when LLM compaction is not available.
    pub fn rule_based_compact(&self, content: &str) -> String {
        self.compact_single_level(content, CompactionLevel::Raw)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Compress content for promotion from a single level to the next.
    ///
    /// Uses a rule-based strategy:
    /// 1. If content is short enough, return as-is.
    /// 2. Extract key lines: section headers, first/last lines of sections.
    /// 3. Apply compression ratio to determine how many lines to keep.
    /// 4. Preserve structural markers (headers, separators).
    fn compact_single_level(&self, content: &str, from_level: CompactionLevel) -> String {
        let lines: Vec<&str> = content.lines().collect();

        // Short content: no compaction needed
        if lines.len() < 5 {
            return content.to_string();
        }

        let ratio = from_level.compression_ratio();
        let target = from_level.target_summary_lines();
        let keep_count = ((lines.len() as f64) * ratio) as usize;
        let keep_count = keep_count.max(target).min(lines.len());

        // If we'd keep everything, skip
        if keep_count >= lines.len() {
            return content.to_string();
        }

        // Strategy: keep structural lines + head + tail
        let mut kept_indices = std::collections::HashSet::new();

        // 1. Always preserve section markers (lines starting with # or ---)
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#')
                || trimmed.starts_with("---")
                || trimmed.starts_with("===")
                || trimmed.starts_with('[')
                || trimmed.starts_with('*')
                || trimmed.is_empty()
            {
                kept_indices.insert(i);
            }
        }

        // 2. Preserve first N non-structural lines
        let head_count = (keep_count / 3).max(2);
        let mut head_taken = 0;
        for (i, _line) in lines.iter().enumerate() {
            if head_taken >= head_count {
                break;
            }
            if !kept_indices.contains(&i) {
                kept_indices.insert(i);
                head_taken += 1;
            }
        }

        // 3. Preserve last N non-structural lines
        let tail_count = (keep_count / 3).max(2);
        let mut tail_taken = 0;
        for (i, _line) in lines.iter().enumerate().rev() {
            if tail_taken >= tail_count {
                break;
            }
            if !kept_indices.contains(&i) {
                kept_indices.insert(i);
                tail_taken += 1;
            }
        }

        // 4. If still under target, sample middle lines at even intervals
        if kept_indices.len() < keep_count {
            let remaining = keep_count - kept_indices.len();
            let middle_lines: Vec<usize> = (0..lines.len())
                .filter(|i| !kept_indices.contains(i))
                .collect();

            if !middle_lines.is_empty() {
                let step = (middle_lines.len() as f64 / remaining as f64).max(1.0) as usize;
                for idx in (0..middle_lines.len()).step_by(step) {
                    if kept_indices.len() >= keep_count {
                        break;
                    }
                    kept_indices.insert(middle_lines[idx]);
                }
            }
        }

        // Build result in order
        let mut sorted_indices: Vec<usize> = kept_indices.into_iter().collect();
        sorted_indices.sort_unstable();

        let mut result = Vec::new();
        let mut last_idx = 0isize;
        for idx in sorted_indices {
            if last_idx >= 0 && (idx as isize) > last_idx + 1 {
                // Gap detected — add omission marker
                let omitted = idx - (last_idx as usize) - 1;
                result.push(format!("... ({omitted} lines omitted) ..."));
            }
            result.push(lines[idx].to_string());
            last_idx = idx as isize;
        }

        // Trailing omitted lines
        if (last_idx as usize) < lines.len() - 1 {
            let omitted = lines.len() - 1 - (last_idx as usize);
            result.push(format!("... ({omitted} lines omitted) ..."));
        }

        result.join("\n")
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
    fn test_compaction_level_u8_roundtrip() {
        for level in CompactionLevel::all() {
            assert_eq!(CompactionLevel::from_u8(level.as_u8()), Some(*level));
        }
        assert_eq!(CompactionLevel::from_u8(5), None);
    }

    #[test]
    fn test_compaction_level_thresholds() {
        assert_eq!(CompactionLevel::Raw.threshold(), 200);
        assert_eq!(CompactionLevel::Daily.threshold(), 300);
        assert_eq!(CompactionLevel::Weekly.threshold(), 500);
        assert_eq!(CompactionLevel::Monthly.threshold(), usize::MAX);
        assert_eq!(CompactionLevel::Root.threshold(), usize::MAX);
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
    fn test_should_promote_at_threshold() {
        let tree = CompactionTree::default_tree();
        // Create content with 200+ lines (Raw threshold)
        let content: String = (0..200)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(tree.should_promote(&content, CompactionLevel::Raw));
    }

    #[test]
    fn test_should_not_promote_below_threshold() {
        let tree = CompactionTree::default_tree();
        let content = "line 1\nline 2\nline 3";
        assert!(!tree.should_promote(content, CompactionLevel::Raw));
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
        let content = (0..50)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = tree.rule_based_compact(&content);
        assert!(result.lines().count() < 50, "Should be compacted");
        assert!(result.contains("line 0"), "Should preserve first line");
        assert!(result.contains("line 49"), "Should preserve last line");
        assert!(
            result.contains("omitted"),
            "Should indicate omitted content"
        );
    }

    #[test]
    fn test_compact_to_level_same_level() {
        let tree = CompactionTree::default_tree();
        let content = "line 1\nline 2\nline 3\nline 4\nline 5";
        let result = tree.compact_to_level(content, CompactionLevel::Raw, CompactionLevel::Raw);
        assert_eq!(result, content, "Same level should return unchanged");
    }

    #[test]
    fn test_compact_to_level_lower_level() {
        let tree = CompactionTree::default_tree();
        let content = "line 1\nline 2\nline 3";
        let result = tree.compact_to_level(content, CompactionLevel::Daily, CompactionLevel::Raw);
        assert_eq!(
            result, content,
            "Compacting to lower level should return unchanged"
        );
    }

    #[test]
    fn test_compact_to_level_single_step() {
        let tree = CompactionTree::default_tree();
        let content: String = (0..50)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");

        let result = tree.compact_to_level(&content, CompactionLevel::Raw, CompactionLevel::Daily);
        assert!(
            result.lines().count() < content.lines().count(),
            "Should be compacted"
        );
        assert!(result.contains("line 0"), "Should preserve first line");
    }

    #[test]
    fn test_compact_to_level_multi_step() {
        let tree = CompactionTree::default_tree();
        let content: String = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");

        // Raw → Weekly (2 steps: Raw→Daily→Weekly)
        let result = tree.compact_to_level(&content, CompactionLevel::Raw, CompactionLevel::Weekly);
        assert!(
            result.lines().count() < 100,
            "Multi-step compaction should reduce size"
        );
    }

    #[test]
    fn test_compact_preserves_headers() {
        let tree = CompactionTree::default_tree();
        let content = "# Header 1\n\
                       line 1\nline 2\nline 3\nline 4\nline 5\n\
                       # Header 2\n\
                       line 6\nline 7\nline 8\nline 9\nline 10";

        let result = tree.compact_single_level(content, CompactionLevel::Raw);
        assert!(result.contains("# Header 1"), "Should preserve headers");
        assert!(result.contains("# Header 2"), "Should preserve headers");
    }

    #[test]
    fn test_promote_multiple_entries() {
        let tree = CompactionTree::default_tree();
        let entries: Vec<String> = (0..3)
            .map(|i| {
                (0..20)
                    .map(|j| format!("entry {} line {}", i, j))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect();

        let result = tree.promote(&entries, CompactionLevel::Raw);
        assert!(
            result.contains("[daily summary from 3 entries]"),
            "Should include promotion header"
        );
        // Result should be shorter than the sum of all entries
        let total_original: usize = entries.iter().map(|e| e.lines().count()).sum();
        assert!(
            result.lines().count() < total_original,
            "Promoted result should be shorter than combined originals"
        );
    }

    #[test]
    fn test_promote_single_entry() {
        let tree = CompactionTree::default_tree();
        let entries = vec![
            (0..20)
                .map(|i| format!("line {}", i))
                .collect::<Vec<_>>()
                .join("\n"),
        ];

        let result = tree.promote(&entries, CompactionLevel::Raw);
        // Single entry should just be compacted (no header)
        assert!(
            !result.contains("summary from"),
            "Single entry should not have multi-entry header"
        );
    }

    #[test]
    fn test_promote_empty() {
        let tree = CompactionTree::default_tree();
        let result = tree.promote(&[], CompactionLevel::Raw);
        assert!(result.is_empty(), "Empty input should return empty string");
    }

    #[test]
    fn test_compact_single_level_very_short() {
        let tree = CompactionTree::default_tree();
        let content = "line 1\nline 2";
        let result = tree.compact_single_level(content, CompactionLevel::Raw);
        assert_eq!(
            result, content,
            "Very short content should not be compacted"
        );
    }

    #[test]
    fn test_dir_name() {
        assert_eq!(CompactionLevel::Raw.dir_name(), "raw");
        assert_eq!(CompactionLevel::Daily.dir_name(), "daily");
        assert_eq!(CompactionLevel::Weekly.dir_name(), "weekly");
        assert_eq!(CompactionLevel::Monthly.dir_name(), "monthly");
        assert_eq!(CompactionLevel::Root.dir_name(), "root");
    }
}
