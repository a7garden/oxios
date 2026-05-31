#![allow(missing_docs)]
//! Skill manager — loads, stores, and manages skills.

use super::frontmatter::parse_skill;
use super::prompt::{compact_path, format_skills_for_prompt};
use super::requirements::check_requirements;
use super::types::*;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

pub struct SkillManager {
    skills_dir: PathBuf,
    bundled_dir: PathBuf,
    installed: RwLock<HashMap<String, SkillEntry>>,
}
impl std::fmt::Debug for SkillManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillManager")
            .field("skills_dir", &self.skills_dir)
            .field("bundled_dir", &self.bundled_dir)
            .finish()
    }
}
impl SkillManager {
    pub fn new(skills_dir: PathBuf, bundled_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            bundled_dir,
            installed: RwLock::new(HashMap::new()),
        }
    }
    pub async fn init(&self) -> Result<()> {
        if !self.skills_dir.exists() {
            tokio::fs::create_dir_all(&self.skills_dir).await?;
        }
        if self.is_dir_empty(&self.skills_dir).await? && self.bundled_dir.exists() {
            self.bootstrap_from_bundled().await?;
        }
        let mut map: HashMap<String, SkillEntry> = HashMap::new();
        if self.bundled_dir.exists() {
            self.load_skills_from_dir(&self.bundled_dir, true, &mut map)
                .await?;
        }
        self.load_skills_from_dir(&self.skills_dir, false, &mut map)
            .await?;
        *self.installed.write().await = map;
        Ok(())
    }
    pub async fn list_skills(&self) -> Vec<SkillEntry> {
        let mut s: Vec<SkillEntry> = self.installed.read().await.values().cloned().collect();
        s.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
        s
    }
    pub async fn get_skill(&self, name: &str) -> Option<SkillEntry> {
        self.installed.read().await.get(name).cloned()
    }
    pub async fn get_skill_content(&self, name: &str) -> Option<String> {
        self.installed
            .read()
            .await
            .get(name)
            .map(|e| e.skill.content.clone())
    }
    pub async fn build_snapshot(
        &self,
        _agent_id: Option<&str>,
        skill_filter: Option<&[String]>,
    ) -> SkillSnapshot {
        let entries = self.list_skills().await;
        let visible: Vec<&SkillEntry> = entries
            .iter()
            .filter(|e| {
                e.status != SkillStatus::Disabled
                    && e.eligibility.eligible
                    && !e.invocation.disable_model_invocation
            })
            .collect();
        let filtered: Vec<&SkillEntry> = if let Some(f) = skill_filter {
            visible
                .into_iter()
                .filter(|e| f.contains(&e.skill.name))
                .collect()
        } else {
            visible
        };
        SkillSnapshot {
            prompt: format_skills_for_prompt(&filtered),
            skills: filtered
                .iter()
                .map(|e| SkillRef {
                    name: e.skill.name.clone(),
                    description: e.skill.description.clone(),
                    file_path: compact_path(&e.skill.file_path),
                    primary_env: e.metadata.as_ref().and_then(|m| m.primary_env.clone()),
                    required_env: e
                        .metadata
                        .as_ref()
                        .map(|m| m.requires.env.clone())
                        .unwrap_or_default(),
                })
                .collect(),
            skill_filter: skill_filter.map(|f| f.to_vec()),
        }
    }
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut installed = self.installed.write().await;
        if let Some(entry) = installed.get_mut(name) {
            let state = SkillState {
                enabled,
                installed_at: chrono::Utc::now().to_rfc3339(),
                last_modified: chrono::Utc::now().to_rfc3339(),
            };
            tokio::fs::write(
                entry.skill.base_dir.join("state.json"),
                serde_json::to_string_pretty(&state)?,
            )
            .await?;
            entry.status = if enabled {
                if entry.eligibility.eligible {
                    SkillStatus::Ready
                } else {
                    SkillStatus::NeedsSetup
                }
            } else {
                SkillStatus::Disabled
            };
        } else {
            anyhow::bail!("skill not found: {name}");
        }
        Ok(())
    }
    pub async fn create_skill(&self, name: &str, description: &str, content: &str) -> Result<()> {
        let dir = self.skills_dir.join(name);
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n\n{content}"),
        )
        .await?;
        let entry = Self::load_skill_entry(&dir.join("SKILL.md"), false)?;
        self.installed.write().await.insert(name.to_string(), entry);
        Ok(())
    }
    pub async fn delete_skill(&self, name: &str) -> Result<()> {
        let dir = self.skills_dir.join(name);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await?;
        }
        self.installed.write().await.remove(name);
        Ok(())
    }
    pub async fn list_skills_meta(&self) -> Vec<SkillMeta> {
        let mut m: Vec<SkillMeta> = self
            .installed
            .read()
            .await
            .values()
            .map(|e| SkillMeta::from(&e.skill))
            .collect();
        m.sort_by(|a, b| a.name.cmp(&b.name));
        m
    }
    pub async fn load_skill(&self, name: &str) -> Result<Option<Skill>> {
        Ok(self
            .installed
            .read()
            .await
            .get(name)
            .map(|e| e.skill.clone()))
    }
    pub fn path(&self) -> &PathBuf {
        &self.skills_dir
    }

    /// Load additional skills from an external directory (e.g. bundled defaults).
    /// Each subdirectory containing a `SKILL.md` is loaded as a bundled skill.
    pub async fn load_from_dir(&self, dir: &Path) -> Result<()> {
        let mut map = self.installed.write().await;
        self.load_skills_from_dir(dir, true, &mut map).await?;
        Ok(())
    }

    async fn load_skills_from_dir(
        &self,
        dir: &Path,
        bundled: bool,
        map: &mut HashMap<String, SkillEntry>,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let sf = path.join("SKILL.md");
                if sf.exists() {
                    match Self::load_skill_entry(&sf, bundled) {
                        Ok(se) => {
                            let n = se.skill.name.clone();
                            if bundled && map.contains_key(&n) {
                                continue;
                            }
                            map.insert(n, se);
                        }
                        Err(e) => {
                            tracing::warn!("failed to parse skill {:?}: {}", sf, e);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn load_skill_entry(skill_file: &Path, bundled: bool) -> Result<SkillEntry> {
        let content = std::fs::read_to_string(skill_file)
            .with_context(|| format!("reading {skill_file:?}"))?;
        let skill_dir = skill_file.parent().context("no parent")?;
        let (parsed, body) = parse_skill(&content, skill_dir)?;
        let name = if parsed.name.is_empty() {
            skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            parsed.name
        };
        let base_dir = skill_dir.parent().context("no grandparent")?.to_path_buf();
        let skill = Skill {
            name: name.clone(),
            description: parsed.description,
            content: body,
            path: skill_file.to_path_buf(),
            base_dir,
            file_path: skill_file.to_path_buf(),
        };
        let (eligibility, status) = {
            let c = check_requirements(&parsed.metadata);
            let elig = c.eligible;
            (
                c,
                if elig {
                    SkillStatus::Ready
                } else {
                    SkillStatus::NeedsSetup
                },
            )
        };
        let status = {
            let sp = skill.path.parent().unwrap().join("state.json");
            if sp.exists() {
                if let Ok(sc) = std::fs::read_to_string(&sp) {
                    if let Ok(s) = serde_json::from_str::<SkillState>(&sc) {
                        if !s.enabled {
                            SkillStatus::Disabled
                        } else {
                            status
                        }
                    } else {
                        status
                    }
                } else {
                    status
                }
            } else {
                status
            }
        };
        Ok(SkillEntry {
            skill,
            metadata: Some(parsed.metadata),
            eligibility,
            status,
            bundled,
            source: if bundled {
                SkillSource::Bundled
            } else {
                SkillSource::Managed
            },
            invocation: parsed.invocation,
            format: parsed.format,
            raw_yaml: parsed.raw_yaml,
        })
    }
    async fn bootstrap_from_bundled(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.bundled_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src = entry.path();
            if src.is_dir() {
                let name = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let dest = self.skills_dir.join(name);
                tokio::fs::create_dir_all(&dest).await?;
                let sf = src.join("SKILL.md");
                if sf.exists() {
                    let df = dest.join("SKILL.md");
                    if !df.exists() {
                        tokio::fs::write(&df, tokio::fs::read_to_string(&sf).await?).await?;
                    }
                }
            }
        }
        Ok(())
    }
    async fn is_dir_empty(&self, dir: &Path) -> Result<bool> {
        if !dir.exists() {
            return Ok(true);
        }
        let mut e = tokio::fs::read_dir(dir).await?;
        Ok(e.next_entry().await?.is_none())
    }
}
