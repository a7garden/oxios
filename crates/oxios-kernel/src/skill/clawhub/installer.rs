#![allow(missing_docs)]
//! ClawHub skill installer.
//!
//! Handles install, update, and update-all workflows for ClawHub skills,
//! including origin tracking and lockfile management.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use super::client::{ClawHubClient, DownloadedArchive};
use super::types::{ClawHubLockEntry, ClawHubLockfile, ClawHubOrigin};

/// Installation result returned to callers.
#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub ok: bool,
    pub slug: String,
    pub version: String,
    pub target_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog: Option<String>,
}

/// Update result for a single skill.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateResult {
    pub ok: bool,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
    pub version: String,
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Summary of an available update.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateAvailable {
    pub slug: String,
    pub current_version: String,
    pub latest_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog: Option<String>,
}

/// ClawHub skill installer.
pub struct ClawHubInstaller {
    client: ClawHubClient,
    /// Directory where skills are installed (e.g. ~/.oxios/skills/).
    skills_dir: PathBuf,
    /// Workspace root directory — lockfile lives at `{workspace_dir}/.clawhub/lock.json`.
    workspace_dir: PathBuf,
}

impl ClawHubInstaller {
    /// Create a new installer.
    ///
    /// - `skills_dir` — parent directory holding each skill subdirectory.
    /// - `workspace_dir` — root of the workspace; `.clawhub/` is created here for the lockfile.
    pub fn new(skills_dir: PathBuf, workspace_dir: PathBuf, base_url: Option<String>) -> Self {
        Self {
            client: ClawHubClient::new(base_url).expect("valid ClawHub base URL"),
            skills_dir,
            workspace_dir,
        }
    }

    /// Install a skill from ClawHub.
    ///
    /// If `version` is `None`, the latest version is resolved from the API.
    pub async fn install(&self, slug: &str, version: Option<&str>) -> Result<InstallResult> {
        // Resolve version from API if not specified
        let version = match version {
            Some(v) => v.to_string(),
            None => {
                let detail = self.client.get_skill(slug).await?;
                detail
                    .latest_version
                    .as_ref()
                    .map(|v| v.version.clone())
                    .unwrap_or_else(|| "latest".to_string())
            }
        };

        // Download
        let archive = self.client.download_skill(slug, Some(&version)).await?;
        let target_dir = self.skills_dir.join(slug);

        if target_dir.exists() {
            anyhow::bail!("skill already installed: {slug} (use update to reinstall)");
        }

        // Extract
        fs::create_dir_all(&target_dir).context("create skills_dir")?;
        self.extract_archive(&archive, &target_dir)?;

        // Origin file
        let origin = ClawHubOrigin {
            version: 1,
            registry: self.client.base_url().to_string(),
            slug: slug.to_string(),
            installed_version: version.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            sha256: Some(archive.sha256.clone()),
        };
        let origin_path = target_dir.join(".clawhub").join("origin.json");
        fs::create_dir_all(origin_path.parent().unwrap())?;
        fs::write(
            &origin_path,
            serde_json::to_string_pretty(&origin).context("serialize origin")?,
        )?;

        // Update lockfile
        self.update_lockfile(slug, &version)?;

        let changelog = self
            .client
            .get_skill(slug)
            .await?
            .latest_version
            .as_ref()
            .and_then(|v| v.changelog.clone());

        Ok(InstallResult {
            ok: true,
            slug: slug.to_string(),
            version,
            target_dir,
            changelog,
        })
    }

