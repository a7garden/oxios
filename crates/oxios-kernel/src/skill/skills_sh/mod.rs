//! Skills.sh marketplace integration.
//!
//! Provides a client for searching and fetching skills from the [skills.sh]
//! registry, plus an installer that writes skill files directly (no zip
//! extraction needed — skills.sh provides file contents via JSON API).
//!
//! # Comparison with ClawHub
//!
//! | Aspect       | ClawHub                        | Skills.sh                       |
//! |--------------|--------------------------------|----------------------------------|
//! | Distribution | Zip archive download           | JSON API with file contents      |
//! | Discovery    | ClawHub search API             | Skills.sh search + leaderboard   |
//! | Source       | `clawhub.ai`                   | GitHub repos via skills.sh       |
//! | Auth         | `CLAWHUB_TOKEN` env var        | `SKILLS_SH_TOKEN` env var        |
//!
//! # Directory Layout
//!
//! ```text
//! workspace/
//!   skills/
//!     frontend-design/
//!       SKILL.md
//!       .skills_sh/
//!         origin.json   ← per-skill origin metadata
//! ```
//!
//! # Example
//!
//! ```no_run
//! use oxios_kernel::skills_sh::{SkillsShInstaller, SkillsShInstallResult};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let installer = SkillsShInstaller::new(
//!     PathBuf::from("/home/user/.oxios/skills"),
//!     None,  // default base URL
//!     None,  // no API key (reads SKILLS_SH_TOKEN env var)
//! );
//!
//! let result = installer
//!     .install("vercel-labs/agent-skills/frontend-design")
//!     .await?;
//! println!("Installed {} files to {}", result.file_count, result.target_dir.display());
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod installer;
pub mod types;

pub use client::SkillsShClient;
pub use installer::{SkillsShInstallResult, SkillsShInstaller, SkillsShOrigin};
pub use types::{
    SkillsShAuditEntry, SkillsShAuditResponse, SkillsShCuratedOwner, SkillsShCuratedResponse,
    SkillsShFile, SkillsShListResponse, SkillsShPagination, SkillsShSearchResponse,
    SkillsShSkill, SkillsShSkillDetail,
};
