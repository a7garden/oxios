//! Skills.sh skill installer.
//!
//! Installs skills from the skills.sh registry into the local skill directory.
//! Unlike ClawHub (which downloads zip archives), skills.sh provides file contents
//! directly via the API — no zip extraction needed.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use super::client::SkillsShClient;

/// Installation result for a skills.sh skill.
#[derive(Debug, Clone, Serialize)]
pub struct SkillsShInstallResult {
    pub ok: bool,
    /// Skill slug (e.g. `"frontend-design"`).
    pub slug: String,
    /// Source identifier (e.g. `"vercel-labs/agent-skills"`).
    pub source: String,
    /// Full id (e.g. `"vercel-labs/agent-skills/frontend-design"`).
    pub skill_id: String,
    /// Directory where the skill was installed.
    pub target_dir: PathBuf,
    /// Number of files written.
    pub file_count: usize,
    /// Install count from skills.sh.
    pub installs: i64,
    /// SHA-256 hash of the skill's contents (from skills.sh).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// Origin metadata for a skill installed from skills.sh.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SkillsShOrigin {
    /// Schema version (always 1).
    pub version: u32,
    /// Registry base URL.
    pub registry: String,
    /// Full skill id on skills.sh.
    #[serde(rename = "skillId")]
    pub skill_id: String,
    /// Skill slug.
    pub slug: String,
    /// Source (owner/repo).
    pub source: String,
    /// SHA-256 hash at install time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// ISO 8601 timestamp of installation.
    #[serde(rename = "installedAt")]
    pub installed_at: String,
}

/// Skills.sh skill installer.
pub struct SkillsShInstaller {
    client: SkillsShClient,
    /// Directory where skills are installed (e.g. `~/.oxios/skills/`).
    skills_dir: PathBuf,
}

impl SkillsShInstaller {
    /// Create a new installer.
    pub fn new(skills_dir: PathBuf, base_url: Option<String>, api_key: Option<String>) -> Self {
        Self {
            client: SkillsShClient::new(base_url, api_key).expect("valid skills.sh base URL"),
            skills_dir,
        }
    }

    /// Install a skill from skills.sh by its full id (e.g. `"vercel-labs/agent-skills/frontend-design"`).
    pub async fn install(&self, skill_id: &str) -> Result<SkillsShInstallResult> {
        let detail = self.client.get_skill(skill_id).await?;

        let files = detail.files.as_ref().ok_or_else(|| {
            anyhow::anyhow!("skill {skill_id} has no files available (no snapshot)")
        })?;

        if files.is_empty() {
            anyhow::bail!("skill {skill_id} has no files");
        }

        let target_dir = self.skills_dir.join(&detail.slug);

        if target_dir.exists() {
            // Check if installed from same source
            let origin_path = target_dir.join(".skills_sh").join("origin.json");
            if origin_path.exists() {
                anyhow::bail!(
                    "skill already installed: {} (use update to reinstall)",
                    detail.slug
                );
            }
            // Different origin — allow overwrite with warning
            tracing::warn!(
                "skill directory {} exists but not from skills.sh, overwriting",
                detail.slug
            );
            fs::remove_dir_all(&target_dir).context("remove existing skill dir")?;
        }

        fs::create_dir_all(&target_dir).context("create skill directory")?;

        // Write all files
        let mut file_count = 0usize;
        for file in files {
            let file_path = target_dir.join(&file.path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).context("create parent dir for skill file")?;
            }
            fs::write(&file_path, &file.contents).context("write skill file")?;
            file_count += 1;
        }

        // Write origin metadata
        let origin = SkillsShOrigin {
            version: 1,
            registry: self.client.base_url().to_string(),
            skill_id: detail.id.clone(),
            slug: detail.slug.clone(),
            source: detail.source.clone(),
            hash: detail.hash.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        let origin_dir = target_dir.join(".skills_sh");
        fs::create_dir_all(&origin_dir).context("create .skills_sh dir")?;
        fs::write(
            origin_dir.join("origin.json"),
            serde_json::to_string_pretty(&origin).context("serialize origin")?,
        )
        .context("write origin.json")?;

        Ok(SkillsShInstallResult {
            ok: true,
            slug: detail.slug,
            source: detail.source,
            skill_id: detail.id,
            target_dir,
            file_count,
            installs: detail.installs,
            hash: detail.hash,
        })
    }

    /// Update a skill by re-installing from skills.sh.
    pub async fn update(&self, skill_id: &str) -> Result<SkillsShInstallResult> {
        // For skills.sh, update is just a re-install (overwrite)
        // First remove the existing skill
        let detail = self.client.get_skill(skill_id).await?;
        let target_dir = self.skills_dir.join(&detail.slug);

        if target_dir.exists() {
            // Preserve the origin to check hash changes
            let old_hash = self.read_installed_hash(&detail.slug);
            let new_hash = detail.hash.as_deref();

            if old_hash.as_deref() == new_hash {
                return Ok(SkillsShInstallResult {
                    ok: true,
                    slug: detail.slug,
                    source: detail.source,
                    skill_id: detail.id,
                    target_dir,
                    file_count: 0,
                    installs: detail.installs,
                    hash: detail.hash,
                });
            }

            fs::remove_dir_all(&target_dir).context("remove old skill dir")?;
        }

        // Re-install
        self.install(skill_id).await
    }

    /// Check if a skill is installed from skills.sh.
    pub fn is_installed(&self, slug: &str) -> bool {
        self.skills_dir
            .join(slug)
            .join(".skills_sh")
            .join("origin.json")
            .exists()
    }

    /// Read the installed hash for a skill from its origin file.
    fn read_installed_hash(&self, slug: &str) -> Option<String> {
        let origin_path = self
            .skills_dir
            .join(slug)
            .join(".skills_sh")
            .join("origin.json");
        let content = fs::read_to_string(&origin_path).ok()?;
        let origin: SkillsShOrigin = serde_json::from_str(&content).ok()?;
        origin.hash
    }

    /// Read the installed skill_id for a skill from its origin file.
    pub fn get_installed_skill_id(&self, slug: &str) -> Option<String> {
        let origin_path = self
            .skills_dir
            .join(slug)
            .join(".skills_sh")
            .join("origin.json");
        let content = fs::read_to_string(&origin_path).ok()?;
        let origin: SkillsShOrigin = serde_json::from_str(&content).ok()?;
        Some(origin.skill_id)
    }

    /// Access the underlying client.
    pub fn client(&self) -> &SkillsShClient {
        &self.client
    }

    /// Returns the skills directory.
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin_serialize() {
        let origin = SkillsShOrigin {
            version: 1,
            registry: "https://skills.sh/".to_string(),
            skill_id: "vercel-labs/agent-skills/frontend-design".to_string(),
            slug: "frontend-design".to_string(),
            source: "vercel-labs/agent-skills".to_string(),
            hash: Some("abc123".to_string()),
            installed_at: "2026-06-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string_pretty(&origin).unwrap();
        assert!(json.contains("\"skillId\":"));
        assert!(json.contains("\"frontend-design\""));
    }
}
