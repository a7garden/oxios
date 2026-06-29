//! Shared skill-archive extraction.
//!
//! Stateless helpers for extracting skill `.zip` archives into a target
//! directory. Both the ClawHub marketplace installer and the user-driven
//! `.skill` file import use these so that the Zip-Slip defense
//! ([`crate::skill::is_safe_relative_path`]) and the `SKILL.md` marker
//! detection live in exactly one place.

use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::ZipArchive;

/// Marker filenames that identify a skill root, in order of preference.
const MARKERS: &[&str] = &["SKILL.md", "skill.md", "skills.md"];

/// Find the directory prefix inside the archive that contains a `SKILL.md`
/// marker. Returns `""` when the marker sits at the archive root.
///
/// Some archives zip the skill directory directly (`code-review/SKILL.md`),
/// others zip its contents (`SKILL.md`). This detects the common prefix so
/// extraction can strip it and land contents flat in `target`.
pub fn find_skill_root<R: Read + Seek>(zip: &mut ZipArchive<R>) -> Result<String> {
    for i in 0..zip.len() {
        let name = zip
            .by_index(i)
            .context("read zip entry")?
            .name()
            .to_string();
        let name_lower = name.to_lowercase();
        if MARKERS.iter().any(|m| {
            name_lower.ends_with(&format!("/{}", m.to_lowercase()))
                || name_lower == m.to_lowercase()
        }) {
            // Return everything up to and including the directory component.
            if let Some(slash) = name
                .strip_prefix('/')
                .and_then(|s| s.rfind('/'))
                .map(|p| p + 1)
            {
                return Ok(name[..slash].to_string());
            }
            // File is at root level.
            if let Some(last_slash) = name.rfind('/') {
                return Ok(name[..=last_slash].to_string());
            }
            // No slash — the root is the archive root (empty prefix).
            return Ok(String::new());
        }
    }

    // Fallback: no marker found — extract everything at root.
    tracing::warn!("no SKILL.md marker found in archive, extracting all entries");
    Ok(String::new())
}

/// Extract a skill zip into `target`, stripping the detected skill-root prefix
/// so contents land flat in `target`. Defends against Zip Slip via
/// [`crate::skill::is_safe_relative_path`].
pub fn extract_skill_zip<R: Read + Seek>(zip: &mut ZipArchive<R>, target: &Path) -> Result<()> {
    let root_prefix = find_skill_root(zip).context("parse zip archive")?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).context("read zip entry")?;
        let name = file.name().to_string();

        // Strip the detected root prefix.
        let relative = match name.strip_prefix(&root_prefix) {
            Some(rest) => rest.to_string(),
            None => continue, // entry outside the root — skip
        };

        // Normalize separators.
        let relative = relative.replace('\\', "/");
        if relative.is_empty() || relative == "/" {
            continue;
        }

        // Zip Slip defense: reject entries whose relative path could escape
        // `target` (absolute paths, `..`, drive prefixes).
        if !crate::skill::is_safe_relative_path(&relative) {
            tracing::warn!("skill archive: skipping entry with unsafe path: {relative}");
            continue;
        }
        let out_path = target.join(&relative);

        if file.is_dir() {
            fs::create_dir_all(&out_path).context("create extracted dir")?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).context("create parent dir")?;
            }
            let mut dst = fs::File::create(&out_path).context("create output file")?;
            std::io::copy(&mut file, &mut dst).context("copy zip entry")?;
        }
    }

    Ok(())
}

/// Locate the first `SKILL.md` (case-insensitive) within `dir` recursively.
/// Returns its path, or `None` if no marker is present.
pub fn find_skill_md(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| matches!(n.to_lowercase().as_str(), "skill.md"))
                .unwrap_or(false)
            {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn build_zip(entries: &[(&str, &[u8])]) -> ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut buf = Vec::new();
        {
            let mut zipw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, data) in entries {
                zipw.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zipw.write_all(data).unwrap();
            }
            zipw.finish().unwrap();
        }
        zip::ZipArchive::new(std::io::Cursor::new(buf)).unwrap()
    }

    #[test]
    fn test_find_skill_root_nested() {
        let mut arch = build_zip(&[("code-review/SKILL.md", b"# Code Review\n")]);
        assert_eq!(find_skill_root(&mut arch).unwrap(), "code-review/");
    }

    #[test]
    fn test_find_skill_root_root_level() {
        let mut arch = build_zip(&[("SKILL.md", b"# Skill\n")]);
        assert_eq!(find_skill_root(&mut arch).unwrap(), "");
    }

    #[test]
    fn test_extract_flattens_prefix() {
        // Nested skill dir → contents land flat in target.
        let mut arch = build_zip(&[
            ("my-skill/SKILL.md", b"# Skill\n"),
            ("my-skill/scripts/run.sh", b"#!/bin/sh\n"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        extract_skill_zip(&mut arch, tmp.path()).unwrap();
        assert!(tmp.path().join("SKILL.md").exists());
        assert!(tmp.path().join("scripts/run.sh").exists());
        assert!(!tmp.path().join("my-skill").exists()); // prefix stripped
    }

    #[test]
    fn test_find_skill_md_walks_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("a/b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("SKILL.md"), b"# x\n").unwrap();
        let found = find_skill_md(tmp.path()).unwrap();
        assert!(found.ends_with("SKILL.md"));
    }

    #[test]
    fn test_find_skill_md_none() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("README.md"), b"nope\n").unwrap();
        assert!(find_skill_md(tmp.path()).is_none());
    }
}
