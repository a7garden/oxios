#![allow(missing_docs, dead_code)]
//! Format-specific frontmatter types and unified parsing pipeline.

use super::format::{SkillFormat, resolve_format};
use super::types::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::Value;
use std::path::Path;

// ─── YAML helpers ──────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct YamlRequirements {
    pub bins: Option<Vec<String>>,
    #[serde(default, rename = "anyBins")]
    pub any_bins: Option<Vec<String>>,
    pub env: Option<Vec<String>>,
    pub config: Option<Vec<String>>,
}
impl YamlRequirements {
    pub fn into_requirements(self) -> Requirements {
        Requirements {
            bins: self.bins.unwrap_or_default(),
            any_bins: self.any_bins.unwrap_or_default(),
            env: self.env.unwrap_or_default(),
            config: self.config.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct YamlInstallSpec {
    pub kind: Option<String>,
    pub formula: Option<String>,
    pub package: Option<String>,
    pub module: Option<String>,
    pub url: Option<String>,
    pub archive: Option<String>,
    pub extract: Option<bool>,
    #[serde(rename = "stripComponents")]
    pub strip_components: Option<u32>,
    #[serde(rename = "targetDir")]
    pub target_dir: Option<String>,
    pub os: Option<Vec<String>>,
}
impl From<YamlInstallSpec> for SkillInstallSpec {
    fn from(y: YamlInstallSpec) -> Self {
        SkillInstallSpec {
            kind: match y.kind.as_deref() {
                Some("brew") => InstallKind::Brew,
                Some("node") => InstallKind::Node,
                Some("go") => InstallKind::Go,
                Some("uv") => InstallKind::Uv,
                Some("download") => InstallKind::Download,
                _ => InstallKind::Brew,
            },
            formula: y.formula,
            package: y.package,
            module: y.module,
            url: y.url,
            archive: y.archive,
            extract: y.extract,
            strip_components: y.strip_components,
            target_dir: y.target_dir,
            os: y.os.unwrap_or_default(),
        }
    }
}

// ─── ParsedSkill ──────────────────────────────────────────────

pub struct ParsedSkill {
    pub name: String,
    pub description: String,
    pub metadata: SkillMetadata,
    pub invocation: SkillInvocationPolicy,
    pub format: SkillFormat,
    pub raw_yaml: Value,
}

// ─── Oxios ────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct OxiosFm {
    name: Option<String>,
    description: Option<String>,
    author: Option<String>,
    version: Option<String>,
    emoji: Option<String>,
    homepage: Option<String>,
    requires: Option<YamlRequirements>,
    install: Option<Vec<YamlInstallSpec>>,
    os: Option<Vec<String>>,
    always: Option<bool>,
    #[serde(rename = "primaryEnv")]
    primary_env: Option<String>,
    #[serde(rename = "skillKey")]
    skill_key: Option<String>,
    #[serde(rename = "user-invocable")]
    user_invocable: Option<bool>,
    #[serde(rename = "disable-model-invocation")]
    disable_model_invocation: Option<bool>,
}
impl OxiosFm {
    fn into_parsed(self, raw: Value) -> ParsedSkill {
        ParsedSkill {
            name: self.name.unwrap_or_default(),
            description: self.description.unwrap_or_default(),
            metadata: SkillMetadata {
                author: self.author,
                version: self.version,
                emoji: self.emoji,
                homepage: self.homepage,
                requires: self.requires.unwrap_or_default().into_requirements(),
                install: self
                    .install
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                os: self.os.unwrap_or_default(),
                always: self.always.unwrap_or(false),
                primary_env: self.primary_env,
                skill_key: self.skill_key,
            },
            invocation: SkillInvocationPolicy {
                user_invocable: self.user_invocable.unwrap_or(true),
                disable_model_invocation: self.disable_model_invocation.unwrap_or(false),
            },
            format: SkillFormat::Oxios,
            raw_yaml: raw,
        }
    }
}

// ─── OpenClaw ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenClawFm {
    name: Option<String>,
    description: Option<String>,
    metadata: Option<OcMeta>,
}
#[derive(Deserialize)]
struct OcMeta {
    openclaw: Option<OcRuntime>,
    clawdbot: Option<OcRuntime>,
    clawdis: Option<OcRuntime>,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct OcRuntime {
    requires: Option<YamlRequirements>,
    install: Option<Vec<YamlInstallSpec>>,
    #[serde(rename = "primaryEnv")]
    primary_env: Option<String>,
    #[serde(rename = "envVars")]
    env_vars: Option<Vec<OcEnvVar>>,
    always: Option<bool>,
    #[serde(rename = "skillKey")]
    skill_key: Option<String>,
    emoji: Option<String>,
    version: Option<String>,
    author: Option<String>,
    homepage: Option<String>,
}
#[derive(Deserialize)]
struct OcEnvVar {
    name: String,
    #[serde(default = "default_true")]
    required: bool,
}

impl OpenClawFm {
    fn into_parsed(self, raw: Value) -> ParsedSkill {
        let rt = self
            .metadata
            .and_then(|m| m.openclaw.or(m.clawdbot).or(m.clawdis));
        let (reqs, install, penv, sk, alw, em, ver, auth, hp, evars) = match rt {
            Some(r) => (
                r.requires.unwrap_or_default(),
                r.install.unwrap_or_default(),
                r.primary_env,
                r.skill_key,
                r.always.unwrap_or(false),
                r.emoji,
                r.version,
                r.author,
                r.homepage,
                r.env_vars.unwrap_or_default(),
            ),
            None => Default::default(),
        };
        let mut env = reqs.env.unwrap_or_default();
        for ev in &evars {
            if ev.required && !env.contains(&ev.name) {
                env.push(ev.name.clone());
            }
        }
        ParsedSkill {
            name: self.name.unwrap_or_default(),
            description: self.description.unwrap_or_default(),
            metadata: SkillMetadata {
                author: auth,
                version: ver,
                emoji: em,
                homepage: hp,
                requires: Requirements {
                    bins: reqs.bins.unwrap_or_default(),
                    any_bins: reqs.any_bins.unwrap_or_default(),
                    env,
                    config: reqs.config.unwrap_or_default(),
                },
                install: install.into_iter().map(Into::into).collect(),
                primary_env: penv,
                skill_key: sk,
                always: alw,
                ..Default::default()
            },
            invocation: SkillInvocationPolicy::default(),
            format: SkillFormat::OpenClaw,
            raw_yaml: raw,
        }
    }
}

// ─── Claude Code ──────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ClaudeFm {
    name: Option<String>,
    description: Option<String>,
    allowed_tools: Option<Value>,
    arguments: Option<Value>,
    #[serde(rename = "when_to_use")]
    when_to_use: Option<String>,
    argument_hint: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    context: Option<String>,
    agent: Option<String>,
    paths: Option<Value>,
    hooks: Option<Value>,
    shell: Option<String>,
    #[serde(rename = "disable-model-invocation")]
    disable_model_invocation: Option<bool>,
    #[serde(rename = "user-invocable")]
    user_invocable: Option<bool>,
    license: Option<String>,
    compatibility: Option<String>,
}
impl ClaudeFm {
    fn into_parsed(self, raw: Value) -> ParsedSkill {
        let description = match &self.when_to_use {
            Some(wtu) if !wtu.is_empty() => {
                let b = self.description.as_deref().unwrap_or("");
                if b.contains(wtu) {
                    b.to_string()
                } else {
                    format!("{b} {wtu}")
                }
            }
            _ => self.description.unwrap_or_default(),
        };
        ParsedSkill {
            name: self.name.unwrap_or_default(),
            description,
            metadata: SkillMetadata::default(),
            invocation: SkillInvocationPolicy {
                user_invocable: self.user_invocable.unwrap_or(true),
                disable_model_invocation: self.disable_model_invocation.unwrap_or(false),
            },
            format: SkillFormat::ClaudeCode,
            raw_yaml: raw,
        }
    }
}

// ─── Agent Skills standard ────────────────────────────────────

#[derive(Deserialize)]
struct StandardFm {
    name: Option<String>,
    description: Option<String>,
    license: Option<String>,
    compatibility: Option<String>,
    metadata: Option<Value>,
}
impl StandardFm {
    fn into_parsed(self, raw: Value) -> ParsedSkill {
        ParsedSkill {
            name: self.name.unwrap_or_default(),
            description: self.description.unwrap_or_default(),
            metadata: SkillMetadata::default(),
            invocation: SkillInvocationPolicy::default(),
            format: SkillFormat::AgentSkills,
            raw_yaml: raw,
        }
    }
}

// ─── Pipeline ─────────────────────────────────────────────────

pub fn parse_skill(content: &str, skill_dir: &Path) -> Result<(ParsedSkill, String)> {
    let (yaml_str, body) = split_frontmatter(content)?;
    if yaml_str.trim().is_empty() {
        return Ok((
            ParsedSkill {
                name: String::new(),
                description: String::new(),
                metadata: SkillMetadata::default(),
                invocation: SkillInvocationPolicy::default(),
                format: SkillFormat::AgentSkills,
                raw_yaml: Value::Null,
            },
            body,
        ));
    }
    let value: Value =
        serde_yaml::from_str(&yaml_str).with_context(|| "invalid YAML frontmatter")?;
    let format = resolve_format(&value, skill_dir);
    let parsed = match format {
        SkillFormat::Oxios => {
            let fm: OxiosFm =
                serde_yaml::from_value(value.clone()).with_context(|| "Oxios frontmatter")?;
            fm.into_parsed(value)
        }
        SkillFormat::OpenClaw => {
            let fm: OpenClawFm =
                serde_yaml::from_value(value.clone()).with_context(|| "OpenClaw frontmatter")?;
            fm.into_parsed(value)
        }
        SkillFormat::ClaudeCode => {
            let fm: ClaudeFm =
                serde_yaml::from_value(value.clone()).with_context(|| "Claude frontmatter")?;
            fm.into_parsed(value)
        }
        SkillFormat::AgentSkills => {
            let fm: StandardFm =
                serde_yaml::from_value(value.clone()).with_context(|| "Standard frontmatter")?;
            fm.into_parsed(value)
        }
    };
    Ok((parsed, sanitize_body(&body, format)))
}

fn split_frontmatter(content: &str) -> Result<(String, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok((String::new(), content.to_string()));
    }
    let after = &trimmed[3..];
    let end = after.find("---").context("unclosed frontmatter")?;
    Ok((
        after[..end].to_string(),
        after[end + 3..].trim_start().to_string(),
    ))
}

