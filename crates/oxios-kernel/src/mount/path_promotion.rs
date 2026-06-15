//! Path promotion: auto-create Mounts for frequently-used paths (RFC-025 Phase 5).
//!
//! Unlike runtime detection (which only matches *existing* Mounts), this
//! module scans session history to find paths the agent has *actually worked
//! on* (via tool calls) or the user has *explicitly mentioned*, counts them,
//! and promotes paths that cross a frequency threshold into Mounts.
//!
//! ## Pipeline
//!
//! 1. **Extract** raw paths from session trajectories (`tool_args`) and user
//!    messages.
//! 2. **Normalize** each path to its project root by walking up the directory
//!    tree until a marker file (`Cargo.toml`, `package.json`, `.git`, …) is
//!    found. This collapses `/proj/src/main.rs` and `/proj/Cargo.toml` into
//!    the single root `/proj`.
//! 3. **Count** normalized roots over a sliding window.
//! 4. **Promote** roots that cross the threshold into Mounts (unless one
//!    already covers that root).
//!
//! ## Why background, not realtime
//!
//! Every extraction requires walking the filesystem (to find markers), so we
//! run this on a cadence (alongside Dream consolidation) rather than on every
//! message. The threshold naturally debounces one-off mentions.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};

use crate::state_store::Session;

/// Markers whose presence indicates a project root. A directory containing
/// any of these is treated as a self-contained project.
///
/// Mirrors `meta_detection::MARKERS` but adds `.git` (VCS root) and is
/// intentionally kept separate so promotion logic can evolve independently.
const ROOT_MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "go.mod",
    "pyproject.toml",
    "setup.py",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Gemfile",
    "composer.json",
    "mix.exs",
    "CMakeLists.txt",
    "Makefile",
    "AGENTS.md",
    ".git",
];

/// A path's frequency tally within the promotion window.
#[derive(Debug, Clone, Default)]
pub struct PathFrequency {
    /// Number of times a path under this root was touched/mentioned.
    pub count: usize,
    /// Most recent timestamp among the contributing events.
    pub last_seen: Option<DateTime<Utc>>,
}

/// Configuration for path promotion (mirrors `MountsConfig`).
#[derive(Debug, Clone)]
pub struct PromotionConfig {
    /// Disable promotion entirely.
    pub enabled: bool,
    /// Minimum distinct touches within the window to trigger promotion.
    pub threshold: usize,
    /// How far back to look (days). Events older than this are ignored.
    pub window_days: i64,
}

impl Default for PromotionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 3,
            window_days: 14,
        }
    }
}

/// Extract raw path strings from a single session.
///
/// Sources (most-trusted first):
/// - `tool_args` of trajectory steps (paths the agent actually operated on)
/// - user messages (explicit mentions)
///
/// Each returned `(path, timestamp)` pair is one "touch". A single message
/// may contribute several touches if it references several paths.
pub fn extract_paths(session: &Session) -> Vec<(String, DateTime<Utc>)> {
    let mut out = Vec::new();

    // Trajectory tool_args: pull any string value that looks like a path.
    for step in &session.trajectory_steps {
        collect_path_like_strings(&step.tool_args, &mut out, step.timestamp);
    }

    // User messages: word-level path extraction.
    for msg in &session.user_messages {
        for word in msg.content.split_whitespace() {
            if looks_like_path(word) {
                out.push((word.trim_matches(punct).to_string(), msg.timestamp));
            }
        }
    }

    out
}

/// Normalize a raw path to its project root, then tally frequencies.
///
/// Returns a map of `root -> PathFrequency` restricted to events within the
/// configured window.
///
/// Frequency is computed **per distinct root per session**: a single session
/// that mentions the same project root ten times counts once, not ten times.
/// This prevents one chatty session from inflating a root across the
/// threshold (Promo-7).
pub fn tally_frequencies(
    sessions: &[Session],
    config: &PromotionConfig,
) -> HashMap<PathBuf, PathFrequency> {
    let cutoff = Utc::now() - Duration::days(config.window_days);
    let mut freqs: HashMap<PathBuf, PathFrequency> = HashMap::new();

    for session in sessions {
        // Deduplicate roots within a single session before counting: each
        // distinct root contributes at most one touch per session.
        let mut distinct_roots: HashSet<PathBuf> = HashSet::new();
        let mut root_last_seen: HashMap<PathBuf, DateTime<Utc>> = HashMap::new();
        for (raw, ts) in extract_paths(session) {
            if ts < cutoff {
                continue;
            }
            let Some(root) = normalize_to_root(Path::new(&raw)) else {
                continue;
            };
            distinct_roots.insert(root.clone());
            // Track the most recent touch for this root in this session.
            root_last_seen
                .entry(root)
                .and_modify(|prev| *prev = (*prev).max(ts))
                .or_insert(ts);
        }

        for root in distinct_roots {
            let ts = root_last_seen[&root];
            let entry = freqs.entry(root).or_default();
            entry.count += 1; // +1 per distinct root per session
            entry.last_seen = Some(
                entry
                    .last_seen
                    .map_or(ts, |prev: DateTime<Utc>| prev.max(ts)),
            );
        }
    }

    freqs
}

