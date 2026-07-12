//! Mount detection: find a Mount matching user input (RFC-025).
//!
//! Replaces RFC-011's tag-based detection layer 3 with `auto_meta` keyword
//! matching. Layers:
//! 1. Direct name match ("oxios" → Mount named "oxios")
//! 2. Path extraction + prefix match (most specific wins)
//! 3. `auto_meta` keyword match (languages / stack / summary keywords)

use std::path::PathBuf;

use super::{Mount, MountId};

/// Check if `haystack` contains `needle` as a whole word (token),
/// case-insensitive. A character is considered part of the same word only if
/// it is an ASCII alphanumeric or `_`. This means:
///   - Latin substring false-positives are prevented ("go" does not match
///     "going", "rust" does not match "trust") — the adjacent ASCII letter is
///     a word continuation, not a boundary.
///   - A script transition is a boundary, so Korean/Japanese postpositions
///     written without spaces ("oxios에서", "oxios로") still let the Latin
///     name match. This codebase is Korean-user-facing, so this is the
///     desired behaviour.
///
/// Unicode-safe: boundary checks examine actual characters (not raw bytes),
/// and the search cursor is advanced one character at a time so multi-byte
/// (e.g. CJK) haystacks never slice on a non-char-boundary.
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let h: String = haystack.to_lowercase();
    let n: String = needle.to_lowercase();

    /// `true` if `c` continues the current word (ASCII alphanumeric or `_`).
    /// Everything else — punctuation, whitespace, or a non-ASCII script char
    /// — acts as a word boundary.
    fn continues_word(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    let mut start = 0;
    while start < h.len() {
        let Some(rel) = h[start..].find(&n) else {
            break;
        };
        let abs_pos = start + rel;
        let end_pos = abs_pos + n.len();

        // Character immediately before the match (if any) must be a boundary.
        let before_ok = abs_pos == 0
            || h[..abs_pos]
                .chars()
                .next_back()
                .is_none_or(|c| !continues_word(c));
        // Character immediately after the match (if any) must be a boundary.
        let after_ok = end_pos >= h.len()
            || h[end_pos..]
                .chars()
                .next()
                .is_none_or(|c| !continues_word(c));

        if before_ok && after_ok {
            return true;
        }
        // Advance past this occurrence by exactly one character so that
        // overlapping matches are still considered and `start` remains on a
        // valid char boundary (required for `h[start..]` slicing).
        start = match h[abs_pos..].char_indices().nth(1) {
            Some((i, _)) => abs_pos + i,
            None => h.len(),
        };
    }
    false
}

/// Result of a Mount lookup attempt.
#[derive(Debug, Clone)]
pub enum DetectionResult {
    /// Found a matching Mount.
    Found(MountId),
    /// No Mount matched. Optionally, a path was detected.
    NoMatch { detected_path: Option<PathBuf> },
}

/// Try to detect a Mount from a user message.
///
/// Detection considers **only Mounts**, never Projects (RFC-025: Projects
/// always carry user-written instructions and shouldn't be guessed).
pub fn detect_mounts(message: &str, mounts: &[Mount]) -> DetectionResult {
    let lower = message.to_lowercase();

    // Layer 1: Direct name match (case-insensitive, whole-word match).
    // Match the longest name first so "oxios-dev" wins over "oxios".
    // Names shorter than 3 chars are too ambiguous for Layer 1 ("go", "ai",
    // "os", "pi") — they are skipped here (mirrors Layer 3's `kw.len() >= 3`).
    let mut by_name: Vec<&Mount> = mounts
        .iter()
        .filter(|m| m.name.len() >= 3 && contains_word(&lower, &m.name))
        .collect();
    by_name.sort_by_key(|m| std::cmp::Reverse(m.name.len()));
    if let Some(m) = by_name.first() {
        return DetectionResult::Found(m.id);
    }

    // Layer 2: Path extraction + prefix match (most specific path wins).
    if let Some(path) = extract_path(message) {
        let matching: Vec<&Mount> = mounts
            .iter()
            .filter(|m| {
                m.paths
                    .iter()
                    .any(|p| path.starts_with(p) || p.starts_with(&path))
            })
            .collect();
        if matching.len() == 1 {
            return DetectionResult::Found(matching[0].id);
        }
        if matching.len() > 1 {
            // Prefer the most specific path (longest matching prefix).
            // Audit F-4: matching is non-empty (guarded by len()>1), but if
            // the max_by_key closure somehow yields no elements (e.g. all
            // .paths() return None), avoid panicking — fall back to the
            // first matching entry instead of aborting the daemon.
            let best = matching
                .into_iter()
                .max_by_key(|m| {
                    m.paths
                        .iter()
                        .filter(|p| path.starts_with(p))
                        .map(|p| p.components().count())
                        .max()
                        .unwrap_or(0)
                })
                .or_else(|| {
                    tracing::warn!("mount detection: max_by_key yielded None; using first match");
                    None
                });
            return match best {
                Some(b) => DetectionResult::Found(b.id),
                None => DetectionResult::NoMatch {
                    detected_path: Some(path),
                },
            };
        }
        return DetectionResult::NoMatch {
            detected_path: Some(path),
        };
    }

    // Layer 3: auto_meta keyword match (languages / stack / summary).
    //
    // Iterate in deterministic order: most recently active first, then by
    // name. The caller-supplied `mounts` slice order is not guaranteed stable
    // (MountManager builds it from a HashMap), so without sorting the winner
    // among mounts sharing a keyword would be non-deterministic.
    let mut sorted: Vec<&Mount> = mounts.iter().collect();
    sorted.sort_by(|a, b| {
        b.last_active_at
            .cmp(&a.last_active_at)
            .then_with(|| a.name.cmp(&b.name))
    });
    for mount in &sorted {
        // Split the summary into individual words so that a multi-word summary
        // (e.g. "Agent OS in Rust") does not have to match verbatim.
        let keywords: Vec<String> = mount
            .auto_meta
            .languages
            .iter()
            .chain(mount.auto_meta.stack.iter())
            .cloned()
            .chain(mount.auto_meta.summary.split_whitespace().map(String::from))
            .collect();
        for kw in keywords {
            let kw = kw.trim().to_lowercase();
            if kw.len() >= 3 && contains_word(&lower, &kw) {
                return DetectionResult::Found(mount.id);
            }
        }
    }

    DetectionResult::NoMatch {
        detected_path: None,
    }
}

