//! Deterministic prompt normalization for cache stability.
//!
//! Ensures that the same logical inputs always produce the same byte output,
//! regardless of insertion order, whitespace variation, or platform differences.
//! This is critical for prompt caching — even a single byte difference
//! invalidates the entire cache.

use std::collections::HashSet;

/// Sort tool names deterministically: core tools first, then alphabetical.
///
/// Core tools are ordered by canonical importance; everything else is sorted
/// alphabetically to ensure stable output regardless of registration order.
pub fn sort_tool_names(names: &[String]) -> Vec<&str> {
    const CORE_ORDER: &[&str] = &[
        "read",
        "write",
        "edit",
        "exec",
        "grep",
        "find",
        "ls",
        "browser",
        "web_search",
    ];

    let core_set: HashSet<&str> = CORE_ORDER.iter().copied().collect();

    let mut core: Vec<&str> = Vec::new();
    let mut other: Vec<&str> = Vec::new();

    for name in names {
        if core_set.contains(name.as_str()) {
            core.push(name.as_str());
        } else {
            other.push(name.as_str());
        }
    }

    // Core tools in canonical order
    core.sort_by_key(|n| {
        CORE_ORDER
            .iter()
            .position(|&c| c == *n)
            .unwrap_or(usize::MAX)
    });

    // Other tools alphabetically
    other.sort();

    core.extend(other);
    core
}

/// Sort context files by their kind ordinal (stable ordering).
pub fn sort_context_files(files: &mut [super::types::ContextFile]) {
    files.sort_by_key(|f| f.kind);
}

/// Deduplicate guidelines, preserving insertion order.
///
/// Normalizes by lowercasing and stripping trailing punctuation before comparison.
pub fn deduplicate_guidelines(guidelines: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for g in guidelines {
        let normalized = g.trim()
            .to_lowercase()
            .trim_end_matches('.')
            .trim_end_matches('!')
            .to_string();
        if !normalized.is_empty() && seen.insert(normalized) {
            result.push(g.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_tool_names_core_first() {
        let names = vec![
            "exec".to_string(),
            "read".to_string(),
            "write".to_string(),
            "mcp-github".to_string(),
            "alpha-tool".to_string(),
        ];
        let sorted = sort_tool_names(&names);
        assert_eq!(sorted[0], "read");
        assert_eq!(sorted[1], "write");
        assert_eq!(sorted[2], "exec");
        // Other tools alphabetically
        assert_eq!(sorted[3], "alpha-tool");
        assert_eq!(sorted[4], "mcp-github");
    }

    #[test]
    fn test_sort_tool_names_all_other() {
        let names = vec![
            "zebra".to_string(),
            "alpha".to_string(),
            "middle".to_string(),
        ];
        let sorted = sort_tool_names(&names);
        assert_eq!(sorted, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn test_deduplicate_guidelines() {
        let guidelines = vec![
            "Be concise".to_string(),
            "Be concise.".to_string(), // lowercase normalized matches
            "Show file paths".to_string(),
            "be concise".to_string(), // duplicate (case-insensitive)
        ];
        let deduped = deduplicate_guidelines(&guidelines);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let guidelines = vec![
            "Third".to_string(),
            "First".to_string(),
            "Second".to_string(),
        ];
        let deduped = deduplicate_guidelines(&guidelines);
        assert_eq!(deduped, vec!["Third", "First", "Second"]);
    }
}