/// Find the project root for `path` by walking up until a marker is found.
///
/// - If `path` itself is a directory containing a marker, it is its own root.
/// - Otherwise we walk up ancestors looking for the first directory that
///   contains a marker.
/// - Returns `None` if no marker is found within the filesystem (e.g. the
///   path doesn't exist, or it's a loose file with no project context).
pub fn normalize_to_root(path: &Path) -> Option<PathBuf> {
    // Expand a leading `~` to the home directory before canonicalizing so
    // that home-relative paths (`~/projects/foo`) resolve correctly (Promo-4).
    // `std::fs::canonicalize` does *not* expand `~`, so without this those
    // paths would fall back to the raw form and fail to find markers.
    let expanded = expand_tilde(path);

    // Canonicalize to resolve `..` and symlinks. Fall back to the raw path
    // if the file no longer exists (it may still be a meaningful prefix).
    let canonical = std::fs::canonicalize(&expanded).unwrap_or(expanded);

    // Start from the path itself if it's a directory, else from its parent.
    let start = if canonical.is_dir() {
        canonical.clone()
    } else {
        canonical.parent()?.to_path_buf()
    };

    // Walk up looking for a marker.
    let mut candidate = Some(start.as_path());
    while let Some(dir) = candidate {
        if has_marker(dir) {
            return Some(dir.to_path_buf());
        }
        candidate = dir.parent();
    }
    None
}

/// Returns `true` if `dir` contains any root marker file.
fn has_marker(dir: &Path) -> bool {
    ROOT_MARKERS.iter().any(|m| dir.join(m).exists())
}

/// Expand a leading `~` (or `~user`, though only `~` is common) to the home
/// directory. Returns the original path unchanged if it doesn't start with `~`
/// or if `HOME` is unavailable (Promo-4).
fn expand_tilde(path: &Path) -> PathBuf {
    expand_tilde_with_home(path, std::env::var_os("HOME"))
}

/// Pure helper: same as [`expand_tilde`] but with the home directory passed
/// in explicitly. Split out so tests can exercise the prefix logic without
/// mutating the process-wide `HOME` environment variable (Promo-4).
fn expand_tilde_with_home(path: &Path, home: Option<std::ffi::OsString>) -> PathBuf {
    let s = path.to_string_lossy();
    let Some(home) = home else {
        // No HOME → leave `~` paths untouched.
        return path.to_path_buf();
    };
    if s == "~" {
        return PathBuf::from(home);
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return PathBuf::from(home).join(rest);
    }
    path.to_path_buf()
}

/// Collect path-like string values from a JSON value (recursively).
///
/// `depth` bounds the recursion to prevent stack overflow on pathologically
/// nested JSON (Promo-10). The bound of 32 comfortably exceeds any
/// realistic tool_args payload.
fn collect_path_like_strings(
    value: &serde_json::Value,
    out: &mut Vec<(String, DateTime<Utc>)>,
    ts: DateTime<Utc>,
) {
    collect_path_like_strings_inner(value, out, ts, 0);
}

/// Inner recursive worker carrying the current `depth`.
fn collect_path_like_strings_inner(
    value: &serde_json::Value,
    out: &mut Vec<(String, DateTime<Utc>)>,
    ts: DateTime<Utc>,
    depth: u32,
) {
    // Promo-10: bound recursion depth to avoid stack overflow on deeply
    // (or cyclically) nested JSON.
    const MAX_DEPTH: u32 = 32;
    if depth > MAX_DEPTH {
        return;
    }
    match value {
        serde_json::Value::String(s) => {
            if looks_like_path(s) {
                out.push((s.trim_matches(punct).to_string(), ts));
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_path_like_strings_inner(v, out, ts, depth + 1);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_path_like_strings_inner(v, out, ts, depth + 1);
            }
        }
        _ => {}
    }
}

/// Heuristic: does this string look like an absolute or home-relative path?
fn looks_like_path(s: &str) -> bool {
    let s = s.trim_matches(punct);
    // Absolute unix path with at least one separator and some depth.
    (s.starts_with('/') && s.matches('/').count() >= 2 && s.len() > 3)
        || (s.starts_with("~/") && s.len() > 3)
}

