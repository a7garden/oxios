//! Auto-meta detection: cheap heuristics on marker files (RFC-025 §Auto-Meta).
//!
//! Seeds [`MountMeta`](super::MountMeta) from filesystem markers, then the
//! agent refines it during enrichment. Detection runs at drift-detection time
//! (cheap `stat` + tiny reads), not on every message.

use std::path::Path;

use super::MountMeta;

/// Marker files that imply a language / stack. Checked against a Mount's
/// primary path.
const MARKERS: &[(&str, &str)] = &[
    ("Cargo.toml", "rust"),
    ("package.json", "typescript"),
    ("go.mod", "go"),
    ("pyproject.toml", "python"),
    ("requirements.txt", "python"),
    ("setup.py", "python"),
    ("pom.xml", "java"),
    ("build.gradle", "java"),
    ("build.gradle.kts", "kotlin"),
    ("Gemfile", "ruby"),
    ("composer.json", "php"),
    ("mix.exs", "elixir"),
    ("CMakeLists.txt", "cpp"),
    ("Makefile", "c"),
];

/// Docs/agent markers — recorded but don't imply a language.
const DOC_MARKERS: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    ".cursorrules",
    "README.md",
    "GEMINI.md",
    ".windsurfrules",
];

/// Structure hints from top-level directories.
const STRUCTURE_HINTS: &[(&str, &str)] = &[
    ("crates", "cargo-workspace"),
    ("packages", "monorepo"),
    ("apps", "monorepo"),
    ("libs", "monorepo"),
];

/// Detect [`MountMeta`] from the filesystem at `path`.
///
/// This is a **draft** — the agent refines it during enrichment. We never make
/// an LLM call here; everything is cheap `stat`/`read` on small files.
pub fn detect_meta(path: &Path) -> MountMeta {
    let mut meta = MountMeta::default();

    let mut found_languages: Vec<String> = Vec::new();
    let mut found_markers: Vec<String> = Vec::new();

    // Language + stack markers.
    for (marker, lang) in MARKERS {
        let marker_path = path.join(marker);
        if marker_path.is_file() {
            if !found_languages.contains(&lang.to_string()) {
                found_languages.push(lang.to_string());
            }
            found_markers.push(marker.to_string());

            // Extract stack hints for well-known markers.
            extract_stack(marker, &marker_path, &mut meta.stack);
        }
    }

    // Doc / agent markers (no language, but recorded + seed summary).
    for marker in DOC_MARKERS {
        let marker_path = path.join(marker);
        if marker_path.is_file() {
            found_markers.push(marker.to_string());
            // AGENTS.md / README.md seed the summary (first paragraph).
            if (marker == &"AGENTS.md" || marker == &"README.md") && meta.summary.is_empty() {
                if let Ok(content) = std::fs::read_to_string(&marker_path) {
                    meta.summary = first_meaningful_line(&content);
                }
            }
        }
    }

    // Structure hints.
    for (dir, hint) in STRUCTURE_HINTS {
        if path.join(dir).is_dir() && !meta.stack.contains(&hint.to_string()) {
            meta.stack.push(hint.to_string());
        }
    }

    meta.languages = found_languages;
    meta.markers = found_markers;

    // If no summary yet, derive one from languages.
    if meta.summary.is_empty() && !meta.languages.is_empty() {
        meta.summary = meta.languages.join(" + ");
    }

    meta
}

/// Compute the set of marker files to watch for drift, given a path.
///
/// Returns `(path, mtime)` pairs for existing markers — this is the snapshot
/// the drift detector compares against on the next session.
pub fn snapshot_markers(
    path: &Path,
) -> Vec<(std::path::PathBuf, std::time::SystemTime)> {
    let all: Vec<&str> = MARKERS
        .iter()
        .map(|(m, _)| *m)
        .chain(DOC_MARKERS.iter().copied())
        .collect();

    all.into_iter()
        .filter_map(|m| {
            let p = path.join(m);
            p.metadata().and_then(|md| md.modified()).ok().map(|t| (p, t))
        })
        .collect()
}

