//! Domain types for the skill system.

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use super::format::SkillFormat;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Requirements {
    #[serde(default)]
    pub bins: Vec<String>,
    #[serde(default, rename = "anyBins")]
    pub any_bins: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub config: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstallSpec {
    pub kind: InstallKind,
    #[serde(default)] pub formula: Option<String>,
    #[serde(default)] pub package: Option<String>,
    #[serde(default)] pub module: Option<String>,
    #[serde(default)] pub url: Option<String>,
    #[serde(default)] pub archive: Option<String>,
    #[serde(default)] pub extract: Option<bool>,
    #[serde(default, rename = "stripComponents")] pub strip_components: Option<u32>,
    #[serde(default, rename = "targetDir")] pub target_dir: Option<String>,
    #[serde(default)] pub os: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind { Brew, Node, Go, #[serde(rename = "uv")] Uv, Download }

impl std::fmt::Display for InstallKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallKind::Brew => write!(f, "brew"),
            InstallKind::Node => write!(f, "node"),
            InstallKind::Go => write!(f, "go"),
            InstallKind::Uv => write!(f, "uv"),
            InstallKind::Download => write!(f, "download"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RequirementsCheck {
    pub missing_bins: Vec<String>,
    pub missing_any_bins: Vec<String>,
    pub missing_env: Vec<String>,
    pub missing_config: Vec<String>,
    pub missing_os: Vec<String>,
    pub eligible: bool,
    pub config_checks: Vec<ConfigCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigCheck { pub path: String, pub satisfied: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus { Ready, NeedsSetup, Disabled }

impl std::fmt::Display for SkillStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { SkillStatus::Ready => write!(f, "ready"), SkillStatus::NeedsSetup => write!(f, "needs_setup"), SkillStatus::Disabled => write!(f, "disabled") }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource { Bundled, Managed, Workspace }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocationPolicy {
    #[serde(default = "default_true")] pub user_invocable: bool,
    #[serde(default)] pub disable_model_invocation: bool,
}
impl Default for SkillInvocationPolicy {
    fn default() -> Self { Self { user_invocable: true, disable_model_invocation: false } }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillMetadata {
    #[serde(default)] pub author: Option<String>,
    #[serde(default)] pub version: Option<String>,
    #[serde(default)] pub emoji: Option<String>,
    #[serde(default)] pub homepage: Option<String>,
    #[serde(default)] pub requires: Requirements,
    #[serde(default)] pub os: Vec<String>,
    #[serde(default)] pub install: Vec<SkillInstallSpec>,
    #[serde(default)] pub always: bool,
    #[serde(default, rename = "primaryEnv")] pub primary_env: Option<String>,
    #[serde(default, rename = "skillKey")] pub skill_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillConfig {
    #[serde(default = "default_true")] pub enabled: bool,
    #[serde(default)] pub env: HashMap<String, String>,
    #[serde(default)] pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillState { pub enabled: bool, pub installed_at: String, pub last_modified: String }
impl Default for SkillState {
    fn default() -> Self { let now = chrono::Utc::now().to_rfc3339(); Self { enabled: true, installed_at: now.clone(), last_modified: now } }
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String, pub description: String, pub content: String,
    pub path: PathBuf, pub base_dir: PathBuf, pub file_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta { pub name: String, pub description: String }
impl From<&Skill> for SkillMeta {
    fn from(s: &Skill) -> Self { SkillMeta { name: s.name.clone(), description: s.description.clone() } }
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub skill: Skill,
    pub metadata: Option<SkillMetadata>,
    pub eligibility: RequirementsCheck,
    pub status: SkillStatus,
    pub bundled: bool,
    pub source: SkillSource,
    pub invocation: SkillInvocationPolicy,
    pub format: SkillFormat,
    pub raw_yaml: serde_yaml::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRef { pub name: String, pub description: String, pub file_path: String, pub primary_env: Option<String>, pub required_env: Vec<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot { pub prompt: String, pub skills: Vec<SkillRef>, pub skill_filter: Option<Vec<String>> }

pub(crate) fn default_true() -> bool { true }
