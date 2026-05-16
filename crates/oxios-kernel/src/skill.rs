//! Skill system: markdown-based instructions for agents.
//!
//! Skills are markdown files with YAML frontmatter that define
//! reusable instruction templates. Agents read skills to understand
//! expected behaviors and patterns.
//!
//! Skill files are structured as:
//! ```markdown
//! ---
//! name: skill-name
//! description: Brief description of what this skill provides
//! ---
//!
//! # Skill Title
//!
//! Detailed instructions and guidelines...
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Metadata extracted from SKILL.md frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    /// Unique name for this skill.
    pub name: String,
    /// Human-readable description.
    pub description: String,
}

/// A loaded skill with its metadata and content.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Metadata extracted from frontmatter.
    pub meta: SkillMeta,
    /// The full markdown content (including frontmatter).
    pub content: String,
    /// Path to the source file.
    pub path: PathBuf,
}

/// Simple frontmatter parser for skill metadata.
///
/// Parses YAML frontmatter from the beginning of a markdown file.
/// Returns the metadata and remaining content.
fn parse_frontmatter(content: &str) -> Result<(SkillMeta, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        // No frontmatter, use defaults
        return Ok((
            SkillMeta {
                name: String::new(),
                description: String::new(),
            },
            content.to_string(),
        ));
    }

    // Find closing ---
    let after_open = &trimmed[3..];
    let closing_pos = after_open.find("---").context("unclosed frontmatter")?;
    let yaml_content = &after_open[..closing_pos];
    let rest = &after_open[closing_pos + 3..];

    // Parse YAML manually (simple key: value parsing)
    let mut name = String::new();
    let mut description = String::new();

    for line in yaml_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(val) = line.strip_prefix("name:") {
            name = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }

    Ok((
        SkillMeta { name, description },
        rest.trim_start().to_string(),
    ))
}

/// Store for managing skills as markdown files.
#[derive(Clone)]
pub struct SkillStore {
    /// Directory containing skill files.
    skills_dir: PathBuf,
}

impl std::fmt::Debug for SkillStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillStore")
            .field("skills_dir", &self.skills_dir)
            .finish()
    }
}

impl SkillStore {
    /// Creates a new skill store pointing to the given directory.
    ///
    /// The directory will be created if it doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use oxios_kernel::SkillStore;
    /// use std::path::PathBuf;
    ///
    /// let store = SkillStore::new(PathBuf::from("/tmp/skills")).unwrap();
    /// ```
    pub fn new(skills_dir: PathBuf) -> Result<Self> {
        Ok(Self { skills_dir })
    }

    /// Initialize the skills directory with default skills if empty.
    pub async fn init_defaults(&self, defaults_dir: &PathBuf) -> Result<()> {
        if !self.skills_dir.exists() {
            fs::create_dir_all(&self.skills_dir).await?;
        }

        // Check if any skills exist
        {
            let mut entries = fs::read_dir(&self.skills_dir).await?;
            let mut count = 0;
            while entries.next_entry().await?.is_some() {
                count += 1;
            }
            if count > 0 {
                return Ok(()); // Already has skills
            }
        }

        // Copy default skills from embedded or provided defaults directory
        if defaults_dir.exists() {
            let mut entries = fs::read_dir(defaults_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let src = entry.path();
                if src.is_dir() {
                    let skill_name = src
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    let dest = self.skills_dir.join(skill_name);
                    fs::create_dir_all(&dest).await?;

                    let mut skill_files = fs::read_dir(&src).await?;
                    while let Some(sfile) = skill_files.next_entry().await? {
                        if sfile.file_name() == "SKILL.md" {
                            let content = fs::read_to_string(sfile.path()).await?;
                            let dest_file = dest.join("SKILL.md");
                            if !dest_file.exists() {
                                fs::write(&dest_file, content).await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// List all available skills with their metadata.
    pub async fn list_skills(&self) -> Result<Vec<SkillMeta>> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            return Ok(skills);
        }

        let mut entries = fs::read_dir(&self.skills_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    if let Ok(content) = fs::read_to_string(&skill_file).await {
                        if let Ok((meta, _)) = parse_frontmatter(&content) {
                            if !meta.name.is_empty() {
                                skills.push(meta);
                            }
                        }
                    }
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Load a specific skill by name.
    ///
    /// Looks for `<name>/SKILL.md` in the skills directory.
    pub async fn load_skill(&self, name: &str) -> Result<Option<Skill>> {
        let skill_path = self.skills_dir.join(name).join("SKILL.md");

        if !skill_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&skill_path).await?;
        let (meta, content) = parse_frontmatter(&content)?;

        Ok(Some(Skill {
            meta,
            content,
            path: skill_path,
        }))
    }

    /// Create a new skill with the given metadata and content.
    ///
    /// The skill will be saved as `<skills_dir>/<name>/SKILL.md`.
    pub async fn create_skill(&self, name: &str, description: &str, content: &str) -> Result<()> {
        fs::create_dir_all(self.skills_dir.join(name)).await?;

        let skill_file = self.skills_dir.join(name).join("SKILL.md");
        let frontmatter = format!(
            "---\nname: {}\ndescription: {}\n---\n\n{}",
            name, description, content
        );

        fs::write(&skill_file, frontmatter).await?;
        Ok(())
    }

    /// Delete a skill by name.
    ///
    /// Removes the entire `<name>/` directory.
    pub async fn delete_skill(&self, name: &str) -> Result<()> {
        let skill_dir = self.skills_dir.join(name);
        if skill_dir.exists() {
            fs::remove_dir_all(&skill_dir).await?;
        }
        Ok(())
    }

    /// Get the path to the skills directory.
    pub fn path(&self) -> &PathBuf {
        &self.skills_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_with_metadata() {
        let content = r#"---
name: code-review
description: Guidelines for reviewing code changes
---

# Code Review

Follow these steps to review code effectively.
"#;
        let (meta, rest) = parse_frontmatter(content).unwrap();
        assert_eq!(meta.name, "code-review");
        assert_eq!(meta.description, "Guidelines for reviewing code changes");
        assert!(rest.contains("Code Review"));
    }

    #[test]
    fn test_parse_frontmatter_no_metadata() {
        let content = "# Just a Title\n\nSome content";
        let (meta, rest) = parse_frontmatter(content).unwrap();
        assert!(meta.name.is_empty());
        assert!(rest.contains("Just a Title"));
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = r#"---
name: "test-skill"
description: 'A test skill'
---

Content here
"#;
        let (meta, _) = parse_frontmatter(content).unwrap();
        assert_eq!(meta.name, "test-skill");
        assert_eq!(meta.description, "A test skill");
    }
}
