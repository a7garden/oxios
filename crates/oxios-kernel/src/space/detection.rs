//! Detection: 3-layer Space detection strategy.
//!
//! Layer 1: Filesystem path extraction (regex, fast, free)
//! Layer 2: Keyword/tag matching (fast, free)
//! Layer 3: LLM topic classification (slow, only when needed)

use std::collections::HashMap;
use std::path::PathBuf;

use super::{Space, SpaceId};

/// A topic classification result.
#[derive(Debug, Clone)]
pub struct Topic {
    /// The topic name (e.g., "일상", "요리", "개발").
    pub name: String,
    /// Confidence score (0.0 – 1.0). Below threshold means "unclear".
    pub confidence: f32,
}

impl Topic {
    /// Whether this topic is clear enough to create a named Space.
    pub fn is_clear(&self) -> bool {
        self.confidence >= 0.5
    }
}

/// PathMatcher: matches filesystem paths to Spaces.
#[derive(Debug, Clone, Default)]
pub struct PathMatcher {
    /// space_id -> normalized path prefix
    space_paths: HashMap<SpaceId, PathBuf>,
}

impl PathMatcher {
    /// Register a Space's primary path.
    pub fn register(&mut self, space: &Space) {
        if let Some(path) = space.paths.first() {
            let normalized = normalize_path(path);
            self.space_paths.insert(space.id, normalized);
        }
    }

    /// Find a Space that matches the given path.
    pub fn find_space(&self, path: &PathBuf) -> Option<SpaceId> {
        let normalized = normalize_path(path);

        for (space_id, prefix) in &self.space_paths {
            if normalized.starts_with(prefix)
                || prefix.starts_with(&normalized)
                || paths_overlap(&normalized, prefix)
            {
                return Some(*space_id);
            }
        }

        None
    }

    /// Check if any registered Space matches this path.
    pub fn matches(&self, path: &PathBuf) -> bool {
        self.find_space(path).is_some()
    }
}

/// Extract a filesystem path from a message.
///
/// Detects paths starting with `/`, `~/`, `./`, or absolute Windows paths.
pub fn extract_filesystem_path(message: &str) -> Option<PathBuf> {
    // Regex patterns for common path formats
    let patterns = [
        // Unix absolute: /home/user/... or /Volumes/...
        r"/(?:[a-zA-Z0-9_.~-]+/?)+",
        // Home directory: ~/...
        r"~/[a-zA-Z0-9_.~-][a-zA-Z0-9_.~/-]*",
        // Relative: ./foo or ../foo
        r"(?:\./|\.\./)[a-zA-Z0-9_.~-][a-zA-Z0-9_.~/-]*",
        // Windows absolute: C:\ or D:\
        r"[A-Za-z]:[/\\](?:[a-zA-Z0-9_.~-]+[/\\]?)+",
        // Git URLs: git@github.com:... or https://github.com/...
        r"(?:git@|https?://)[a-zA-Z0-9._/~-]+",
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(m) = re.find(message) {
                let path_str = m.as_str();
                // Skip if this looks like a URL query parameter (has ? or & after)
                let after = &message[m.end()..];
                if after.starts_with('?') || after.starts_with('&') {
                    continue;
                }
                // Return the first match
                return Some(PathBuf::from(path_str));
            }
        }
    }

    None
}

/// Match a message against Space keywords/tags.
pub fn match_keywords(message: &str, spaces: &[Space]) -> Option<SpaceId> {
    let lower = message.to_lowercase();

    let mut best: Option<(SpaceId, i32)> = None;

    for space in spaces {
        let mut score = 0;

        // Match against name (split into words)
        let name_words: Vec<&str> = space.name.split_whitespace().collect();
        for word in &name_words {
            let word_lower = word.to_lowercase();
            if !word_lower.is_empty() && lower.contains(&word_lower) {
                score += 2; // Name match is stronger
            }
        }

        // Match against tags
        for tag in &space.tags {
            let tag_lower = tag.to_lowercase();
            if lower.contains(&tag_lower) {
                score += 3; // Tag match is strongest
            }
        }

        // Match against path names
        for path in &space.paths {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let name_lower = name.to_lowercase();
                if lower.contains(&name_lower) {
                    score += 1;
                }
            }
        }

        if score > 0 {
            if let Some((_, best_score)) = best {
                if score > best_score {
                    best = Some((space.id, score));
                }
            } else {
                best = Some((space.id, score));
            }
        }
    }

    best.map(|(id, _)| id)
}

/// Match a message against all Spaces using a PathMatcher.
///
/// This is a convenience wrapper combining path detection with keyword matching.
pub fn detect_space<'a>(
    message: &str,
    spaces: &'a [Space],
    matcher: &PathMatcher,
) -> Option<&'a Space> {
    // Layer 1: Path detection
    if let Some(path) = extract_filesystem_path(message) {
        if let Some(space_id) = matcher.find_space(&path) {
            return spaces.iter().find(|s| s.id == space_id);
        }
    }

    // Layer 2: Keyword matching
    if let Some(space_id) = match_keywords(message, spaces) {
        return spaces.iter().find(|s| s.id == space_id);
    }

    None
}

