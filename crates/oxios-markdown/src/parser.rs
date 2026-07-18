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
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower {
        return 100.0;
    }
    let max_len = a_lower.len().max(b_lower.len());
    if max_len == 0 {
        return 100.0;
    }
    let distance = levenshtein(&a_lower, &b_lower);
    ((max_len - distance) as f64 / max_len as f64) * 100.0
}

/// Compute Levenshtein edit distance between two strings.
#[allow(clippy::needless_range_loop)]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let len_a = a.len();
    let len_b = b.len();
    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }

    let mut matrix = vec![vec![0usize; len_b + 1]; len_a + 1];
    for i in 0..=len_a {
        matrix[i][0] = i;
    }
    for j in 0..=len_b {
        matrix[0][j] = j;
    }

    for i in 1..=len_a {
        for j in 1..=len_b {
            let cost = if a.as_bytes()[i - 1] == b.as_bytes()[j - 1] {
                0
            } else {
                1
            };
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
const EMOJI_STRIP_PREFIXES: &[&str] = &["WRK ", "UA ", "US ", "CY ", "HOB ", "SRB ", "PL "];

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
    format!("{emoji} {s}")
}

/// Check if text contains a markdown image.
pub fn has_image(msg: &str) -> bool {
    Regex::new(r"!\[.*?\]\(.*?\)").unwrap().is_match(msg)
}

/// Strip a leading `` `HH:MM` `` timestamp from chat entries.
pub fn strip_chat_timestamp(s: &str) -> String {
    Regex::new(r"^`\d{2}:\d{2}` ")
        .unwrap()
        .replace(s, "")
        .to_string()
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
            if path.starts_with("http://") || path.starts_with("https://") {
                return None;
            }
            Some((text, path))
        })
        .collect()
}

/// Rewrite markdown-link targets matching `old_target` to `new_target`.
///
/// Matches the `[text](target)` form produced/consumed by
/// [`extract_markdown_links`]. The target is matched literally (regex-escaped)
/// inside the trailing `(...)` of a link, so it won't touch the same string
/// appearing in prose or code. Anchors/extensions are preserved only when
/// they were part of the captured target — i.e. an exact-target match.
///
/// Returns the number of replacements made.
pub fn rewrite_link_targets(content: &str, old_target: &str, new_target: &str) -> (String, usize) {
    if old_target == new_target || old_target.is_empty() {
        return (content.to_string(), 0);
    }
    // Match `](<old_target>)` — the `]` guards against replacing the target
    // text when it shows up in link labels or body prose.
    let pattern = format!(r"\]\({}\)", regex::escape(old_target));
    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return (content.to_string(), 0),
    };
    let replacement = format!("]({new_target})");
    let count = re.find_iter(content).count();
    (
        re.replace_all(content, replacement.as_str()).to_string(),
        count,
    )
}

/// Extract all wikilinks `[[target]]` / `[[target|alias]]` from content.
///
/// Returns `(target, alias?)` pairs. The alias is the LAST group when
/// multiple `|`-separated aliases are present (mirrors the frontend
/// widget's backreference semantics). Frontmatter is stripped first so
/// links inside metadata blocks are ignored.
pub fn extract_wikilinks(content: &str) -> Vec<(String, Option<String>)> {
    let body = crate::backlinks::strip_frontmatter(content);
    let re = match Regex::new(r"\[\[([^\[\]\n|]+)(?:\|([^\[\]\n]+))*\]\]") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    re.captures_iter(body)
        .filter_map(|cap| {
            let target = cap.get(1)?.as_str().trim().to_string();
            if target.is_empty() {
                return None;
            }
            let alias = cap.get(2).map(|m| m.as_str().trim().to_string());
            Some((target, alias))
        })
        .collect()
}

/// Map of lowercase filename stem → candidate full paths. Built by the
/// knowledge base from the filesystem; consumed by [`resolve_wikilink`].
pub type StemIndex = std::collections::HashMap<String, Vec<String>>;