/// Extract a filesystem path from a message string.
///
/// Looks for patterns like `/path/to/something` or `~/path`.
pub fn extract_path(message: &str) -> Option<PathBuf> {
    // Absolute paths
    for word in message.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_'
        });
        if cleaned.starts_with('/') && cleaned.len() > 2 {
            let path = PathBuf::from(cleaned);
            if path.parent().is_some() {
                return Some(path);
            }
        }
    }
    // ~-prefixed paths
    for word in message.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_' && c != '~'
        });
        if cleaned.starts_with("~/")
            && cleaned.len() > 2
            && let Some(home) = std::env::var_os("HOME")
        {
            let expanded = cleaned.replacen("~", &home.to_string_lossy(), 1);
            return Some(PathBuf::from(expanded));
        }
    }
    None
}

/// Find a Mount by exact ID.
pub fn find_by_id(mounts: &[Mount], id: MountId) -> Option<&Mount> {
    mounts.iter().find(|m| m.id == id)
}

/// Find a Mount by name (case-insensitive).
pub fn find_by_name<'a>(mounts: &'a [Mount], name: &str) -> Option<&'a Mount> {
    let lower = name.to_lowercase();
    mounts.iter().find(|m| m.name.to_lowercase() == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mounts() -> Vec<Mount> {
        let mut oxios =
            Mount::from_name_and_path("oxios", PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"));
        oxios.auto_meta.languages = vec!["rust".to_string()];
        oxios.auto_meta.stack = vec!["tokio".to_string()];

        let mut oxi =
            Mount::from_name_and_path("oxi", PathBuf::from("/Volumes/MERCURY/PROJECTS/oxi"));
        oxi.auto_meta.languages = vec!["rust".to_string()];
        oxi.auto_meta.summary = "SDK for Oxios agents".to_string();

        let mut blog = Mount::from_name_and_path("my-blog", PathBuf::from("/Users/me/blog"));
        blog.auto_meta.languages = vec!["typescript".to_string()];
        blog.auto_meta.stack = vec!["nextjs".to_string()];

        vec![oxios, oxi, blog]
    }

    #[test]
    fn test_detect_by_name() {
        let mounts = make_mounts();
        let result = detect_mounts("oxios 코드리뷰해줘", &mounts);
        assert!(matches!(result, DetectionResult::Found(id) if id == mounts[0].id));
    }

    #[test]
    fn test_detect_longest_name_wins() {
        // "oxios-dev" and "oxios" both present; longest name should win.
        let mut mounts = make_mounts();
        mounts.push(Mount::from_name_and_path(
            "oxios-dev",
            PathBuf::from("/dev"),
        ));
        let result = detect_mounts("working on oxios-dev now", &mounts);
        match result {
            DetectionResult::Found(id) => {
                let m = mounts.iter().find(|m| m.id == id).unwrap();
                assert_eq!(m.name, "oxios-dev");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_by_path() {
        let mounts = make_mounts();
        let result = detect_mounts("/Volumes/MERCURY/PROJECTS/oxios에서 작업", &mounts);
        assert!(matches!(result, DetectionResult::Found(id) if id == mounts[0].id));
    }

    #[test]
    fn test_detect_by_meta_keyword() {
        let mounts = make_mounts();
        // "nextjs" is a stack keyword on my-blog.
        let result = detect_mounts("nextjs 관련 도움이 필요해", &mounts);
        match result {
            DetectionResult::Found(id) => {
                let m = mounts.iter().find(|m| m.id == id).unwrap();
                assert_eq!(m.name, "my-blog");
            }
            other => panic!("expected Found (my-blog), got {other:?}"),
        }
    }

    #[test]
    fn test_detect_no_match_with_path() {
        let mounts = make_mounts();
        let result = detect_mounts("/Volumes/MERCURY/PROJECTS/unknown 에서 작업", &mounts);
        assert!(matches!(
            result,
            DetectionResult::NoMatch {
                detected_path: Some(_)
            }
        ));
    }

    #[test]
    fn test_detect_no_match() {
        let mounts = make_mounts();
        let result = detect_mounts("오늘 점심 뭐 먹지?", &mounts);
        assert!(matches!(
            result,
            DetectionResult::NoMatch {
                detected_path: None
            }
        ));
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(
            extract_path("/Volumes/MERCURY/PROJECTS/oxios"),
            Some(PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"))
        );
        assert_eq!(extract_path("no path here"), None);
    }

    #[test]
    fn test_find_by_name() {
        let mounts = make_mounts();
        assert!(find_by_name(&mounts, "oxios").is_some());
        assert!(find_by_name(&mounts, "Oxios").is_some());
        assert!(find_by_name(&mounts, "nonexistent").is_none());
    }

    // --- RFC-025 detection hardening (issues M1/M2/M3) ---

    #[test]
    fn test_short_name_not_substring_matched() {
        // A mount named "go" (len < 3) must NOT match messages where it only
        // appears as a substring of a larger word ("going", "again").
        let mounts = vec![Mount::from_name_and_path("go", PathBuf::from("/p/go"))];
        let result = detect_mounts("i am going there again", &mounts);
        assert!(
            matches!(result, DetectionResult::NoMatch { .. }),
            "short name 'go' must not substring-match 'going'/'again'"
        );
    }

    #[test]
    fn test_name_word_boundary_no_substring() {
        // A 3+ char name must not match as a substring of a larger token.
        // "ring" (len 4) should not match "during", "string", or "brings".
        let mounts = vec![Mount::from_name_and_path("ring", PathBuf::from("/p/ring"))];
        let result = detect_mounts("during the string test it brings results", &mounts);
        assert!(
            matches!(result, DetectionResult::NoMatch { .. }),
            "name 'ring' must not substring-match 'during'/'string'/'brings'"
        );
        // But it SHOULD match as a standalone word.
        let result = detect_mounts("let's talk about ring design", &mounts);
        assert!(matches!(result, DetectionResult::Found(_)));
    }

    #[test]
    fn test_keyword_word_boundary_no_substring() {
        // Layer 3 keyword "rust" must not substring-match "trust".
        let mounts = make_mounts();
        let result = detect_mounts("i really trust you on this", &mounts);
        assert!(
            matches!(result, DetectionResult::NoMatch { .. }),
            "keyword 'rust' must not substring-match 'trust'"
        );
    }

    #[test]
    fn test_word_boundary_with_cjk_after() {
        // A name followed (after a space) by CJK must still match as a word.
        let mounts = make_mounts();
        let result = detect_mounts("oxios 코드리뷰", &mounts);
        assert!(matches!(result, DetectionResult::Found(id) if id == mounts[0].id));
    }

    #[test]
    fn test_layer3_most_recent_active_wins() {
        // Two mounts share the "rust" keyword. The more recently active one
        // must win regardless of the order they appear in the input slice
        // (deterministic tie-break on shared keywords — issue M3).
        let mut oxios = Mount::from_name_and_path("oxios", PathBuf::from("/p/oxios"));
        oxios.auto_meta.languages = vec!["rust".to_string()];

        let mut oxi = Mount::from_name_and_path("oxi", PathBuf::from("/p/oxi"));
        oxi.auto_meta.languages = vec!["rust".to_string()];
        // Make `oxi` more recently active than `oxios`.
        oxi.last_active_at = oxios.last_active_at + chrono::Duration::seconds(60);

        // Deliberately pass them in least-recent-first order.
        let mounts = vec![oxios, oxi];
        let recent_id = mounts[1].id;
        let result = detect_mounts("help with a rust project", &mounts);
        match result {
            DetectionResult::Found(id) => assert_eq!(
                id, recent_id,
                "most recently active mount should win on shared keyword"
            ),
            other => panic!("expected Found, got {other:?}"),
        }
    }
}
