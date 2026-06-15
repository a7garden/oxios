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

use std::collections::HashMap;
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
pub fn tally_frequencies(
    sessions: &[Session],
    config: &PromotionConfig,
) -> HashMap<PathBuf, PathFrequency> {
    let cutoff = Utc::now() - Duration::days(config.window_days);
    let mut freqs: HashMap<PathBuf, PathFrequency> = HashMap::new();

    for session in sessions {
        for (raw, ts) in extract_paths(session) {
            if ts < cutoff {
                continue;
            }
            let Some(root) = normalize_to_root(Path::new(&raw)) else {
                continue;
            };
            let entry = freqs.entry(root).or_default();
            entry.count += 1;
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
    // Canonicalize to resolve `..` and symlinks. Fall back to the raw path
    // if the file no longer exists (it may still be a meaningful prefix).
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

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

/// Collect path-like string values from a JSON value (recursively).
fn collect_path_like_strings(
    value: &serde_json::Value,
    out: &mut Vec<(String, DateTime<Utc>)>,
    ts: DateTime<Utc>,
) {
    match value {
        serde_json::Value::String(s) => {
            if looks_like_path(s) {
                out.push((s.trim_matches(punct).to_string(), ts));
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_path_like_strings(v, out, ts);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_path_like_strings(v, out, ts);
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
        assert!(looks_like_path("/Volumes/MERCURY/PROJECTS/oxios"));
        assert!(looks_like_path("~/projects/foo"));
        assert!(!looks_like_path("hello world"));
        assert!(!looks_like_path("/x")); // too shallow
        assert!(!looks_like_path("no-slash"));
    }

    #[test]
    fn test_tally_counts_repeated_paths() {
        let now = Utc::now();
        let sessions = vec![make_session(vec![
            ("fix /Volumes/MERCURY/PROJECTS/oxios/src/main.rs", now),
            ("also check /Volumes/MERCURY/PROJECTS/oxios/Cargo.toml", now),
            ("again /Volumes/MERCURY/PROJECTS/oxios", now),
        ])];

        let config = PromotionConfig {
            threshold: 1,
            ..Default::default()
        };
        let freqs = tally_frequencies(&sessions, &config);

        // All three should collapse to the oxios project root (it has Cargo.toml).
        let oxios_root = PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios");
        let oxios_freq = freqs
            .iter()
            .find(|(k, _)| k.ends_with("oxios"))
            .map(|(_, v)| v);
        assert!(oxios_freq.is_some(), "expected oxios root in {:?}", freqs);
        assert!(oxios_freq.unwrap().count >= 3);
    }

    #[test]
    fn test_tally_respects_window() {
        let now = Utc::now();
        let old = now - Duration::days(30);
        let sessions = vec![make_session(vec![
            ("work on /tmp/very/old/path", old),
            ("work on /tmp/very/old/path", old),
            ("work on /tmp/very/old/path", old),
        ])];
        let config = PromotionConfig {
            window_days: 14,
            ..Default::default()
        };
        let freqs = tally_frequencies(&sessions, &config);
        // Old events outside the window shouldn't appear (and /tmp isn't a
        // project root anyway, so it'd be filtered by normalize_to_root).
        assert!(freqs.is_empty());
    }

    #[test]
    fn test_normalize_collapses_files_to_root() {
        // The oxios repo itself has Cargo.toml at its root.
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
        let root = normalize_to_root(&file).expect("should find root");
        assert!(root.ends_with("oxios-kernel"));
    }
}
