//! Mount module: path-alias system (RFC-025).
//!
//! A **Mount** is a lightweight name bound to one or more filesystem paths
//! (`oxios` → `/Volumes/MERCURY/PROJECTS/oxios`). It is the path-alias role
//! that RFC-011's `Project` conflated with memory partitioning.
//!
//! The agent explores a Mount's paths with tools (`ls`/`read`/`grep`) and
//! accumulates `auto_description` / `auto_meta` over time. Mounts are living
//! objects — they are refreshed during sessions, on marker drift, and during
//! Dream consolidation (RFC-008).
//!
//! ## Structure
//!
//! - `mod.rs` — `Mount`, `MountMeta`, `MountSource`, `MountId` (this file)
//! - `mount_db.rs` — SQLite persistence (`mounts` table)
//! - `manager.rs` — `MountManager` (CRUD, lookup, detection, touch)
//! - `detection.rs` — `detect_mounts` (name/path/meta matching)

pub mod detection;
pub mod manager;
pub mod meta_detection;
pub mod mount_db;
pub mod path_promotion;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Re-exports ──────────────────────────────────────────────
pub use detection::{DetectionResult, detect_mounts, extract_path, find_by_id, find_by_name};
pub use manager::{MountManager, MountManagerError};
pub use meta_detection::{detect_meta, snapshot_markers};
pub use path_promotion::{PathFrequency, PromotionConfig};

/// Unique identifier for a Mount.
pub type MountId = Uuid;

/// How a Mount was registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountSource {
    /// User explicitly created via UI/CLI.
    #[default]
    Manual,
    /// OS auto-detected from a path in the conversation.
    AutoDetected,
    /// RFC-025 Phase 5: auto-promoted from a frequently-used path.
    /// Created by the background scanner when a path crosses the frequency
    /// threshold. Distinguishable in the UI so users can review/prune them.
    AutoPromoted,
}

impl std::fmt::Display for MountSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountSource::Manual => write!(f, "manual"),
            MountSource::AutoDetected => write!(f, "auto_detected"),
            MountSource::AutoPromoted => write!(f, "auto_promoted"),
        }
    }
}

/// Auto-detected metadata, written/refined by the agent as it explores.
///
/// Replaces RFC-011's manual `tags`. Seeded by cheap heuristics on marker
/// files (`Cargo.toml`, `package.json`, …) at drift-detection time, then
/// refined by the agent during enrichment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MountMeta {
    /// Detected programming languages (e.g. `["rust", "typescript"]`).
    #[serde(default)]
    pub languages: Vec<String>,
    /// Detected stack / key dependencies (e.g. `["tokio", "axum", "react"]`).
    #[serde(default)]
    pub stack: Vec<String>,
    /// Detected marker files (e.g. `["Cargo.toml", "AGENTS.md"]`).
    #[serde(default)]
    pub markers: Vec<String>,
    /// One-line derived summary.
    #[serde(default)]
    pub summary: String,
}

/// A path alias: a name bound to one or more filesystem paths.
///
/// The agent explores the path(s) with tools and writes
/// [`auto_description`](Self::auto_description) /
/// [`auto_meta`](Self::auto_meta) over time. `paths[0]` is the CWD when this
/// Mount is the session's primary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    /// Unique identifier.
    pub id: MountId,
    /// Human-readable name (unique, e.g. "oxios").
    pub name: String,
    /// Filesystem paths. `paths[0]` is CWD when this Mount is primary.
    /// Must contain ≥1 path for a code Mount.
    pub paths: Vec<PathBuf>,
    /// Agent-explored description; updated over time.
    #[serde(default)]
    pub auto_description: String,
    /// Auto-detected stack / languages / structure.
    #[serde(default)]
    pub auto_meta: MountMeta,
    /// How this Mount was registered.
    pub source: MountSource,

    // ── Enrichment state (RFC-025 §Enrichment Triggers) ──
    /// Marker-file mtime at the last enrichment, for drift detection.
    /// Keys are marker file paths.
    #[serde(default)]
    pub last_marker_snapshot: HashMap<PathBuf, SystemTime>,
    /// Drift detected; the agent is nudged to refresh.
    #[serde(default)]
    pub enrichment_pending: bool,
    /// When this Mount was last enriched by the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_enriched_at: Option<DateTime<Utc>>,

    /// When this Mount was created.
    pub created_at: DateTime<Utc>,
    /// When this Mount was last modified.
    pub updated_at: DateTime<Utc>,
    /// When this Mount was last active (used in a session).
    pub last_active_at: DateTime<Utc>,
}