    /// Update a specific installed skill to the latest version.
    pub async fn update(&self, slug: &str) -> Result<UpdateResult> {
        let current = self.get_installed_version(slug).ok();

        let detail = self.client.get_skill(slug).await?;
        let latest = detail
            .latest_version
            .as_ref()
            .map(|v| v.version.clone())
            .unwrap_or_else(|| "latest".to_string());

        // No-op if already at latest
        if current.as_deref() == Some(&latest) {
            return Ok(UpdateResult {
                ok: true,
                slug: slug.to_string(),
                previous_version: current,
                version: latest,
                changed: false,
                error: None,
            });
        }

        // Download and extract
        let archive = self.client.download_skill(slug, Some(&latest)).await?;
        let target_dir = self.skills_dir.join(slug);

        // Remove old directory and re-extract
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).context("remove old skill dir")?;
        }
        fs::create_dir_all(&target_dir).context("create skills_dir")?;
        self.extract_archive(&archive, &target_dir)?;

        // Update origin file
        let origin = ClawHubOrigin {
            version: 1,
            registry: self.client.base_url().to_string(),
            slug: slug.to_string(),
            installed_version: latest.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            sha256: Some(archive.sha256.clone()),
        };
        let origin_path = target_dir.join(".clawhub").join("origin.json");
        fs::create_dir_all(origin_path.parent().unwrap())?;
        fs::write(
            &origin_path,
            serde_json::to_string_pretty(&origin).context("serialize origin")?,
        )?;
        self.update_lockfile(slug, &latest)?;

        Ok(UpdateResult {
            ok: true,
            slug: slug.to_string(),
            previous_version: current,
            version: latest,
            changed: true,
            error: None,
        })
    }

    /// Update all installed ClawHub skills.
    pub async fn update_all(&self) -> Result<Vec<UpdateResult>> {
        let lock = self.read_lockfile()?;
        let mut results = Vec::with_capacity(lock.skills.len());

        for (slug, entry) in lock.skills {
            let result = match self.update(&slug).await {
                Ok(r) => r,
                Err(e) => UpdateResult {
                    ok: false,
                    slug,
                    previous_version: Some(entry.version),
                    version: String::new(),
                    changed: false,
                    error: Some(e.to_string()),
                },
            };
            results.push(result);
        }

        Ok(results)
    }

    /// Check which installed skills have updates available.
    ///
    /// Fetches skill details concurrently for lower latency.
    pub async fn check_updates(&self) -> Result<Vec<UpdateAvailable>> {
        let lock = self.read_lockfile()?;
        let skills: Vec<(String, ClawHubLockEntry)> = lock.skills.into_iter().collect();

        let futures: Vec<_> = skills
            .into_iter()
            .map(|(slug, entry)| {
                let client = self.client.clone();
                async move {
                    let detail = client.get_skill(&slug).await.ok()?;
                    let latest = detail.latest_version.as_ref()?;
                    if latest.version != entry.version {
                        Some(UpdateAvailable {
                            slug,
                            current_version: entry.version,
                            latest_version: latest.version.clone(),
                            changelog: latest.changelog.clone(),
                        })
                    } else {
                        None
                    }
                }
            })
            .collect();

        let updates: Vec<UpdateAvailable> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        Ok(updates)
    }

    /// Read the lockfile from `{workspace_dir}/.clawhub/lock.json`.
    fn read_lockfile(&self) -> Result<ClawHubLockfile> {
        let path = self.lockfile_path();
        if !path.exists() {
            return Ok(ClawHubLockfile {
                version: 1,
                skills: HashMap::new(),
            });
        }
        let mut file = fs::File::open(&path).context("open lockfile")?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .context("read lockfile content")?;
        serde_json::from_str(&buf).context("parse lockfile JSON")
    }

    /// Write the lockfile to `{workspace_dir}/.clawhub/lock.json`.
    fn write_lockfile(&self, lock: &ClawHubLockfile) -> Result<()> {
        let path = self.lockfile_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("create .clawhub dir")?;
        }
        let json = serde_json::to_string_pretty(lock).context("serialize lockfile to JSON")?;
        fs::write(&path, json).context("write lockfile")?;
        Ok(())
    }

    /// Path to the lockfile.
    fn lockfile_path(&self) -> PathBuf {
        self.workspace_dir.join(".clawhub").join("lock.json")
    }

    /// Extract a zip archive into the target directory, finding the skill root
    /// that contains `SKILL.md` / `skill.md` / `skills.md`.
    fn extract_archive(&self, archive: &DownloadedArchive, target: &Path) -> Result<()> {
        let file = fs::File::open(&archive.path).context("open downloaded zip")?;
        let mut zip = zip::ZipArchive::new(file)?;

        // Find the root directory inside the zip that contains a SKILL.md marker.
        // Some archives zip the skill directory directly, others zip the contents.
        let root_prefix = self
            .find_skill_root(&mut zip)
            .context("parse zip archive")?;

        // Extract all entries, stripping the root prefix so contents land in `target`.
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).context("read zip entry")?;
            let name = file.name();

            // Strip the detected root prefix
            let relative = if let Some(rest) = name.strip_prefix(&root_prefix) {
                rest.to_string()
            } else {
                // Entry outside the root — skip
                continue;
            };

            // Normalize separators
            let relative = relative.replace('\\', "/");
            if relative.is_empty() || relative == "/" {
                continue;
            }

            // Zip Slip defense: reject entries whose relative path could
            // escape `target` (absolute paths, `..`, drive prefixes).
            if !crate::skill::is_safe_relative_path(&relative) {
                tracing::warn!("clawhub: skipping zip entry with unsafe path: {relative}");
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

    /// Find the directory prefix inside the zip that contains a SKILL.md marker.
    fn find_skill_root<R: std::io::Read + std::io::Seek>(
        &self,
        zip: &mut zip::ZipArchive<R>,
    ) -> Result<String> {
        // MARKER_FILES in order of preference
        const MARKERS: &[&str] = &["SKILL.md", "skill.md", "skills.md"];

        for i in 0..zip.len() {
            let name = zip.by_index(i).unwrap().name().to_string();
            let name_lower = name.to_lowercase();
            if MARKERS.iter().any(|m| {
                name_lower.ends_with(&format!("/{}", m.to_lowercase()))
                    || name_lower == m.to_lowercase()
            }) {
                // Return everything up to and including the directory component
                if let Some(slash) = name
                    .strip_prefix('/')
                    .and_then(|s| s.rfind('/'))
                    .map(|p| p + 1)
                {
                    return Ok(name[..slash].to_string());
                }
                // File is at root level
                if let Some(last_slash) = name.rfind('/') {
                    return Ok(name[..=last_slash].to_string());
                }
                // No slash — the root is the archive root (empty prefix)
                return Ok(String::new());
            }
        }

        // Fallback: no marker found — extract everything at root
        tracing::warn!("no SKILL.md marker found in archive, extracting all entries");
        Ok(String::new())
    }

    /// Update (or insert) a lockfile entry for the given slug.
    fn update_lockfile(&self, slug: &str, version: &str) -> Result<()> {
        let mut lock = self.read_lockfile()?;
        lock.skills.insert(
            slug.to_string(),
            ClawHubLockEntry {
                version: version.to_string(),
                installed_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        self.write_lockfile(&lock)
    }

    /// Read the installed version for a slug from its origin file.
    fn get_installed_version(&self, slug: &str) -> Result<String> {
        let origin_path = self
            .skills_dir
            .join(slug)
            .join(".clawhub")
            .join("origin.json");
        let mut file = fs::File::open(&origin_path).context("open origin.json")?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .context("read origin.json content")?;
        let origin: ClawHubOrigin = serde_json::from_str(&buf).context("parse origin.json")?;
        Ok(origin.installed_version)
    }

    /// Access the underlying ClawHub client.
    pub fn client(&self) -> &ClawHubClient {
        &self.client
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_skill_root() {
        use std::io::Write;
        // Build a zip with SKILL.md nested inside a skill directory
        let mut buf = Vec::new();
        {
            let mut zipw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zipw.start_file(
                "code-review/SKILL.md",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zipw.write_all(b"# Code Review\n").unwrap();
            zipw.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(buf);
        let mut arch = zip::ZipArchive::new(cursor).unwrap();

        let installer = ClawHubInstaller::new(
            PathBuf::from("/tmp/skills"),
            PathBuf::from("/tmp/workspace"),
            None,
        );
        let prefix = installer.find_skill_root(&mut arch).unwrap();
        assert_eq!(prefix, "code-review/");
    }

    #[test]
    fn test_find_skill_root_skips_root_level() {
        use std::io::Write;
        // Some archives may have SKILL.md at root level (no subdirectory)
        let mut buf = Vec::new();
        {
            let mut zipw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zipw.start_file("SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            zipw.write_all(b"# Skill\n").unwrap();
            zipw.finish().unwrap();
        }
        let cursor = std::io::Cursor::new(buf);
        let mut arch = zip::ZipArchive::new(cursor).unwrap();
        let installer = ClawHubInstaller::new(
            PathBuf::from("/tmp/skills"),
            PathBuf::from("/tmp/workspace"),
            None,
        );
        // SKILL.md at root → prefix is empty (extract everything as-is)
        let prefix = installer.find_skill_root(&mut arch).unwrap();
        assert_eq!(prefix, "");
    }

    #[test]
    fn test_install_result_serialize() {
        let res = InstallResult {
            ok: true,
            slug: "test".to_string(),
            version: "1.0.0".to_string(),
            target_dir: PathBuf::from("/tmp/test"),
            changelog: Some("fixes".to_string()),
        };
        let json = serde_json::to_string_pretty(&res).unwrap();
        assert!(json.contains("\"ok\": true"));
        assert!(json.contains("\"slug\": \"test\""));
    }

    #[test]
    fn test_update_result_serialize() {
        let res = UpdateResult {
            ok: true,
            slug: "test".to_string(),
            previous_version: Some("1.0.0".to_string()),
            version: "2.0.0".to_string(),
            changed: true,
            error: None,
        };
        let json = serde_json::to_string_pretty(&res).unwrap();
        assert!(json.contains("\"version\": \"2.0.0\""));
        assert!(json.contains("\"changed\": true"));
    }
}
