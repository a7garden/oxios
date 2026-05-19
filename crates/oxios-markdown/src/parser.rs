//! Markdown text processing utilities.
//!
//! Ported from files.md (`server/txt/mod.rs`) by Artem Zakirullin.
//! Provides string similarity, link extraction, and text normalization.

use regex::Regex;

/// Normalize CRLF and CR to LF.
pub fn norm_new_lines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Get the first word from a string.
pub fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or(s)
}

/// Calculate similarity between two strings (0.0 – 100.0) using Levenshtein distance.
pub fn similar(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() { return 0.0; }
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower { return 100.0; }
    let max_len = a_lower.len().max(b_lower.len());
    if max_len == 0 { return 100.0; }
    let distance = levenshtein(&a_lower, &b_lower);
    ((max_len - distance) as f64 / max_len as f64) * 100.0
}

/// Compute Levenshtein edit distance between two strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let len_a = a.len();
    let len_b = b.len();
    if len_a == 0 { return len_b; }
    if len_b == 0 { return len_a; }

    let mut matrix = vec![vec![0usize; len_b + 1]; len_a + 1];
    for i in 0..=len_a { matrix[i][0] = i; }
    for j in 0..=len_b { matrix[0][j] = j; }

    for i in 1..=len_a {
        for j in 1..=len_b {
            let cost = if a.as_bytes()[i - 1] == b.as_bytes()[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }
    matrix[len_a][len_b]
}

/// Truncate a string to `max_len`, appending "..." if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Check if text contains a markdown image.
pub fn has_image(msg: &str) -> bool {
    Regex::new(r"!\[.*?\]\(.*?\)").unwrap().is_match(msg)
}

/// Strip a leading `` `HH:MM` `` timestamp from chat entries.
pub fn strip_chat_timestamp(s: &str) -> String {
    Regex::new(r"^`\d{2}:\d{2}` ").unwrap().replace(s, "").to_string()
}

/// Extract all markdown links `[text](path)` from content.
///
/// Returns a list of (link_text, target_path) pairs.
pub fn extract_markdown_links(content: &str) -> Vec<(String, String)> {
    let re = Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| {
            let text = cap.get(1)?.as_str().to_string();
            let path = cap.get(2)?.as_str().to_string();
            // Skip external links and images
            if path.starts_with("http://") || path.starts_with("https://") { return None; }
            Some((text, path))
        })
        .collect()
}

/// Extract all headings (`## Title`) from content.
///
/// Returns heading texts (without the `#` prefix).
pub fn extract_headings(content: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^(#{1,6})\s+(.+)$").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(2).map(|m| m.as_str().trim().to_string()))
        .collect()
}

/// Minimum similarity score (0-100) for fuzzy name search.
pub const MIN_SEARCH_SIMILARITY: i32 = 70;

/// 오늘 날짜의 Chat.md 헤더 문자열 (예: "#### 20 May, Tuesday").
pub fn today_chat_header() -> String {
    use chrono::Local;
    let now = Local::now();
    format!("#### {} {}", now.format("%d %B,"), now.format("%A"))
}

/// 오늘 날짜의 저널 파일 경로 (예: "journal/2026.05 May.md").
pub fn today_journal_path() -> String {
    use chrono::Local;
    let now = Local::now();
    format!("journal/{}.{} {}.md", now.format("%Y.%m"), now.format("%B"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_norm_newlines() {
        assert_eq!(norm_new_lines("a\r\nb\r\nc"), "a\nb\nc");
        assert_eq!(norm_new_lines("a\rb\rc"), "a\nb\nc");
    }

    #[test]
    fn test_similar() {
        assert!(similar("hello", "helo") > 70.0);
        assert!(similar("test", "test") > 99.0);
        assert_eq!(similar("", ""), 0.0);
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("test", "test"), 0);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_extract_links() {
        let md = "See [Rust](brain/Rust.md) and [Go](brain/Go.md) but not [ext](https://example.com)";
        let links = extract_markdown_links(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].0, "Rust");
        assert_eq!(links[0].1, "brain/Rust.md");
    }

    #[test]
    fn test_extract_headings() {
        let md = "# Title\n## Section\n### Sub\nsome text";
        let headings = extract_headings(md);
        assert_eq!(headings, vec!["Title", "Section", "Sub"]);
    }

    #[test]
    fn test_has_image() {
        assert!(has_image("look: ![alt](img.png)"));
        assert!(!has_image("just text"));
    }
}
