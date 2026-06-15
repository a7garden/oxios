//! Mount detection: find a Mount matching user input (RFC-025).
//!
//! Replaces RFC-011's tag-based detection layer 3 with `auto_meta` keyword
//! matching. Layers:
//! 1. Direct name match ("oxios" → Mount named "oxios")
//! 2. Path extraction + prefix match (most specific wins)
//! 3. `auto_meta` keyword match (languages / stack / summary keywords)

use std::path::PathBuf;

#[cfg(test)]
use super::MountSource;
use super::{Mount, MountId};

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

    // Layer 1: Direct name match (case-insensitive, word-ish boundary).
    // Match the longest name first so "oxios-dev" wins over "oxios".
    let mut by_name: Vec<&Mount> = mounts
        .iter()
        .filter(|m| lower.contains(&m.name.to_lowercase()))
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
                .expect("non-empty");
            return DetectionResult::Found(best.id);
        }
        return DetectionResult::NoMatch {
            detected_path: Some(path),
        };
    }

    // Layer 3: auto_meta keyword match (languages / stack / summary).
    for mount in mounts {
        let keywords: Vec<&str> = mount
            .auto_meta
            .languages
            .iter()
            .chain(mount.auto_meta.stack.iter())
            .chain(std::iter::once(&mount.auto_meta.summary))
            .map(String::as_str)
            .collect();
        for kw in keywords {
            let kw = kw.trim().to_lowercase();
            if kw.len() >= 3 && lower.contains(&kw) {
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
}
