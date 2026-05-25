//! Skill system: unified Skill model with OpenClaw-compatible frontmatter.
//!
//! Skills are markdown files (SKILL.md) with YAML frontmatter that define
//! reusable instruction templates for agents. The frontmatter carries all
//! metadata — requirements, install specs, invocation policy — in a single
//! file (no separate TOML).
//!
//! # Directory Layout
//!
//! ```text
//! ~/.oxios/workspace/skills/     ← managed (user) skills
//!   code-review/
//!     SKILL.md
//! share/skills/                   ← bundled skills (lowest priority)
//!   guardian/
//!     SKILL.md
//! ```
//!
//! # Frontmatter Format
//!
//! ```yaml
//! ---
//! name: code-review
//! description: Deep code review...
//! author: oxios
//! version: 1.0.0
//! emoji: 🔍
//! homepage: https://...
//! requires:
//!   bins: ["git"]
//!   anyBins: ["ffmpeg", "avconv"]
//!   env: ["GITHUB_TOKEN"]
//!   config: []
//! os: ["darwin", "linux"]
//! install:
//!   - kind: brew
//!     formula: git
//! always: false
//! user-invocable: true
//! disable-model-invocation: false
//! ---
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// ─── Types (RFC-009 §3.3-3.5) ─────────────────────────────────────

/// 4-dimensional requirements (OpenClaw model).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Requirements {
    /// Required binaries — all must be present.
    #[serde(default)]
    pub bins: Vec<String>,
    /// Alternative binaries — at least one must be present.
    #[serde(default, rename = "anyBins")]
    pub any_bins: Vec<String>,
    /// Required environment variables.
    #[serde(default)]
    pub env: Vec<String>,
    /// Required config paths.
    #[serde(default)]
    pub config: Vec<String>,
}

/// Install spec for automatic dependency installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstallSpec {
    pub kind: InstallKind,
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub archive: Option<String>,
    #[serde(default)]
    pub extract: Option<bool>,
    #[serde(default, rename = "stripComponents")]
    pub strip_components: Option<u32>,
    #[serde(default, rename = "targetDir")]
    pub target_dir: Option<String>,
    #[serde(default)]
    pub os: Vec<String>,
}

/// Install mechanism kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind {
    Brew,
    Node,
    Go,
    #[serde(rename = "uv")]
    Uv,
    Download,
}

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

/// Skill eligibility check result.
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

/// Individual config-path satisfaction check.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigCheck {
    pub path: String,
    pub satisfied: bool,
}

/// Skill eligibility status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    /// All requirements met, enabled.
    Ready,
    /// Some requirements not met.
    NeedsSetup,
    /// User-disabled.
    Disabled,
}

impl std::fmt::Display for SkillStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillStatus::Ready => write!(f, "ready"),
            SkillStatus::NeedsSetup => write!(f, "needs_setup"),
            SkillStatus::Disabled => write!(f, "disabled"),
        }
    }
}

/// Skill source scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    /// Bundled with oxios (lowest priority).
    Bundled,
    /// User-managed global skills.
    Managed,
    /// Project workspace skills (highest priority).
    Workspace,
}

/// Invocation policy from frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocationPolicy {
    #[serde(default = "default_true")]
    pub user_invocable: bool,
    #[serde(default)]
    pub disable_model_invocation: bool,
}

impl Default for SkillInvocationPolicy {
    fn default() -> Self {
        Self {
            user_invocable: true,
            disable_model_invocation: false,
        }
    }
}

/// Skill metadata parsed from frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillMetadata {
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub requires: Requirements,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub install: Vec<SkillInstallSpec>,
    #[serde(default)]
    pub always: bool,
    #[serde(default, rename = "primaryEnv")]
    pub primary_env: Option<String>,
    #[serde(default, rename = "skillKey")]
    pub skill_key: Option<String>,
}

/// Per-skill config from config.toml.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub config: HashMap<String, String>,
}

/// Persisted state for a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillState {
    pub enabled: bool,
    pub installed_at: String,
    pub last_modified: String,
}

impl Default for SkillState {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            enabled: true,
            installed_at: now.clone(),
            last_modified: now,
        }
    }
}

/// A loaded skill with its content and provenance.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Skill name (from frontmatter, or directory name as fallback).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Full markdown content (without frontmatter).
    pub content: String,
    /// Path to the SKILL.md file.
    pub path: PathBuf,
    /// Base directory (parent of the skill directory).
    pub base_dir: PathBuf,
    /// Full file path alias (same as path, for ergonomics).
    pub file_path: PathBuf,
}

/// Lightweight view of a skill for listing / API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
}

impl From<&Skill> for SkillMeta {
    fn from(s: &Skill) -> Self {
        SkillMeta {
            name: s.name.clone(),
            description: s.description.clone(),
        }
    }
}

/// A loaded skill with full metadata and eligibility state.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    /// The skill itself.
    pub skill: Skill,
    /// Raw frontmatter key-value map (for debugging / extensions).
    pub frontmatter: HashMap<String, String>,
    /// Extended metadata parsed from frontmatter.
    pub metadata: Option<SkillMetadata>,
    /// Eligibility check result.
    pub eligibility: RequirementsCheck,
    /// Status derived from eligibility + config.
    pub status: SkillStatus,
    /// Whether this is a bundled skill.
    pub bundled: bool,
    /// Source scope.
    pub source: SkillSource,
    /// Invocation policy.
    pub invocation: SkillInvocationPolicy,
}

/// Reference to a skill inside a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRef {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub primary_env: Option<String>,
    pub required_env: Vec<String>,
}