/// Extract stack keywords from a marker file's contents.
///
/// Reads only the marker file (small), scans for dependency names. Keeps the
/// result bounded — at most ~8 entries.
fn extract_stack(marker: &str, path: &Path, stack: &mut Vec<String>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let push = |stack: &mut Vec<String>, s: &str| {
        if s.len() >= 2 && !stack.iter().any(|e| e.eq_ignore_ascii_case(s)) {
            stack.push(s.to_string());
        }
    };

    match marker {
        "Cargo.toml" => {
            // Look for crate names in [dependencies] / [workspace.dependencies].
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(name) = trimmed.strip_suffix(" = ") {
                    push(stack, name.trim());
                } else if let Some(eq_pos) = trimmed.find('=') {
                    let name = trimmed[..eq_pos].trim();
                    // Only dependency-style lines (not section headers).
                    if !name.starts_with('[')
                        && !name.is_empty()
                        && !["path", "version", "features", "default-features", "optional"]
                            .contains(&name)
                    {
                        push(stack, name);
                    }
                }
            }
        }
        "package.json" => {
            // Parse JSON, pull keys from dependencies + devDependencies.
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                for key in &["dependencies", "devDependencies", "peerDependencies"] {
                    if let Some(obj) = val.get(key).and_then(|v| v.as_object()) {
                        for dep in obj.keys() {
                            push(stack, dep);
                        }
                    }
                }
            }
        }
        "go.mod" => {
            // Lines like `\tgithub.com/foo/bar v1.2.3`.
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("require ") || trimmed.contains(" v") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    for part in parts {
                        if part.contains('/') && part.contains('.') && !part.starts_with("require") {
                            // Take the last path segment as the stack name.
                            if let Some(name) = part.rsplit('/').next() {
                                push(stack, name);
                            }
                        }
                    }
                }
            }
        }
        "pyproject.toml" | "requirements.txt" => {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
                    continue;
                }
                let name = trimmed
                    .split(['=', '<', '>', ';', '[', ' '])
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    push(stack, name);
                }
            }
        }
        _ => {}
    }

    // Bound the stack list.
    stack.truncate(8);
}

/// Take the first non-heading, non-empty line as a summary seed.
fn first_meaningful_line(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") {
            continue;
        }
        // Strip markdown emphasis for a cleaner summary.
        let clean = trimmed
            .trim_start_matches('>')
            .replace("**", "")
            .replace('*', "")
            .replace('`', "");
        let clean = clean.trim();
        let capped = if clean.len() > 120 { &clean[..120] } else { clean };
        // Find a safe UTF-8 boundary.
        let mut end = capped.len();
        while end > 0 && !capped.is_char_boundary(end) {
            end -= 1;
        }
        let safe = &capped[..end];
        if clean.len() > 120 {
            return format!("{}…", safe);
        }
        return safe.to_string();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust_project() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"oxios\"\n\n[dependencies]\ntokio = \"1\"\nserde = \"1\"\naxum = \"0.7\"\n",
        )
        .unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Oxios\nAgent OS in Rust.").unwrap();

        let meta = detect_meta(dir.path());
        assert!(meta.languages.contains(&"rust".to_string()));
        assert!(meta.markers.contains(&"Cargo.toml".to_string()));
        assert!(meta.markers.contains(&"AGENTS.md".to_string()));
        assert!(meta.stack.iter().any(|s| s == "tokio"));
        assert!(meta.stack.iter().any(|s| s == "axum"));
        assert!(!meta.summary.is_empty());
    }

    #[test]
    fn test_detect_node_project() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"react": "^18", "next": "^14"}, "devDependencies": {"typescript": "^5"}}"#,
        )
        .unwrap();

        let meta = detect_meta(dir.path());
        assert!(meta.languages.contains(&"typescript".to_string()));
        assert!(meta.stack.iter().any(|s| s == "react"));
        assert!(meta.stack.iter().any(|s| s == "next"));
    }

    #[test]
    fn test_detect_empty_dir() {
        let dir = TempDir::new().unwrap();
        let meta = detect_meta(dir.path());
        assert!(meta.languages.is_empty());
        assert!(meta.markers.is_empty());
        assert!(meta.summary.is_empty());
    }

    #[test]
    fn test_structure_hints() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("crates")).unwrap();
        let meta = detect_meta(dir.path());
        assert!(meta.stack.contains(&"cargo-workspace".to_string()));
    }

    #[test]
    fn test_snapshot_markers() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let snap = snapshot_markers(dir.path());
        assert!(snap.iter().any(|(p, _)| p.file_name().unwrap() == "Cargo.toml"));
        // Non-existent markers are excluded.
        assert!(!snap.iter().any(|(p, _)| p.file_name().unwrap() == "package.json"));
    }

    #[test]
    fn test_first_meaningful_line() {
        assert_eq!(
            first_meaningful_line("# Title\n\nThis is the **summary**.\nMore."),
            "This is the summary."
        );
    }
}
