//! Skill system: multi-format SKILL.md parsing with format detection.

pub mod format;
pub mod frontmatter;
pub mod manager;
pub mod prompt;
pub mod requirements;
pub mod types;

pub use format::SkillFormat;
pub use manager::SkillManager;
pub use prompt::{compact_path, escape_xml};
pub use requirements::check_requirements;
pub use types::{
    ConfigCheck, InstallKind, Requirements, RequirementsCheck, Skill, SkillConfig, SkillEntry,
    SkillInstallSpec, SkillInvocationPolicy, SkillMeta, SkillMetadata, SkillRef, SkillSnapshot,
    SkillSource, SkillState, SkillStatus,
};