/// Resolve a wikilink target to a canonical note path.
///
/// Mirrors the frontend resolver in `web/src/lib/wikilink-resolve.ts`:
///   - `brain/Rust.md` → exact match
///   - `brain/Rust`    → `brain/Rust.md`
///   - `Rust`          → unique stem match; on collision prefer the same
///     directory as `source_path`; still ambiguous → None.
///
/// Returning `None` for ambiguous bare stems is load-bearing: it is what
/// prevents a bare-stem wikilink from being indexed (and later rewritten
/// on rename) when we can't prove which file it meant. See the design
/// doc §6.
pub fn resolve_wikilink(
    target: &str,
    source_path: Option<&str>,
    stem_index: &StemIndex,
) -> Option<String> {
    let t = target.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_lowercase();
    // Form 1: full path with extension — exact membership.
    if lower.ends_with(".md") {
        return path_exists(t, stem_index).then_some(t.to_string());
    }
    // Form 2: path with a directory separator — append `.md`, exact.
    if t.contains('/') {
        let with_ext = format!("{t}.md");
        return path_exists(&with_ext, stem_index).then_some(with_ext);
    }
    // Form 3: bare stem — basename lookup with same-dir preference.
    let candidates = stem_index.get(&lower)?;
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }
    if let Some(src) = source_path {
        let src_dir = dir_of(src);
        let same_dir: Vec<&String> = candidates.iter().filter(|p| dir_of(p) == src_dir).collect();
        if same_dir.len() == 1 {
            return Some(same_dir[0].clone());
        }
    }
    None
}

/// Rewrite `[[target]]` / `[[target|alias]]` wikilinks whose target points
/// at `old_path` so they point at `new_path`, preserving the user's
/// original form (bare stem stays bare, path-without-ext stays that way,
/// aliases are kept verbatim).
///
/// Matches three target forms derived from `old_path`:
///   - the full path (`brain/Rust.md`)
///   - the path without extension (`brain/Rust`)
///   - the bare stem (`Rust`)
///
/// Per the design doc §6, bare-stem rewrites are safe here because the
/// caller (`note_move`) only feeds in source files already known (via the
/// backward index) to have linked to `old_path` — meaning their bare-stem
/// wikilinks were proven unique at index time. We do NOT re-resolve at
/// rewrite time (old_path is already gone from disk by then).
///
/// Returns the rewritten content and the number of substitutions made.
pub fn rewrite_wikilink_targets(
    content: &str,
    old_path: &str,
    new_path: &str,
    // Pre-rename stem index. Required to safely rewrite BARE-STEM wikilinks
    // (`[[Rust]]`): we only rewrite when the stem is globally unique, so an
    // ambiguous `[[Dup]]` (two files share the stem) is left untouched even
    // if its source also contained an explicit-path link to old_path that
    // put it in `sources_for`. Explicit-path forms are exact matches and
    // need no ambiguity check. Pass None to disable bare-stem rewrites.
    stem_index: Option<&StemIndex>,
) -> (String, usize) {
    if old_path == new_path || old_path.is_empty() {
        return (content.to_string(), 0);
    }
    let old_no_ext = old_path.strip_suffix(".md").unwrap_or(old_path);
    let new_no_ext = new_path.strip_suffix(".md").unwrap_or(new_path);
    let old_stem = old_no_ext.rsplit('/').next().unwrap_or(old_no_ext);
    let new_stem = new_no_ext.rsplit('/').next().unwrap_or(new_no_ext);

    let re = match Regex::new(r"\[\[([^\[\]\n|]+)((?:\|[^\[\]\n]+)*)\]\]") {
        Ok(r) => r,
        Err(_) => return (content.to_string(), 0),
    };
    let mut count = 0usize;
    let result = re
        .replace_all(content, |caps: &regex::Captures| {
            let full = caps.get(0).unwrap().as_str();
            let target = caps.get(1).unwrap().as_str();
            let alias_part = caps.get(2).unwrap().as_str();
            // Explicit-path forms are exact matches — always safe.
            // The bare-stem form is only safe when the stem is globally
            // unique (design doc §6): a stem shared by several files
            // could have pointed at any of them, so rewriting would
            // risk retargeting a link the system can't disambiguate.
            let new_target = if target == old_path {
                Some(new_path)
            } else if target == old_no_ext {
                Some(new_no_ext)
            } else if target == old_stem
                && old_stem != new_stem
                && stem_index
                    .and_then(|idx| idx.get(&target.to_lowercase()))
                    .is_some_and(|candidates| candidates.len() == 1)
            {
                Some(new_stem)
            } else {
                None
            };
            match new_target {
                Some(nt) => {
                    count += 1;
                    format!("[[{nt}{alias_part}]]")
                }
                None => full.to_string(),
            }
        })
        .to_string();
    (result, count)
}

