//! ClawHub marketplace integration.
//!
//! Provides a client for searching and downloading skills from the ClawHub
//! registry, plus an installer that handles zip extraction, origin tracking,
//! and lockfile management.
//!
//! # Directory Layout
//!
//! ```text
//! workspace/
//!   .clawhub/
//!     lock.json         ← lockfile (all installed ClawHub skills)
//!   skills/
//!     code-review/
//!       SKILL.md
//!       .clawhub/
//!         origin.json   ← per-skill origin metadata
//! ```
//!
//! # Example
//!
//! ```ignore
//! use oxios_kernel::skill::clawhub::{ClawHubInstaller, InstallResult};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let installer = ClawHubInstaller::new(
//!     PathBuf::from("/home/user/.oxios/skills"),
//!     PathBuf::from("/home/user/oxios-workspace"),
//!     None,
//! );
//!
//! let result = installer.install("code-review-helper", None).await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod installer;
pub mod types;

pub use client::{ClawHubClient, DownloadedArchive};
pub use installer::{ClawHubInstaller, InstallResult, UpdateAvailable, UpdateResult};
pub use types::{
    ClawHubLockEntry, ClawHubLockfile, ClawHubMetadata, ClawHubOrigin, ClawHubOwner,
    ClawHubSearchResult, ClawHubSkillDetail, ClawHubSkillMeta, ClawHubVersion, SearchResponse,
};