/// Characters to strip from the edges of a path-like token (quotes, commas,
/// brackets) before parsing.
fn punct(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '`' | ',' | ';' | ')' | ']' | '}' | '(' | '[' | '{'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_store::{Session, SessionId};

    fn make_session(msgs: Vec<(&str, DateTime<Utc>)>) -> Session {
        let mut s = Session::new("test");
        s.id = SessionId("s1".into());
        for (content, ts) in msgs {
            let mut m = crate::state_store::UserMessage {
                content: content.into(),
                timestamp: ts,
            };
            // Session::add_user_message stamps Utc::now(); override it.
            s.user_messages.push(std::mem::replace(
                &mut m,
                crate::state_store::UserMessage {
                    content: String::new(),
                    timestamp: ts,
                },
            ));
        }
        s
    }

    #[test]
    fn test_looks_like_path() {
        assert!(looks_like_path("/usr/local/bin"));
        assert!(looks_like_path("~/projects/foo"));
        assert!(!looks_like_path("hello world"));
        assert!(!looks_like_path("/x")); // too shallow
        assert!(!looks_like_path("no-slash"));
    }

    /// Path to this crate's root — used instead of a hardcoded developer
    /// path so the tests are portable (Promo-2). It has `Cargo.toml` at its
    /// root, so `normalize_to_root` collapses any child to it.
    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn test_tally_counts_repeated_paths() {
        let now = Utc::now();
        let root = crate_root();
        let sessions = vec![make_session(vec![
            (format!("fix {}/src/lib.rs", root.display()).as_str(), now),
            (
                format!("also check {}/Cargo.toml", root.display()).as_str(),
                now,
            ),
            (format!("again {}", root.display()).as_str(), now),
        ])];

        let config = PromotionConfig {
            threshold: 1,
            ..Default::default()
        };
        let freqs = tally_frequencies(&sessions, &config);

        // All three mentions collapse to the same project root (Cargo.toml).
        let final_segment = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("oxios-kernel");
        let freq = freqs
            .iter()
            .find(|(k, _)| k.ends_with(final_segment))
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("expected root in {:?}", freqs));
        // Promo-7: a single session counts each distinct root once, so the
        // count is exactly 1 regardless of how many times it was mentioned.
        assert_eq!(freq.count, 1);
    }

    #[test]
    fn test_tally_respects_window() {
        let now = Utc::now();
        let old = now - Duration::days(30);
        let root = crate_root();
        // Use a real project root (the crate itself) so that the *only* reason
        // the tally is empty is the window filter, not `normalize_to_root`
        // returning `None` (Promo-2). Three old touches of the same root.
        let sessions = vec![make_session(vec![
            (
                format!("work on {}/src/lib.rs", root.display()).as_str(),
                old,
            ),
            (
                format!("work on {}/Cargo.toml", root.display()).as_str(),
                old,
            ),
            (format!("work on {}", root.display()).as_str(), old),
        ])];
        let config = PromotionConfig {
            window_days: 14,
            ..Default::default()
        };
        let freqs = tally_frequencies(&sessions, &config);
        // Old events outside the window must not appear.
        assert!(freqs.is_empty(), "expected empty freqs, got {:?}", freqs);
    }

    #[test]
    fn test_normalize_collapses_files_to_root() {
        // The oxios repo itself has Cargo.toml at its root.
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
        let root = normalize_to_root(&file).expect("should find root");
        assert!(root.ends_with("oxios-kernel"));
    }

    #[test]
    fn test_normalize_expands_tilde() {
        // Promo-4: test the pure prefix-expansion logic without touching the
        // process environment (avoids unsafe set_var + parallel-test races).
        let home = std::ffi::OsString::from("/Users/test");
        // `~/foo` → `$HOME/foo`
        assert_eq!(
            expand_tilde_with_home(Path::new("~/foo"), Some(home.clone())),
            PathBuf::from("/Users/test/foo")
        );
        // bare `~` → `$HOME`
        assert_eq!(
            expand_tilde_with_home(Path::new("~"), Some(home.clone())),
            PathBuf::from("/Users/test")
        );
        // absolute path unchanged
        assert_eq!(
            expand_tilde_with_home(Path::new("/etc/passwd"), Some(home.clone())),
            PathBuf::from("/etc/passwd")
        );
        // relative path unchanged
        assert_eq!(
            expand_tilde_with_home(Path::new("relative/path"), Some(home.clone())),
            PathBuf::from("relative/path")
        );
        // No HOME available → `~` paths pass through untouched.
        assert_eq!(
            expand_tilde_with_home(Path::new("~/foo"), None),
            PathBuf::from("~/foo")
        );
    }

    #[test]
    fn test_collect_path_like_bounds_recursion_depth() {
        // Promo-10: deeply nested JSON must not overflow the stack.
        // Build a value nested far beyond MAX_DEPTH and confirm collection
        // returns without panicking.
        let mut value = serde_json::json!({"path": "/usr/local/bin"});
        for _ in 0..100 {
            value = serde_json::json!({ "nested": value });
        }
        let mut out = Vec::new();
        collect_path_like_strings(&value, &mut out, Utc::now());
        // The deep path is unreachable (>32 levels) so nothing is collected.
        assert!(out.is_empty(), "expected no paths past depth bound");

        // A shallow path IS collected.
        let shallow = serde_json::json!({
            "a": { "b": { "file": "/usr/local/bin/oxios" } }
        });
        let mut out2 = Vec::new();
        collect_path_like_strings(&shallow, &mut out2, Utc::now());
        assert_eq!(out2.len(), 1);
        assert_eq!(out2[0].0, "/usr/local/bin/oxios");
    }
}