impl Mount {
    /// Create a new Mount with the given name and source.
    pub fn new(name: impl Into<String>, source: MountSource) -> Self {
        let now = Utc::now();
        Self {
            id: MountId::new_v4(),
            name: name.into(),
            paths: Vec::new(),
            auto_description: String::new(),
            auto_meta: MountMeta::default(),
            source,
            last_marker_snapshot: HashMap::new(),
            enrichment_pending: false,
            last_enriched_at: None,
            created_at: now,
            updated_at: now,
            last_active_at: now,
        }
    }

    /// Create a minimal Mount from a name + single path (the common
    /// "name + path only" creation flow from RFC-025).
    pub fn from_name_and_path(name: impl Into<String>, path: PathBuf) -> Self {
        let mut mount = Self::new(name, MountSource::Manual);
        mount.paths.push(path);
        mount
    }

    /// Record that this Mount was used in a session.
    pub fn touch(&mut self) {
        self.last_active_at = Utc::now();
    }

    /// Whether this Mount has any filesystem paths.
    pub fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }

    /// Get the primary path (CWD source when this Mount is primary).
    pub fn primary_path(&self) -> Option<&PathBuf> {
        self.paths.first()
    }

    /// A one-line display summary, preferring the agent-written summary then
    /// the auto-meta summary, falling back to the detected languages.
    pub fn summary_line(&self) -> String {
        if !self.auto_meta.summary.is_empty() {
            return self.auto_meta.summary.clone();
        }
        if !self.auto_description.is_empty() {
            // First non-empty line, trimmed.
            return self
                .auto_description
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim()
                .to_string();
        }
        if !self.auto_meta.languages.is_empty() {
            return self.auto_meta.languages.join(", ");
        }
        String::new()
    }

    /// Get the display tag (e.g. "[🔧 oxios]").
    pub fn tag(&self) -> String {
        format!("[🔧 {}]", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_new() {
        let m = Mount::new("oxios", MountSource::Manual);
        assert_eq!(m.name, "oxios");
        assert_eq!(m.source, MountSource::Manual);
        assert!(m.paths.is_empty());
        assert!(!m.enrichment_pending);
    }

    #[test]
    fn test_mount_from_name_and_path() {
        let m =
            Mount::from_name_and_path("oxios", PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"));
        assert_eq!(m.name, "oxios");
        assert!(m.has_paths());
        assert_eq!(
            m.primary_path(),
            Some(&PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"))
        );
    }

    #[test]
    fn test_mount_tag() {
        let m = Mount::new("oxios", MountSource::Manual);
        assert_eq!(m.tag(), "[🔧 oxios]");
    }

    #[test]
    fn test_summary_line_prefers_meta_summary() {
        let mut m = Mount::new("oxios", MountSource::Manual);
        m.auto_description = "Detailed description.\nSecond line.".to_string();
        m.auto_meta.summary = "Agent OS in Rust".to_string();
        m.auto_meta.languages = vec!["rust".to_string()];
        assert_eq!(m.summary_line(), "Agent OS in Rust");
    }

    #[test]
    fn test_summary_line_falls_back_to_description() {
        let mut m = Mount::new("oxios", MountSource::Manual);
        m.auto_description = "First line.\nSecond.".to_string();
        assert_eq!(m.summary_line(), "First line.");
    }

    #[test]
    fn test_summary_line_falls_back_to_languages() {
        let mut m = Mount::new("oxios", MountSource::Manual);
        m.auto_meta.languages = vec!["rust".to_string(), "typescript".to_string()];
        assert_eq!(m.summary_line(), "rust, typescript");
    }
}
