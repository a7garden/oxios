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

/// First character uppercase (Unicode-aware).
///
/// ```
/// use oxios_markdown::parser::ucfirst;
/// assert_eq!(ucfirst("hello"), "Hello");
/// assert_eq!(ucfirst(""), "");
/// assert_eq!(ucfirst("über"), "Über");
/// ```
pub fn ucfirst(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// First character lowercase (Unicode-aware).
///
/// ```
/// use oxios_markdown::parser::lcfirst;
/// assert_eq!(lcfirst("Hello"), "hello");
/// assert_eq!(lcfirst(""), "");
/// ```
pub fn lcfirst(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Unicode-safe substring.
///
/// Respects Unicode codepoints but is not grapheme-cluster aware
/// (combining characters like skin-tone modifiers count as separate codepoints).
///
/// ```
/// use oxios_markdown::parser::substr;
/// assert_eq!(substr("Hello", 0, 3), "Hel");
/// assert_eq!(substr("Hello", 3, 10), "lo");
/// assert_eq!(substr("Hello", 10, 2), "");
/// ```
pub fn substr(input: &str, start: usize, length: usize) -> String {
    let runes: Vec<char> = input.chars().collect();
    if start >= runes.len() {
        return String::new();
    }
    let end = (start + length).min(runes.len());
    runes[start..end].iter().collect()
}

/// Check if text has multiple lines.
///
/// ```
/// use oxios_markdown::parser::is_multiline;
/// assert!(is_multiline("line one\nline two"));
/// assert!(!is_multiline("single line"));
/// ```
pub fn is_multiline(text: &str) -> bool {
    let text = norm_new_lines(text);
    text.lines().count() > 1
}

/// Split text into chunks of at most `max_len` characters.
///
/// Tries to break at the last newline, then the last space within the window.
/// Trims leading/trailing whitespace from each chunk.
///
/// ```
/// use oxios_markdown::parser::split_text_into_chunks;
/// let chunks = split_text_into_chunks("Hello world how are you", 11);
/// assert!(chunks.len() > 1);
/// for chunk in &chunks {
///     assert!(chunk.len() <= 11);
/// }
/// ```
pub fn split_text_into_chunks(text: &str, max_len: usize) -> Vec<String> {
    let text = text.trim();

    if max_len == 0 {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut runes: Vec<char> = text.chars().collect();

    while runes.len() > max_len {
        let window = &runes[..max_len];

        // Find the last newline in the window
        let mut split_index = None;
        for i in (0..window.len()).rev() {
            if window[i] == '\n' {
                split_index = Some(i);
                break;
            }
        }

        // No newline — find the last space
        if split_index.is_none() {
            for i in (0..window.len()).rev() {
                if window[i] == ' ' {
                    split_index = Some(i);
                    break;
                }
            }
        }

        // No space either — split at max_len
        let split_index = split_index.unwrap_or(max_len);

        let chunk: String = runes[..split_index].iter().collect();
        let chunk = chunk.trim();
        if !chunk.is_empty() {
            chunks.push(chunk.to_string());
        }

        let remainder: String = runes[split_index..].iter().collect();
        runes = remainder.trim().chars().collect();
    }

    // Add the remaining runes as the final chunk
    let remainder: String = runes.iter().collect();
    let remainder = remainder.trim();
    if !remainder.is_empty() {
        chunks.push(remainder.to_string());
    }

    chunks
}

/// Known emoji prefixes to strip before re-adding.
const EMOJI_STRIP_PREFIXES: &[&str] = &[
    "WRK ", "UA ", "US ", "CY ", "HOB ", "SRB ", "PL ",
];

/// Add emoji prefix to string, stripping known prefixes first.
///
/// If `emoji` is empty the string is returned with prefixes stripped only.
///
/// ```
/// use oxios_markdown::parser::emoji_prefix;
/// assert_eq!(emoji_prefix("📝", "WRK Task"), "📝 Task");
/// assert_eq!(emoji_prefix("", "Hello"), "Hello");
/// ```
pub fn emoji_prefix(emoji: &str, s: &str) -> String {
    let mut s = s.to_string();
    for prefix in EMOJI_STRIP_PREFIXES {
        s = s.trim_start_matches(prefix).to_string();
    }
    if emoji.is_empty() {
        return s;
    }
    format!("{} {}", emoji, s)
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
    format!("journal/{}.{}.md", now.format("%Y.%m"), now.format("%B"))
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
    fn test_ucfirst() {
        assert_eq!(ucfirst("hello"), "Hello");
        assert_eq!(ucfirst(""), "");
        assert_eq!(ucfirst("Already"), "Already");
        assert_eq!(ucfirst("über"), "Über");
    }

    #[test]
    fn test_lcfirst() {
        assert_eq!(lcfirst("Hello"), "hello");
        assert_eq!(lcfirst(""), "");
        assert_eq!(lcfirst("lower"), "lower");
    }

    #[test]
    fn test_substr() {
        assert_eq!(substr("Hello", 0, 3), "Hel");
        assert_eq!(substr("Hello", 2, 3), "llo");
        assert_eq!(substr("Hello", 3, 10), "lo");
        assert_eq!(substr("Hello", 10, 2), "");
        assert_eq!(substr("", 0, 5), "");
        // Unicode
        assert_eq!(substr("안녕하세요", 0, 2), "안녕");
    }

    #[test]
    fn test_is_multiline() {
        assert!(is_multiline("line one\nline two"));
        assert!(!is_multiline("single line"));
        assert!(is_multiline("a\r\nb"));
        assert!(!is_multiline(""));
    }

    #[test]
    fn test_split_text_into_chunks() {
        // Exact fit
        let chunks = split_text_into_chunks("Hello", 5);
        assert_eq!(chunks, vec!["Hello"]);

        // Split at space
        let chunks = split_text_into_chunks("Hello world how are you", 11);
        for chunk in &chunks {
            assert!(chunk.len() <= 11, "chunk too long: '{}' ({})", chunk, chunk.len());
        }
        assert!(chunks.len() > 1);

        // Split at newline
        let chunks = split_text_into_chunks("Line one\nLine two\nLine three", 9);
        assert_eq!(chunks.len(), 3);

        // max_len == 0 returns everything as one chunk
        let chunks = split_text_into_chunks("Hello world", 0);
        assert_eq!(chunks, vec!["Hello world"]);
    }

    #[test]
    fn test_emoji_prefix() {
        assert_eq!(emoji_prefix("📝", "WRK Task"), "📝 Task");
        assert_eq!(emoji_prefix("✅", "Task"), "✅ Task");
        assert_eq!(emoji_prefix("", "Hello"), "Hello");
        assert_eq!(emoji_prefix("🎉", "UA Celebration"), "🎉 Celebration");
    }

    #[test]
    fn test_has_image() {
        assert!(has_image("look: ![alt](img.png)"));
        assert!(!has_image("just text"));
    }
}