/// Snapshot of resolved skills for an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    /// Formatted XML prompt block for skills.
    pub prompt: String,
    /// All skills the agent can see (model-visible).
    pub skills: Vec<SkillRef>,
    /// Skill filter used to build this snapshot.
    pub skill_filter: Option<Vec<String>>,
}

// ─── Helpers ────────────────────────────────────────────────────────

fn default_true() -> bool {
    true
}

/// Escape special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Compact a file path by replacing the home directory prefix with `~`.
fn compact_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        let path_str = path.to_string_lossy();
        if let Some(rest) = path_str.strip_prefix(home_str.as_ref()) {
            return format!("~{}", rest);
        }
    }
    path.to_string_lossy().into_owned()
}

/// Check whether a binary is available on the host.
fn has_bin(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current platform string (darwin / linux / windows).
fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

// ─── Frontmatter Parser ────────────────────────────────────────────

/// Parsed frontmatter result: name, description, raw map, metadata, invocation.
struct ParsedFrontmatter {
    name: String,
    description: String,
    raw: HashMap<String, String>,
    metadata: Option<SkillMetadata>,
    invocation: SkillInvocationPolicy,
}

/// Parse YAML frontmatter from a SKILL.md file.
///
/// Returns `(ParsedFrontmatter, remaining_markdown_content)`.
fn parse_frontmatter(content: &str) -> Result<(ParsedFrontmatter, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        // No frontmatter — use whole content
        return Ok((
            ParsedFrontmatter {
                name: String::new(),
                description: String::new(),
                raw: HashMap::new(),
                metadata: None,
                invocation: SkillInvocationPolicy::default(),
            },
            content.to_string(),
        ));
    }

    let after_open = &trimmed[3..];
    let closing_pos = after_open
        .find("---")
        .context("unclosed frontmatter — missing closing '---'")?;
    let yaml = &after_open[..closing_pos];
    let rest = after_open[closing_pos + 3..].trim_start().to_string();

    // ── Simple key: value + nested block parsing ──
    let mut name = String::new();
    let mut description = String::new();
    let mut raw: HashMap<String, String> = HashMap::new();

    // For metadata we do a more structured parse
    let mut bins: Vec<String> = Vec::new();
    let mut any_bins: Vec<String> = Vec::new();
    let mut env: Vec<String> = Vec::new();
    let mut config: Vec<String> = Vec::new();
    let mut os: Vec<String> = Vec::new();
    let mut install_specs: Vec<SkillInstallSpec> = Vec::new();
    let mut author: Option<String> = None;
    let mut version: Option<String> = None;
    let mut emoji: Option<String> = None;
    let mut homepage: Option<String> = None;
    let mut always = false;
    let mut primary_env: Option<String> = None;
    let mut skill_key: Option<String> = None;
    let mut user_invocable = true;
    let mut disable_model_invocation = false;

    let lines: Vec<&str> = yaml.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed_line = line.trim();

        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            i += 1;
            continue;
        }

        // ── Top-level scalar fields ──
        if let Some(val) = parse_scalar(trimmed_line, "name:") {
            name = val;
            raw.insert("name".into(), name.clone());
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "description:") {
            description = val;
            raw.insert("description".into(), description.clone());
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "author:") {
            author = Some(val.clone());
            raw.insert("author".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "version:") {
            version = Some(val.clone());
            raw.insert("version".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "emoji:") {
            emoji = Some(val.clone());
            raw.insert("emoji".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "homepage:") {
            homepage = Some(val.clone());
            raw.insert("homepage".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "always:") {
            always = val == "true";
            raw.insert("always".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "primary-env:") {
            primary_env = Some(val.clone());
            raw.insert("primary-env".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "primaryEnv:") {
            primary_env = Some(val.clone());
            raw.insert("primaryEnv".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "skill-key:") {
            skill_key = Some(val.clone());
            raw.insert("skill-key".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "skillKey:") {
            skill_key = Some(val.clone());
            raw.insert("skillKey".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "user-invocable:") {
            user_invocable = val == "true";
            raw.insert("user-invocable".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "disable-model-invocation:") {
            disable_model_invocation = val == "true";
            raw.insert("disable-model-invocation".into(), val);
            i += 1;
            continue;
        }
        if let Some(val) = parse_scalar(trimmed_line, "os:") {
            os = parse_inline_list(&val);
            raw.insert("os".into(), val);
            i += 1;
            continue;
        }

        // ── requires: block ──
        if trimmed_line == "requires:" || trimmed_line.starts_with("requires:") {
            // Consume indented sub-lines
            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                let sub_trimmed = sub.trim();
                if sub_trimmed.is_empty() {
                    i += 1;
                    continue;
                }
                // Stop if we lose indentation (back to top level)
                if !sub.starts_with(' ') && !sub.starts_with('\t') {
                    break;
                }
                if let Some(val) = parse_scalar(sub_trimmed, "bins:") {
                    bins = parse_inline_list(&val);
                } else if let Some(val) = parse_scalar(sub_trimmed, "anyBins:") {
                    any_bins = parse_inline_list(&val);
                } else if let Some(val) = parse_scalar(sub_trimmed, "env:") {
                    env = parse_inline_list(&val);
                } else if let Some(val) = parse_scalar(sub_trimmed, "config:") {
                    config = parse_inline_list(&val);
                }
                i += 1;
            }
            continue;
        }

        // ── install: block (list of maps) ──
        if trimmed_line == "install:" || trimmed_line.starts_with("install:") {
            i += 1;
            // Each install entry starts with "  - kind: ..."
            while i < lines.len() {
                let sub = lines[i];
                let sub_trimmed = sub.trim();
                if sub_trimmed.is_empty() {
                    i += 1;
                    continue;
                }
                if !sub.starts_with(' ') && !sub.starts_with('\t') {
                    break;
                }
                // Detect "- kind:" or just "kind:"
                if sub_trimmed.starts_with("- kind:") || sub_trimmed.starts_with("kind:") {
                    let spec = parse_install_entry(&lines, &mut i);
                    install_specs.push(spec);
                    continue;
                }
                i += 1;
            }
            continue;
        }

        // Unknown top-level key — store in raw map
        if let Some(colon_pos) = trimmed_line.find(':') {
            let key = trimmed_line[..colon_pos].trim();
            let val = trimmed_line[colon_pos + 1..].trim();
            if !key.is_empty() {
                raw.insert(key.to_string(), unquote(val));
            }
        }
        i += 1;
    }

    let metadata = SkillMetadata {
        author,
        version,
        emoji,
        homepage,
        requires: Requirements {
            bins,
            any_bins,
            env,
            config,
        },
        os,
        install: install_specs,
        always,
        primary_env,
        skill_key,
    };

    Ok((
        ParsedFrontmatter {
            name,
            description,
            raw,
            metadata: Some(metadata),
            invocation: SkillInvocationPolicy {
                user_invocable,
                disable_model_invocation,
            },
        },
        rest,
    ))
}

/// Extract a scalar value after a key prefix, or None.
fn parse_scalar<'a>(line: &'a str, prefix: &str) -> Option<String> {
    if line.starts_with(prefix) {
        let val = line[prefix.len()..].trim();
        if !val.is_empty() {
            return Some(unquote(val));
        }
    }
    None
}

/// Remove matching quotes from a string.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Parse a YAML inline list like `["git", "gh"]` or `[]`.
fn parse_inline_list(s: &str) -> Vec<String> {
    let s = s.trim();
    if s == "[]" || s.is_empty() {
        return Vec::new();
    }
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        return inner
            .split(',')
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect();
    }
    // Not an inline list — treat as single value
    vec![unquote(s)]
}

/// Parse one install entry starting at the current `kind:` line.
/// Advances `i` past all lines that belong to this entry.
fn parse_install_entry(lines: &[&str], i: &mut usize) -> SkillInstallSpec {
    let mut kind = InstallKind::Brew;
    let mut formula: Option<String> = None;
    let mut package: Option<String> = None;
    let mut module: Option<String> = None;
    let mut url: Option<String> = None;
    let mut archive: Option<String> = None;
    let mut extract: Option<bool> = None;
    let mut strip_components: Option<u32> = None;
    let mut target_dir: Option<String> = None;
    let mut os: Vec<String> = Vec::new();

    // Process lines until we hit the next entry or lose indentation
    let base_indent = indent_level(lines[*i]);
    loop {
        let line = lines[*i];
        let trimmed = line.trim();

        if trimmed.starts_with("- kind:") {
            let val = trimmed["- kind:".len()..].trim();
            kind = match unquote(&val.to_lowercase()) {
                k if k == "brew" => InstallKind::Brew,
                k if k == "node" => InstallKind::Node,
                k if k == "go" => InstallKind::Go,
                k if k == "uv" => InstallKind::Uv,
                k if k == "download" => InstallKind::Download,
                _ => InstallKind::Brew,
            };
        } else if let Some(val) = parse_scalar(trimmed, "kind:") {
            kind = match val.to_lowercase().as_str() {
                "brew" => InstallKind::Brew,
                "node" => InstallKind::Node,
                "go" => InstallKind::Go,
                "uv" => InstallKind::Uv,
                "download" => InstallKind::Download,
                _ => InstallKind::Brew,
            };
        } else if let Some(val) = parse_scalar(trimmed, "formula:") {
            formula = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "package:") {
            package = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "module:") {
            module = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "url:") {
            url = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "archive:") {
            archive = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "extract:") {
            extract = Some(val == "true");
        } else if let Some(val) = parse_scalar(trimmed, "stripComponents:") {
            strip_components = val.parse().ok();
        } else if let Some(val) = parse_scalar(trimmed, "strip-components:") {
            strip_components = val.parse().ok();
        } else if let Some(val) = parse_scalar(trimmed, "targetDir:") {
            target_dir = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "target-dir:") {
            target_dir = Some(val);
        } else if let Some(val) = parse_scalar(trimmed, "os:") {
            os = parse_inline_list(&val);
        }

        *i += 1;
        if *i >= lines.len() {
            break;
        }
        // Stop if we hit a line with same or less indentation than the entry start,
        // unless it's a blank line or still part of the list item
        let next_line = lines[*i];
        let next_trimmed = next_line.trim();
        if next_trimmed.is_empty() {
            continue;
        }
        // A new list item ("- kind:") at the same indent level starts a new entry
        if next_trimmed.starts_with("- kind:") && indent_level(next_line) == base_indent {
            break;
        }
        // A non-indented line means we've left the install block
        if indent_level(next_line) == 0 {
            break;
        }
    }

    SkillInstallSpec {
        kind,
        formula,
        package,
        module,
        url,
        archive,
        extract,
        strip_components,
        target_dir,
        os,
    }
}

/// Count leading spaces / tabs for indentation level.
fn indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

// ─── Requirements Evaluation ───────────────────────────────────────

/// Evaluate all requirements for a skill given its metadata.
pub fn check_requirements(metadata: &SkillMetadata) -> RequirementsCheck {
    let platform = current_platform();

    // bins: all must be present
    let missing_bins: Vec<String> = metadata
        .requires
        .bins
        .iter()
        .filter(|b| !has_bin(b))
        .cloned()
        .collect();

    // any_bins: at least one must be present (skip if empty)
    let missing_any_bins = if metadata.requires.any_bins.is_empty() {
        Vec::new()
    } else if metadata
        .requires
        .any_bins
        .iter()
        .any(|b| has_bin(b))
    {
        Vec::new()
    } else {
        metadata.requires.any_bins.clone()
    };

    // env: check std::env
    let missing_env: Vec<String> = metadata
        .requires
        .env
        .iter()
        .filter(|e| std::env::var(e).is_err())
        .cloned()
        .collect();

    // config: stub — pass-through (config checking requires OxiosConfig integration)
    let config_checks: Vec<ConfigCheck> = metadata
        .requires
        .config
        .iter()
        .map(|path| ConfigCheck {
            path: path.clone(),
            satisfied: true, // stub
        })
        .collect();
    let missing_config: Vec<String> = config_checks
        .iter()
        .filter(|c| !c.satisfied)
        .map(|c| c.path.clone())
        .collect();

    // os: if specified, current platform must be in the list
    let missing_os = if metadata.os.is_empty() || metadata.os.iter().any(|o| o == platform) {
        Vec::new()
    } else {
        metadata.os.clone()
    };

    let eligible = metadata.always
        || (missing_bins.is_empty()
            && missing_any_bins.is_empty()
            && missing_env.is_empty()
            && missing_config.is_empty()
            && missing_os.is_empty());

    RequirementsCheck {
        missing_bins,
        missing_any_bins,
        missing_env,
        missing_config,
        missing_os,
        eligible,
        config_checks,
    }
}

// ─── Prompt Formatting ─────────────────────────────────────────────

/// Format skills as XML prompt block (matches OpenClaw output format).
fn format_skills_for_prompt(skills: &[&SkillEntry]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "\n\nThe following skills provide specialized instructions for specific tasks.".into(),
        "Use the read tool to load a skill's file when the task matches its description.".into(),
        "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.".into(),
        String::new(),
        "<available_skills>".into(),
    ];
    for skill in skills {
        lines.push("  <skill>".into());
        lines.push(format!(
            "    <name>{}</name>",
            escape_xml(&skill.skill.name)
        ));
        lines.push(format!(
            "    <description>{}</description>",
            escape_xml(&skill.skill.description)
        ));
        lines.push(format!(
            "    <location>{}</location>",
            escape_xml(&compact_path(&skill.skill.file_path))
        ));
        lines.push("  </skill>".into());
    }
    lines.push("</available_skills>".into());
    lines.join("\n")
}

// ─── SkillManager ──────────────────────────────────────────────────

/// Unified skill manager — replaces both SkillStore and ProgramManager.
pub struct SkillManager {
    /// Workspace / managed skills directory.
    skills_dir: PathBuf,
    /// Bundled skills directory (share/skills/).
    bundled_dir: PathBuf,
    /// In-memory cache of loaded skills.
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
    /// Create a new skill manager.
    pub fn new(skills_dir: PathBuf, bundled_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            bundled_dir,
            installed: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize: load skills from both directories.
    ///
    /// Workspace skills take precedence over bundled skills with the same name.
    /// If the skills directory is empty, bootstraps from bundled_dir.
    pub async fn init(&self) -> Result<()> {
        // Ensure skills_dir exists
        if !self.skills_dir.exists() {
            tokio::fs::create_dir_all(&self.skills_dir).await?;
        }

        // Bootstrap from bundled_dir if skills_dir is empty
        if self.is_dir_empty(&self.skills_dir).await? && self.bundled_dir.exists() {
            self.bootstrap_from_bundled().await?;
        }

        // Load bundled skills first (lower priority)
        let mut map: HashMap<String, SkillEntry> = HashMap::new();
        if self.bundled_dir.exists() {
            self.load_skills_from_dir(&self.bundled_dir, true, &mut map)
                .await?;
        }

        // Load workspace/managed skills (higher priority, overwrite bundled)
        self.load_skills_from_dir(&self.skills_dir, false, &mut map)
            .await?;

        let mut installed = self.installed.write().await;
        *installed = map;

        Ok(())
    }

    /// List all loaded skills.
    pub async fn list_skills(&self) -> Vec<SkillEntry> {
        let installed = self.installed.read().await;
        let mut skills: Vec<SkillEntry> = installed.values().cloned().collect();
        skills.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
        skills
    }

    /// Get a specific skill by name.
    pub async fn get_skill(&self, name: &str) -> Option<SkillEntry> {
        let installed = self.installed.read().await;
        installed.get(name).cloned()
    }

    /// Get skill content (markdown body without frontmatter).
    pub async fn get_skill_content(&self, name: &str) -> Option<String> {
        let installed = self.installed.read().await;
        installed.get(name).map(|e| e.skill.content.clone())
    }

    /// Build a skill snapshot for an agent run.
    pub async fn build_snapshot(
        &self,
        _agent_id: Option<&str>,
        skill_filter: Option<&[String]>,
    ) -> SkillSnapshot {
        let entries = self.list_skills().await;

        // Filter: eligible, not disabled, model-invocable
        let visible: Vec<&SkillEntry> = entries
            .iter()
            .filter(|e| {
                e.status != SkillStatus::Disabled
                    && e.eligibility.eligible
                    && !e.invocation.disable_model_invocation
            })
            .collect();

        // Apply skill filter
        let filtered: Vec<&SkillEntry> = if let Some(filter) = skill_filter {
            visible
                .into_iter()
                .filter(|e| filter.contains(&e.skill.name))
                .collect()
        } else {
            visible
        };

        let prompt = format_skills_for_prompt(&filtered);
        let skills = filtered
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
            .collect();

        SkillSnapshot {
            prompt,
            skills,
            skill_filter: skill_filter.map(|f| f.to_vec()),
        }
    }

    /// Enable or disable a skill. Persists to state.json in the skill directory.
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        {
            let mut installed = self.installed.write().await;
            if let Some(entry) = installed.get_mut(name) {
                let state = SkillState {
                    enabled,
                    installed_at: chrono::Utc::now().to_rfc3339(),
                    last_modified: chrono::Utc::now().to_rfc3339(),
                };
                let state_path = entry.skill.base_dir.join("state.json");
                tokio::fs::write(&state_path, serde_json::to_string_pretty(&state)?).await?;

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
                anyhow::bail!("skill not found: {}", name);
            }
        }
        Ok(())
    }

    /// Create a new skill with the given metadata and content.
    pub async fn create_skill(&self, name: &str, description: &str, content: &str) -> Result<()> {
        let skill_dir = self.skills_dir.join(name);
        tokio::fs::create_dir_all(&skill_dir).await?;

        let skill_file = skill_dir.join("SKILL.md");
        let frontmatter = format!(
            "---\nname: {}\ndescription: {}\n---\n\n{}",
            name, description, content
        );
        tokio::fs::write(&skill_file, frontmatter).await?;

        // Reload this skill into cache
        let entry = Self::load_skill_entry(&skill_file, false)?;
        let mut installed = self.installed.write().await;
        installed.insert(name.to_string(), entry);

        Ok(())
    }

    /// Delete a skill by name.
    pub async fn delete_skill(&self, name: &str) -> Result<()> {
        let skill_dir = self.skills_dir.join(name);
        if skill_dir.exists() {
            tokio::fs::remove_dir_all(&skill_dir).await?;
        }
        let mut installed = self.installed.write().await;
        installed.remove(name);
        Ok(())
    }

    /// List skill metadata only (lightweight).
    pub async fn list_skills_meta(&self) -> Vec<SkillMeta> {
        let installed = self.installed.read().await;
        let mut metas: Vec<SkillMeta> = installed
            .values()
            .map(|e| SkillMeta::from(&e.skill))
            .collect();
        metas.sort_by(|a, b| a.name.cmp(&b.name));
        metas
    }

    /// Load a specific skill by name (returns the Skill struct).
    pub async fn load_skill(&self, name: &str) -> Result<Option<Skill>> {
        let installed = self.installed.read().await;
        Ok(installed.get(name).map(|e| e.skill.clone()))
    }

    /// Get the path to the skills directory.
    pub fn path(&self) -> &PathBuf {
        &self.skills_dir
    }

    // ── Internal helpers ──

    /// Load all SKILL.md files from a directory into the map.
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
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    match Self::load_skill_entry(&skill_file, bundled) {
                        Ok(skill_entry) => {
                            // Workspace skills override bundled
                            let name = skill_entry.skill.name.clone();
                            if bundled && map.contains_key(&name) {
                                continue; // don't overwrite workspace skill
                            }
                            map.insert(name, skill_entry);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "failed to parse skill {:?}: {}",
                                skill_file,
                                e
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Load a single SKILL.md into a SkillEntry.
    fn load_skill_entry(skill_file: &Path, bundled: bool) -> Result<SkillEntry> {
        let content = std::fs::read_to_string(skill_file)
            .with_context(|| format!("reading {:?}", skill_file))?;

        let (parsed, body) = parse_frontmatter(&content)?;

        // Fallback name from directory name
        let name = if parsed.name.is_empty() {
            skill_file
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            parsed.name
        };

        let skill_dir = skill_file
            .parent()
            .context("SKILL.md has no parent directory")?
            .to_path_buf();
        let base_dir = skill_dir
            .parent()
            .context("skill directory has no parent")?
            .to_path_buf();

        let skill = Skill {
            name: name.clone(),
            description: parsed.description,
            content: body,
            path: skill_file.to_path_buf(),
            base_dir,
            file_path: skill_file.to_path_buf(),
        };

        // Check requirements if metadata is present
        let (eligibility, status) = if let Some(ref meta) = parsed.metadata {
            let check = check_requirements(meta);
            let status = if check.eligible {
                SkillStatus::Ready
            } else {
                SkillStatus::NeedsSetup
            };
            (check, status)
        } else {
            // No metadata → always eligible
            (
                RequirementsCheck {
                    eligible: true,
                    ..Default::default()
                },
                SkillStatus::Ready,
            )
        };

        // Check for persisted state
        let status = {
            let state_path = skill.path.parent().unwrap().join("state.json");
            if state_path.exists() {
                if let Ok(state_content) = std::fs::read_to_string(&state_path) {
                    if let Ok(state) = serde_json::from_str::<SkillState>(&state_content) {
                        if !state.enabled {
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

        let source = if bundled {
            SkillSource::Bundled
        } else {
            SkillSource::Managed
        };

        Ok(SkillEntry {
            skill,
            frontmatter: parsed.raw,
            metadata: parsed.metadata,
            eligibility,
            status,
            bundled,
            source,
            invocation: parsed.invocation,
        })
    }

    /// Bootstrap skills_dir from bundled_dir.
    async fn bootstrap_from_bundled(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.bundled_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src = entry.path();
            if src.is_dir() {
                let skill_name = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let dest = self.skills_dir.join(skill_name);
                tokio::fs::create_dir_all(&dest).await?;

                let src_skill = src.join("SKILL.md");
                if src_skill.exists() {
                    let dest_skill = dest.join("SKILL.md");
                    if !dest_skill.exists() {
                        let content = tokio::fs::read_to_string(&src_skill).await?;
                        tokio::fs::write(&dest_skill, content).await?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if a directory is empty (or doesn't exist).
    async fn is_dir_empty(&self, dir: &Path) -> Result<bool> {
        if !dir.exists() {
            return Ok(true);
        }
        let mut entries = tokio::fs::read_dir(dir).await?;
        Ok(entries.next_entry().await?.is_none())
    }
}

// ─── Legacy SkillStore compatibility shim ──────────────────────────
//
// SkillStore is kept as a thin wrapper around SkillManager so existing
// callers (kernel_bridge, extension_api, supervisor) can migrate incrementally.
// The shim delegates to SkillManager internally.

/// Legacy skill store (thin wrapper around SkillManager).
#[derive(Clone)]
pub struct SkillStore {
    manager: std::sync::Arc<tokio::sync::RwLock<SkillManager>>,
}

impl std::fmt::Debug for SkillStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillStore").finish()
    }
}

impl SkillStore {
    /// Create a new skill store pointing to the given directory.
    pub fn new(skills_dir: PathBuf) -> Result<Self> {
        let bundled_dir = skills_dir.join("../share/skills");
        let manager = SkillManager::new(skills_dir, bundled_dir);
        Ok(Self {
            manager: std::sync::Arc::new(tokio::sync::RwLock::new(manager)),
        })
    }

    /// Create with explicit bundled dir.
    pub fn with_bundled(skills_dir: PathBuf, bundled_dir: PathBuf) -> Result<Self> {
        let manager = SkillManager::new(skills_dir, bundled_dir);
        Ok(Self {
            manager: std::sync::Arc::new(tokio::sync::RwLock::new(manager)),
        })
    }

    /// Initialize defaults (bootstrap from bundled).
    pub async fn init_defaults(&self, defaults_dir: &PathBuf) -> Result<()> {
        let skills_dir = {
            let mgr = self.manager.read().await;
            mgr.path().clone()
        };
        let manager = SkillManager::new(skills_dir, defaults_dir.clone());
        manager.init().await?;
        let mut mgr = self.manager.write().await;
        *mgr = manager;
        Ok(())
    }

    /// List all available skills with their metadata.
    pub async fn list_skills(&self) -> Result<Vec<SkillMeta>> {
        let mgr = self.manager.read().await;
        Ok(mgr.list_skills_meta().await)
    }

    /// Load a specific skill by name.
    pub async fn load_skill(&self, name: &str) -> Result<Option<Skill>> {
        let mgr = self.manager.read().await;
        mgr.load_skill(name).await
    }

    /// Create a new skill.
    pub async fn create_skill(&self, name: &str, description: &str, content: &str) -> Result<()> {
        let mgr = self.manager.read().await;
        mgr.create_skill(name, description, content).await
    }

    /// Delete a skill by name.
    pub async fn delete_skill(&self, name: &str) -> Result<()> {
        let mgr = self.manager.read().await;
        mgr.delete_skill(name).await
    }

    /// Get the path to the skills directory.
    pub fn path(&self) -> PathBuf {
        // Synchronous access — use try_read, fall back to default
        match self.manager.try_read() {
            Ok(mgr) => mgr.path().clone(),
            Err(_) => PathBuf::new(),
        }
    }

    /// Access the underlying SkillManager (read guard).
    pub async fn manager(&self) -> tokio::sync::RwLockReadGuard<'_, SkillManager> {
        self.manager.read().await
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frontmatter parser tests ──

    #[test]
    fn test_parse_frontmatter_with_metadata() {
        let content = r#"---
name: code-review
description: Guidelines for reviewing code changes
---

# Code Review

Follow these steps to review code effectively.
"#;
        let (parsed, rest) = parse_frontmatter(content).unwrap();
        assert_eq!(parsed.name, "code-review");
        assert_eq!(parsed.description, "Guidelines for reviewing code changes");
        assert!(rest.contains("Code Review"));
    }

    #[test]
    fn test_parse_frontmatter_no_metadata() {
        let content = "# Just a Title\n\nSome content";
        let (parsed, rest) = parse_frontmatter(content).unwrap();
        assert!(parsed.name.is_empty());
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
        let (parsed, _) = parse_frontmatter(content).unwrap();
        assert_eq!(parsed.name, "test-skill");
        assert_eq!(parsed.description, "A test skill");
    }

    #[test]
    fn test_parse_frontmatter_full_openclaw_format() {
        let content = r#"---
name: code-review
description: Deep code review with quality domain analysis
author: oxios
version: 1.0.0
emoji: 🔍
homepage: https://github.com/oxios/skills
requires:
  bins: ["git", "gh"]
  anyBins: ["ffmpeg", "avconv"]
  env: ["GITHUB_TOKEN"]
  config: ["github.pr.default-base"]
os: ["darwin", "linux"]
install:
  - kind: brew
    formula: git
    os: ["darwin"]
always: false
user-invocable: true
disable-model-invocation: false
---

# Code Review

Instructions here.
"#;
        let (parsed, rest) = parse_frontmatter(content).unwrap();
        assert_eq!(parsed.name, "code-review");
        assert_eq!(
            parsed.description,
            "Deep code review with quality domain analysis"
        );

        let meta = parsed.metadata.unwrap();
        assert_eq!(meta.author.as_deref(), Some("oxios"));
        assert_eq!(meta.version.as_deref(), Some("1.0.0"));
        assert_eq!(meta.emoji.as_deref(), Some("🔍"));
        assert_eq!(
            meta.homepage.as_deref(),
            Some("https://github.com/oxios/skills")
        );

        // Requirements
        assert_eq!(meta.requires.bins, vec!["git", "gh"]);
        assert_eq!(meta.requires.any_bins, vec!["ffmpeg", "avconv"]);
        assert_eq!(meta.requires.env, vec!["GITHUB_TOKEN"]);
        assert_eq!(
            meta.requires.config,
            vec!["github.pr.default-base"]
        );

        // OS
        assert_eq!(meta.os, vec!["darwin", "linux"]);

        // Install
        assert_eq!(meta.install.len(), 1);
        assert_eq!(meta.install[0].kind, InstallKind::Brew);
        assert_eq!(meta.install[0].formula.as_deref(), Some("git"));
        assert_eq!(meta.install[0].os, vec!["darwin"]);

        // Flags
        assert!(!meta.always);

        // Invocation
        assert!(parsed.invocation.user_invocable);
        assert!(!parsed.invocation.disable_model_invocation);

        // Body
        assert!(rest.contains("Code Review"));
        assert!(rest.contains("Instructions here."));
    }

    #[test]
    fn test_parse_frontmatter_multiple_install_specs() {
        let content = r#"---
name: deploy
description: Deploy services
install:
  - kind: brew
    formula: curl
    os: ["darwin"]
  - kind: download
    url: https://example.com/tool.tar.gz
    extract: true
    stripComponents: 1
  - kind: node
    package: typescript
  - kind: go
    module: golang.org/x/tools/cmd/goimports
  - kind: uv
    package: black
---

Content.
"#;
        let (parsed, _) = parse_frontmatter(content).unwrap();
        let meta = parsed.metadata.unwrap();
        assert_eq!(meta.install.len(), 5);

        assert_eq!(meta.install[0].kind, InstallKind::Brew);
        assert_eq!(meta.install[0].formula.as_deref(), Some("curl"));

        assert_eq!(meta.install[1].kind, InstallKind::Download);
        assert_eq!(
            meta.install[1].url.as_deref(),
            Some("https://example.com/tool.tar.gz")
        );
        assert_eq!(meta.install[1].extract, Some(true));
        assert_eq!(meta.install[1].strip_components, Some(1));

        assert_eq!(meta.install[2].kind, InstallKind::Node);
        assert_eq!(meta.install[2].package.as_deref(), Some("typescript"));

        assert_eq!(meta.install[3].kind, InstallKind::Go);
        assert_eq!(
            meta.install[3].module.as_deref(),
            Some("golang.org/x/tools/cmd/goimports")
        );

        assert_eq!(meta.install[4].kind, InstallKind::Uv);
        assert_eq!(meta.install[4].package.as_deref(), Some("black"));
    }

    #[test]
    fn test_parse_frontmatter_always_flag() {
        let content = r#"---
name: base
description: Base skill
always: true
---

Content.
"#;
        let (parsed, _) = parse_frontmatter(content).unwrap();
        let meta = parsed.metadata.unwrap();
        assert!(meta.always);
    }

    #[test]
    fn test_parse_frontmatter_empty_requires() {
        let content = r#"---
name: minimal
description: Minimal skill
requires:
  bins: []
  anyBins: []
  env: []
  config: []
---

Content.
"#;
        let (parsed, _) = parse_frontmatter(content).unwrap();
        let meta = parsed.metadata.unwrap();
        assert!(meta.requires.bins.is_empty());
        assert!(meta.requires.any_bins.is_empty());
        assert!(meta.requires.env.is_empty());
        assert!(meta.requires.config.is_empty());
    }

    #[test]
    fn test_parse_frontmatter_no_requires_block() {
        let content = r#"---
name: simple
description: Simple skill
---

Content.
"#;
        let (parsed, _) = parse_frontmatter(content).unwrap();
        let meta = parsed.metadata.unwrap();
        assert!(meta.requires.bins.is_empty());
        assert!(meta.requires.env.is_empty());
    }

    #[test]
    fn test_parse_frontmatter_unclosed() {
        let content = "---\nname: test\nno closing";
        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }

    // ── Requirements evaluation tests ──

    #[test]
    fn test_check_requirements_no_requirements() {
        let meta = SkillMetadata::default();
        let check = check_requirements(&meta);
        assert!(check.eligible);
        assert!(check.missing_bins.is_empty());
        assert!(check.missing_env.is_empty());
    }

    #[test]
    fn test_check_requirements_always_true() {
        let mut meta = SkillMetadata::default();
        meta.always = true;
        meta.requires.bins = vec!["this-tool-does-not-exist-xyz".into()];
        let check = check_requirements(&meta);
        assert!(check.eligible);
    }

    #[test]
    fn test_check_requirements_existing_bin() {
        let mut meta = SkillMetadata::default();
        // 'echo' exists on all platforms
        meta.requires.bins = vec!["echo".into()];
        let check = check_requirements(&meta);
        assert!(check.eligible);
        assert!(check.missing_bins.is_empty());
    }

    #[test]
    fn test_check_requirements_missing_bin() {
        let mut meta = SkillMetadata::default();
        meta.requires.bins = vec!["this-tool-does-not-exist-xyz".into()];
        let check = check_requirements(&meta);
        assert!(!check.eligible);
        assert_eq!(check.missing_bins, vec!["this-tool-does-not-exist-xyz"]);
    }

    #[test]
    fn test_check_requirements_missing_env() {
        let mut meta = SkillMetadata::default();
        meta.requires.env = vec!["OXIOS_TEST_MISSING_ENV_VAR".into()];
        let check = check_requirements(&meta);
        assert!(!check.eligible);
        assert_eq!(check.missing_env, vec!["OXIOS_TEST_MISSING_ENV_VAR"]);
    }

    #[test]
    fn test_check_requirements_any_bins_satisfied() {
        let mut meta = SkillMetadata::default();
        // At least one must exist: 'echo' does
        meta.requires.any_bins = vec!["echo".into(), "nonexistent-tool-xyz".into()];
        let check = check_requirements(&meta);
        assert!(check.eligible);
        assert!(check.missing_any_bins.is_empty());
    }

    #[test]
    fn test_check_requirements_any_bins_unsatisfied() {
        let mut meta = SkillMetadata::default();
        meta.requires.any_bins = vec![
            "nonexistent-a-xyz".into(),
            "nonexistent-b-xyz".into(),
        ];
        let check = check_requirements(&meta);
        assert!(!check.eligible);
        assert_eq!(check.missing_any_bins.len(), 2);
    }

    #[test]
    fn test_check_requirements_os_current() {
        let mut meta = SkillMetadata::default();
        // Include current platform
        let platform = current_platform().to_string();
        meta.os = vec![platform, "other".into()];
        let check = check_requirements(&meta);
        assert!(check.eligible);
        assert!(check.missing_os.is_empty());
    }

    #[test]
    fn test_check_requirements_os_excluded() {
        let mut meta = SkillMetadata::default();
        // Use an OS that is not the current one
        meta.os = vec!["nonexistent-os".into()];
        let check = check_requirements(&meta);
        assert!(!check.eligible);
        assert!(!check.missing_os.is_empty());
    }

    // ── XML formatting tests ──

    #[test]
    fn test_format_skills_for_prompt_empty() {
        let result = format_skills_for_prompt(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_skills_for_prompt() {
        let skill = Skill {
            name: "code-review".into(),
            description: "Deep code review".into(),
            content: String::new(),
            path: PathBuf::from("/home/user/.oxios/skills/code-review/SKILL.md"),
            base_dir: PathBuf::from("/home/user/.oxios/skills"),
            file_path: PathBuf::from("/home/user/.oxios/skills/code-review/SKILL.md"),
        };
        let entry = SkillEntry {
            skill,
            frontmatter: HashMap::new(),
            metadata: None,
            eligibility: RequirementsCheck {
                eligible: true,
                ..Default::default()
            },
            status: SkillStatus::Ready,
            bundled: false,
            source: SkillSource::Managed,
            invocation: SkillInvocationPolicy::default(),
        };

        let result = format_skills_for_prompt(&[&entry]);
        assert!(result.contains("<available_skills>"));
        assert!(result.contains("<name>code-review</name>"));
        assert!(result.contains("<description>Deep code review</description>"));
        assert!(result.contains("</available_skills>"));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }

    #[test]
    fn test_compact_path() {
        let home = dirs::home_dir().unwrap();
        let full = home.join("skills/test/SKILL.md");
        let compacted = compact_path(&full);
        assert!(compacted.starts_with("~/"));
    }

    // ── Inline list parser tests ──

    #[test]
    fn test_parse_inline_list_brackets() {
        assert_eq!(
            parse_inline_list(r#"["git", "gh", "curl"]"#),
            vec!["git", "gh", "curl"]
        );
    }

    #[test]
    fn test_parse_inline_list_empty() {
        assert_eq!(parse_inline_list("[]"), Vec::<String>::new());
    }

    #[test]
    fn test_parse_inline_list_single_quotes() {
        assert_eq!(
            parse_inline_list("['git', 'gh']"),
            vec!["git", "gh"]
        );
    }

    // ── SkillStatus tests ──

    #[test]
    fn test_skill_status_display() {
        assert_eq!(SkillStatus::Ready.to_string(), "ready");
        assert_eq!(SkillStatus::NeedsSetup.to_string(), "needs_setup");
        assert_eq!(SkillStatus::Disabled.to_string(), "disabled");
    }

    #[test]
    fn test_skill_status_serde() {
        let json = serde_json::to_string(&SkillStatus::Ready).unwrap();
        assert_eq!(json, "\"ready\"");
        let loaded: SkillStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, SkillStatus::Ready);
    }

    // ── InstallKind tests ──

    #[test]
    fn test_install_kind_display() {
        assert_eq!(InstallKind::Brew.to_string(), "brew");
        assert_eq!(InstallKind::Node.to_string(), "node");
        assert_eq!(InstallKind::Go.to_string(), "go");
        assert_eq!(InstallKind::Uv.to_string(), "uv");
        assert_eq!(InstallKind::Download.to_string(), "download");
    }

    #[test]
    fn test_install_kind_serde() {
        let json = serde_json::to_string(&InstallKind::Brew).unwrap();
        assert_eq!(json, "\"brew\"");
        let loaded: InstallKind = serde_json::from_str("\"brew\"").unwrap();
        assert_eq!(loaded, InstallKind::Brew);
    }

    // ── SkillState tests ──

    #[test]
    fn test_skill_state_default() {
        let state = SkillState::default();
        assert!(state.enabled);
        assert!(!state.installed_at.is_empty());
        assert!(!state.last_modified.is_empty());
    }
}
