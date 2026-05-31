#![allow(missing_docs)]
//! Skill format detection.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

/// Detected skill format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillFormat {
    Oxios,
    OpenClaw,
    ClaudeCode,
    AgentSkills,
}

impl std::fmt::Display for SkillFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillFormat::Oxios => write!(f, "oxios"),
            SkillFormat::OpenClaw => write!(f, "openclaw"),
            SkillFormat::ClaudeCode => write!(f, "claude_code"),
            SkillFormat::AgentSkills => write!(f, "agent_skills"),
        }
    }
}

pub fn detect_format(value: &Value) -> SkillFormat {
    if let Some(meta) = value.get("metadata") {
        if meta.get("openclaw").is_some()
            || meta.get("clawdbot").is_some()
            || meta.get("clawdis").is_some()
        {
            return SkillFormat::OpenClaw;
        }
    }
    for key in &[
        "allowed-tools",
        "arguments",
        "when_to_use",
        "argument-hint",
        "effort",
        "hooks",
        "paths",
    ] {
        if value.get(*key).is_some() {
            return SkillFormat::ClaudeCode;
        }
    }
    for key in &[
        "requires",
        "install",
        "primaryEnv",
        "primary-env",
        "skillKey",
        "skill-key",
    ] {
        if value.get(*key).is_some() {
            return SkillFormat::Oxios;
        }
    }
    SkillFormat::AgentSkills
}

pub fn resolve_format(value: &Value, skill_dir: &Path) -> SkillFormat {
    if skill_dir.join(".clawhub").join("origin.json").exists() {
        return SkillFormat::OpenClaw;
    }
    detect_format(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_detect_oxios() {
        let v: Value = serde_yaml::from_str("name: test\nrequires:\n  bins:\n    - git\n").unwrap();
        assert_eq!(detect_format(&v), SkillFormat::Oxios);
    }
    #[test]
    fn test_detect_openclaw() {
        let v: Value = serde_yaml::from_str(
            "name: test\nmetadata:\n  openclaw:\n    requires:\n      env:\n        - KEY\n",
        )
        .unwrap();
        assert_eq!(detect_format(&v), SkillFormat::OpenClaw);
    }
    #[test]
    fn test_detect_claude() {
        let v: Value = serde_yaml::from_str("name: test\nallowed-tools: Read Grep\n").unwrap();
        assert_eq!(detect_format(&v), SkillFormat::ClaudeCode);
    }
    #[test]
    fn test_detect_standard() {
        let v: Value = serde_yaml::from_str("name: test\ndescription: A skill\n").unwrap();
        assert_eq!(detect_format(&v), SkillFormat::AgentSkills);
    }
}