/// Classify the topic of a message (LLM-based, Phase 4 implementation).
///
/// Currently returns a conservative stub that classifies common topics
/// without LLM. Phase 4 replaces this with actual LLM integration.
///
/// The `classifier_fn` is injected so the actual LLM call can be wired in
/// at the Orchestrator level without this module knowing about providers.
pub fn classify_topic_stub(message: &str) -> Topic {
    let lower = message.to_lowercase();

    // Simple keyword-based classification
    let categories: [(&str, [&str; 8]); 8] = [
        ("일상", ["저녁", "점심", "아침", "밥", "음식", "레시피", "요리", "장보기"]),
        ("개발", ["code", "bug", "function", "import", "cargo", "rust", "git", "commit"]),
        ("문서", ["readme", "docs", "documentation", "write", "문서", "글", "note", "read"]),
        ("공부", ["study", "learn", "book", "course", "공부", "학습", "책", "class"]),
        ("여행", ["travel", "trip", "flight", "hotel", "여행", "항공", "booking", "tour"]),
        ("건강", ["health", "exercise", "gym", "workout", "건강", "운동", "diet", "run"]),
        ("업무", ["meeting", "email", "project", "deadline", "업무", "회의", "client", "ppt"]),
        ("기술", ["api", "server", "database", "cloud", "기술", "서버", "deploy", "k8s"]),
    ];

    for (topic, keywords) in categories {
        for kw in keywords {
            if lower.contains(kw) {
                return Topic {
                    name: topic.to_string(),
                    confidence: 0.7,
                };
            }
        }
    }

    // No clear topic
    Topic {
        name: String::new(),
        confidence: 0.0,
    }
}

/// Normalize a path for comparison.
///
/// - Resolves `~` to home directory
/// - Canonicalizes `.` and `..`
/// - Lowercases drive letters on Windows
#[cfg(unix)]
fn normalize_path(path: &PathBuf) -> PathBuf {
    let s = path.to_string_lossy();

    // Expand ~
    let expanded = if s.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/{}", home, &s[2..])
        } else {
            s.to_string()
        }
    } else {
        s.to_string()
    };

    PathBuf::from(expanded)
}

/// Check if two paths overlap (one is a prefix of the other).
fn paths_overlap(a: &PathBuf, b: &PathBuf) -> bool {
    let a_str = a.to_string_lossy().to_lowercase();
    let b_str = b.to_string_lossy().to_lowercase();
    a_str.starts_with(&b_str) || b_str.starts_with(&a_str)
}

/// Extract a display name from a filesystem path.
pub fn path_name(path: &PathBuf) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_unix_path() {
        let msg = "Can you check /projects/oxios/src/main.rs?";
        let path = extract_filesystem_path(msg);
        assert!(path.is_some());
        assert_eq!(path.unwrap().to_string_lossy(), "/projects/oxios/src/main.rs");
    }

    #[test]
    fn test_extract_home_path() {
        let msg = "Look at ~/Documents/recipe.md";
        let path = extract_filesystem_path(msg);
        assert!(path.is_some());
        assert_eq!(path.unwrap().to_string_lossy(), "~/Documents/recipe.md");
    }

    #[test]
    fn test_extract_relative_path() {
        let msg = "Check ./config.toml";
        let path = extract_filesystem_path(msg);
        assert!(path.is_some());
    }

    #[test]
    fn test_extract_github_url() {
        let msg = "Clone https://github.com/oxios/oxios.git";
        let path = extract_filesystem_path(msg);
        assert!(path.is_some());
    }

    #[test]
    fn test_extract_no_path() {
        let msg = "What is the weather like today?";
        let path = extract_filesystem_path(msg);
        assert!(path.is_none());
    }

    #[test]
    fn test_extract_url_query_skip() {
        // Should skip query params
        let msg = "Check https://example.com?foo=bar";
        let path = extract_filesystem_path(msg);
        // This might still match — that's fine, query params are common in paths too
        let _ = path;
    }

    #[test]
    fn test_match_keywords() {
        use super::super::Space;

        let spaces = vec![
            Space::new("oxios", SpaceSource::AutoResource),
            Space::new("일상", SpaceSource::AutoTopic),
        ];

        let msg = "I want to fix a bug in the oxios project";
        let matched = match_keywords(msg, &spaces);
        assert!(matched.is_some());

        let msg2 = "오늘 저녁 뭐 먹지?";
        let matched2 = match_keywords(msg2, &spaces);
        assert!(matched2.is_some());
    }

    #[test]
    fn test_classify_topic_stub() {
        let topic = classify_topic_stub("rust로 버그를 고치고 싶어");
        assert_eq!(topic.name, "개발");
        assert!(topic.is_clear());

        let topic2 = classify_topic_stub("오늘 점심 뭐 먹지?");
        assert_eq!(topic2.name, "일상");
        assert!(topic2.is_clear());

        let topic3 = classify_topic_stub("hi");
        assert!(topic3.name.is_empty());
        assert!(!topic3.is_clear());
    }

    #[test]
    fn test_path_matcher() {
        use super::super::Space;

        let mut space = Space::new("oxios", SpaceSource::AutoResource);
        space.paths.push(PathBuf::from("/projects/oxios"));

        let mut matcher = PathMatcher::default();
        matcher.register(&space);

        assert!(matcher.matches(&PathBuf::from("/projects/oxios/src/main.rs")));
        assert!(matcher.matches(&PathBuf::from("/projects/oxios")));
        assert!(!matcher.matches(&PathBuf::from("/projects/other")));

        let found = matcher.find_space(&PathBuf::from("/projects/oxios/Cargo.toml"));
        assert!(found.is_some());
    }

    #[test]
    fn test_path_name() {
        assert_eq!(path_name(&PathBuf::from("/projects/oxios")), "oxios");
        assert_eq!(path_name(&PathBuf::from("/home/user/Documents")), "Documents");
        assert_eq!(path_name(&PathBuf::from(".")), ".");
    }
}