/// Whether a full path exists in the stem index (stem lookup + bucket membership).
fn path_exists(path: &str, stem_index: &StemIndex) -> bool {
    stem_index
        .get(&stem_of(path))
        .is_some_and(|bucket| bucket.iter().any(|p| p == path))
}

fn stem_of(path: &str) -> String {
    let basename = path.rsplit('/').next().unwrap_or(path);
    basename
        .strip_suffix(".md")
        .or_else(|| basename.strip_suffix(".MD"))
        .unwrap_or(basename)
        .to_lowercase()
}

fn dir_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
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
        let md =
            "See [Rust](brain/Rust.md) and [Go](brain/Go.md) but not [ext](https://example.com)";
        let links = extract_markdown_links(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].0, "Rust");
        assert_eq!(links[0].1, "brain/Rust.md");
    }

    fn stem_index(entries: &[&str]) -> StemIndex {
        let mut idx: StemIndex = StemIndex::new();
        for path in entries {
            let stem = path
                .rsplit('/')
                .next()
                .unwrap_or(path)
                .trim_end_matches(".md")
                .to_lowercase();
            idx.entry(stem).or_default().push((*path).to_string());
        }
        idx
    }

    #[test]
    fn test_extract_wikilinks() {
        let md = "See [[Rust]] and [[brain/Go|The Go Page]] but not [md](brain/Other.md)";
        let links = extract_wikilinks(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].0, "Rust");
        assert!(links[0].1.is_none());
        assert_eq!(links[1].0, "brain/Go");
        assert_eq!(links[1].1.as_deref(), Some("The Go Page"));
    }

    #[test]
    fn test_resolve_wikilink() {
        let idx = stem_index(&[
            "brain/Rust.md",
            "brain/Ownership.md",
            "lang/Rust.md",
            "Notes.md",
        ]);
        // Full path with extension — exact.
        assert_eq!(
            resolve_wikilink("brain/Rust.md", None, &idx),
            Some("brain/Rust.md".into())
        );
        assert_eq!(resolve_wikilink("brain/Missing.md", None, &idx), None);
        // Path without extension appends `.md`.
        assert_eq!(
            resolve_wikilink("brain/Ownership", None, &idx),
            Some("brain/Ownership.md".into())
        );
        // Unique bare stem.
        assert_eq!(
            resolve_wikilink("Notes", None, &idx),
            Some("Notes.md".into())
        );
        // Ambiguous bare stem resolves via same-dir hint.
        assert_eq!(
            resolve_wikilink("Rust", Some("brain/Ownership.md"), &idx),
            Some("brain/Rust.md".into()),
        );
        assert_eq!(
            resolve_wikilink("Rust", Some("lang/Other.md"), &idx),
            Some("lang/Rust.md".into())
        );
        // No hint → unresolved.
        assert_eq!(resolve_wikilink("Rust", None, &idx), None);
        // Unknown / empty.
        assert_eq!(resolve_wikilink("Nowhere", None, &idx), None);
        assert_eq!(resolve_wikilink("", None, &idx), None);
    }

    #[test]
    fn test_rewrite_link_targets() {
        let md = "See [Rust](brain/Rust.md) and [also](brain/Rust.md); prose brain/Rust.md stays.";
        let (out, n) = rewrite_link_targets(md, "brain/Rust.md", "brain/Rust Lang.md");
        assert_eq!(n, 2);
        assert!(out.contains("[Rust](brain/Rust Lang.md)"));
        assert!(out.contains("[also](brain/Rust Lang.md)"));
        // Untouched in prose / other links
        assert!(out.contains("prose brain/Rust.md stays"));
        // No-op when targets equal
        let (same, zero) = rewrite_link_targets(md, "brain/Rust.md", "brain/Rust.md");
        assert_eq!(zero, 0);
        assert_eq!(same, md);
    }

    #[test]
    fn test_rewrite_wikilink_targets() {
        // Unique stem: bare, path, full, and alias forms all rewrite.
        let unique = stem_index(&["brain/Rust.md"]);
        let md = "Bare [[Rust]] path [[brain/Rust]] full [[brain/Rust.md]] alias [[Rust|Rusty]].";
        let (out, n) =
            rewrite_wikilink_targets(md, "brain/Rust.md", "brain/Rust Lang.md", Some(&unique));
        assert_eq!(n, 4);
        assert!(out.contains("[[Rust Lang]]"));
        assert!(out.contains("[[brain/Rust Lang]]"));
        assert!(out.contains("[[brain/Rust Lang.md]]"));
        assert!(
            out.contains("[[Rust Lang|Rusty]]"),
            "alias preserved: {out}"
        );

        // Ambiguous stem: bare form is SKIPPED (can't prove which file it
        // meant), explicit-path forms still rewrite.
        let ambiguous = stem_index(&["a/Dup.md", "b/Dup.md"]);
        let md2 = "ambig [[Dup]] explicit [[a/Dup]] full [[a/Dup.md]]";
        let (out2, n2) = rewrite_wikilink_targets(md2, "a/Dup.md", "a/Moved.md", Some(&ambiguous));
        assert!(
            out2.contains("[[Dup]]"),
            "ambiguous bare link preserved: {out2}"
        );
        assert!(
            out2.contains("[[a/Moved]]"),
            "explicit path rewritten: {out2}"
        );
        assert!(
            out2.contains("[[a/Moved.md]]"),
            "full path rewritten: {out2}"
        );
        assert_eq!(n2, 2);

        // Bare stem NOT rewritten when only the directory changed (stem
        // unchanged → no-op, skipped even though stem is unique).
        let (out3, n3) =
            rewrite_wikilink_targets("[[Rust]]", "brain/Rust.md", "lang/Rust.md", Some(&unique));
        assert_eq!(n3, 0);
        assert_eq!(out3, "[[Rust]]");

        // No stem_index → bare-stem rewrites disabled (conservative).
        // rewrite_wikilink_targets ONLY touches wikilinks; the markdown
        // link in the same content is handled separately by rewrite_link_targets.
        let (out4, n4) = rewrite_wikilink_targets(
            "[[Rust]] [r](brain/Rust.md)",
            "brain/Rust.md",
            "brain/X.md",
            None,
        );
        assert_eq!(n4, 0); // no wikilink rewrites (bare stem disabled, markdown links untouched here)
        assert!(out4.contains("[[Rust]]"));
        assert!(out4.contains("[r](brain/Rust.md)"));

        // No-op when paths equal.
        let (same, zero) =
            rewrite_wikilink_targets(md, "brain/Rust.md", "brain/Rust.md", Some(&unique));
        assert_eq!(zero, 0);
        assert_eq!(same, md);
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

        // Split at space (Go test: basic split with spaces)
        let chunks = split_text_into_chunks("This is a test to check the splitting of text", 10);
        for chunk in &chunks {
            assert!(
                chunk.len() <= 10,
                "chunk too long: '{}' ({})",
                chunk,
                chunk.len()
            );
        }

        // Split at newline (Go test: max_len=15)
        let chunks = split_text_into_chunks("Line one\nLine two\nLine three", 15);
        assert_eq!(chunks, vec!["Line one", "Line two", "Line three"]);

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