fn sanitize_body(body: &str, format: SkillFormat) -> String {
    if format != SkillFormat::ClaudeCode {
        return body.to_string();
    }
    let mut result = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '!' && chars.peek() == Some(&'`') {
            chars.next();
            let mut cmd = String::new();
            let mut found = false;
            for cc in chars.by_ref() {
                if cc == '`' {
                    found = true;
                    break;
                }
                cmd.push(cc);
            }
            if found {
                result.push_str(&format!(
                    "<!-- !`{cmd}` (Claude Code dynamic injection, not active in Oxios) -->"
                ));
            } else {
                result.push('!');
                result.push('`');
                result.push_str(&cmd);
            }
        } else {
            result.push(c);
        }
    }
    result
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_split() {
        let (y, b) = split_frontmatter("---\nname: x\n---\n\nBody\n").unwrap();
        assert!(y.contains("name"));
        assert!(b.contains("Body"));
    }
    #[test]
    fn test_split_none() {
        let (y, _) = split_frontmatter("# No fm").unwrap();
        assert!(y.is_empty());
    }
    #[test]
    fn test_split_unclosed() {
        assert!(split_frontmatter("---\nname: x").is_err());
    }
    #[test]
    fn test_oxios_basic() {
        let d = tempfile::tempdir().unwrap();
        // Oxios format is detected by presence of `requires`, `install`, `primaryEnv`, or `skillKey` keys.
        let (p, b) = parse_skill(
            "---\nname: test\ndescription: desc\nrequires:\n  bins:\n    - git\n---\n\nBody\n",
            d.path(),
        )
        .unwrap();
        assert_eq!(p.format, SkillFormat::Oxios);
        assert_eq!(p.name, "test");
        assert!(b.contains("Body"));
    }
    #[test]
    fn test_oxios_full() {
        let d = tempfile::tempdir().unwrap();
        let c = "---\nname: cr\ndescription: review\nauthor: me\nrequires:\n  bins:\n    - git\n  env:\n    - TOKEN\ninstall:\n  - kind: brew\n    formula: git\nalways: false\n---\n\n# Review\n";
        let (p, _) = parse_skill(c, d.path()).unwrap();
        assert_eq!(p.metadata.requires.bins, vec!["git"]);
        assert_eq!(p.metadata.requires.env, vec!["TOKEN"]);
        assert_eq!(p.metadata.install.len(), 1);
    }
    #[test]
    fn test_openclaw_nested() {
        let d = tempfile::tempdir().unwrap();
        let c = "---\nname: todo\nmetadata:\n  openclaw:\n    requires:\n      env:\n        - KEY\n    primaryEnv: KEY\n---\n\n# Body\n";
        let (p, _) = parse_skill(c, d.path()).unwrap();
        assert_eq!(p.format, SkillFormat::OpenClaw);
        assert_eq!(p.metadata.requires.env, vec!["KEY"]);
        assert_eq!(p.metadata.primary_env.as_deref(), Some("KEY"));
    }
    #[test]
    fn test_openclaw_envvars_merge() {
        let d = tempfile::tempdir().unwrap();
        // envVars is a separate field in OpenClaw runtime; must also have requires.env or
        // the merge logic adds envVar names to the env list.
        let c = "---\nname: t\nmetadata:\n  openclaw:\n    requires:\n      env:\n        - KEY\n    envVars:\n      - name: AUTO\n        required: true\n---\n\n";
        let (p, _) = parse_skill(c, d.path()).unwrap();
        assert!(
            p.metadata.requires.env.contains(&"KEY".to_string()),
            "KEY from requires.env should be present"
        );
        assert!(
            p.metadata.requires.env.contains(&"AUTO".to_string()),
            "AUTO from envVars should be merged"
        );
    }
    #[test]
    fn test_claude() {
        let d = tempfile::tempdir().unwrap();
        let c = "---\nname: deploy\nallowed-tools: Bash\ndisable-model-invocation: true\n---\n\nDeploy.\n";
        let (p, _) = parse_skill(c, d.path()).unwrap();
        assert_eq!(p.format, SkillFormat::ClaudeCode);
        assert!(p.invocation.disable_model_invocation);
    }
    #[test]
    fn test_claude_when_to_use() {
        let d = tempfile::tempdir().unwrap();
        // when_to_use key triggers ClaudeCode format detection.
        // ClaudeCode's into_parsed appends when_to_use to description.
        let c = "---\nname: s\ndescription: Sum\nwhen_to_use: use when changed\n---\n\n";
        let (p, _) = parse_skill(c, d.path()).unwrap();
        assert_eq!(
            p.format,
            SkillFormat::ClaudeCode,
            "should be detected as ClaudeCode"
        );
        // description should be "Sum use when changed"
        assert!(
            p.description.contains("Sum"),
            "should contain base description"
        );
        assert!(
            p.description.contains("changed"),
            "should contain when_to_use content"
        );
    }
    #[test]
    fn test_sanitize() {
        let safe = sanitize_body("See !`git diff`\n", SkillFormat::ClaudeCode);
        assert!(safe.contains("<!--"));
        assert!(!safe.contains("!["));
    }
    #[test]
    fn test_sanitize_skip() {
        assert_eq!(sanitize_body("a!`b`", SkillFormat::Oxios), "a!`b`");
    }
    #[test]
    fn test_standard() {
        // name + description only → no Oxios/Claude/OpenClaw keys → AgentSkills format.
        let d = tempfile::tempdir().unwrap();
        let (p, _) = parse_skill("---\nname: s\ndescription: d\n---\n\n", d.path()).unwrap();
        assert_eq!(p.format, SkillFormat::AgentSkills);
    }
    #[test]
    fn test_oxios_name_desc_only() {
        // name + description without requires/install → falls through to AgentSkills.
        let d = tempfile::tempdir().unwrap();
        let (p, _) = parse_skill(
            "---\nname: test\ndescription: desc\n---\n\nBody\n",
            d.path(),
        )
        .unwrap();
        assert_eq!(
            p.format,
            SkillFormat::AgentSkills,
            "name+description only should be AgentSkills, not Oxios"
        );
        assert_eq!(p.name, "test");
    }
}
