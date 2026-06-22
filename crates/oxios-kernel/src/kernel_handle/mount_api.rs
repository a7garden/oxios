//! Mount API — Mount management system calls (RFC-025).
//!
//! Provides API endpoints for:
//! - Listing and querying Mounts
//! - CRUD on Mounts (minimal "name + path" input)
//! - Agent-driven enrichment (`update_enrichment`)
//! - Detection

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mount::{Mount, MountId, MountManager, MountMeta, MountSource};

/// Serialized Mount info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct MountInfo {
    pub id: String,
    pub name: String,
    pub paths: Vec<String>,
    pub auto_description: String,
    pub auto_meta: MountMeta,
    pub source: String,
    pub enrichment_pending: bool,
    pub last_enriched_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_active_at: String,
}

impl From<&Mount> for MountInfo {
    fn from(m: &Mount) -> Self {
        Self {
            id: m.id.to_string(),
            name: m.name.clone(),
            paths: m
                .paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            auto_description: m.auto_description.clone(),
            auto_meta: m.auto_meta.clone(),
            source: m.source.to_string(),
            enrichment_pending: m.enrichment_pending,
            last_enriched_at: m.last_enriched_at.map(|t| t.to_rfc3339()),
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
            last_active_at: m.last_active_at.to_rfc3339(),
        }
    }
}

/// Mount system calls.
#[allow(dead_code)]
pub struct MountApi {
    /// Mount manager.
    pub(crate) mount_manager: Arc<MountManager>,
}

impl MountApi {
    /// Create a new MountApi.
    pub fn new(mount_manager: Arc<MountManager>) -> Self {
        Self { mount_manager }
    }

    /// List all Mounts.
    pub fn list_mounts(&self) -> Vec<MountInfo> {
        self.mount_manager
            .list_mounts()
            .iter()
            .map(MountInfo::from)
            .collect()
    }

    /// Get a Mount by ID.
    pub fn get_mount(&self, id: &str) -> Option<MountInfo> {
        let mount_id: MountId = Uuid::parse_str(id).ok()?;
        self.mount_manager
            .get_mount(mount_id)
            .as_ref()
            .map(MountInfo::from)
    }

    /// Get a Mount by name.
    pub fn get_mount_by_name(&self, name: &str) -> Option<MountInfo> {
        self.mount_manager
            .get_mount_by_name(name)
            .as_ref()
            .map(MountInfo::from)
    }

    /// Resolve a comma-separated list of Mount IDs to Mount records,
    /// preserving order. Used by the orchestrator/runtime.
    pub fn resolve_mounts(&self, mount_ids: &[MountId]) -> Vec<Mount> {
        self.mount_manager.get_mounts_ordered(mount_ids)
    }

    /// Create a new Mount (minimal RFC-025 input: name + paths).
    pub fn create_mount(&self, name: String, paths: Vec<String>) -> Result<MountInfo> {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let mount = self
            .mount_manager
            .create_mount(name, paths, MountSource::Manual)?;
        Ok(MountInfo::from(&mount))
    }

    /// Update a Mount's agent-enriched fields (RFC-025 Phase 3).
    pub fn update_enrichment(
        &self,
        id: &str,
        auto_description: Option<String>,
        auto_meta: Option<MountMeta>,
    ) -> Result<MountInfo> {
        let mount_id: MountId = Uuid::parse_str(id).context("Invalid mount ID")?;
        let mount = self
            .mount_manager
            .update_enrichment(mount_id, auto_description, auto_meta)?;
        Ok(MountInfo::from(&mount))
    }

    /// Rename a Mount.
    pub fn rename_mount(&self, id: &str, new_name: String) -> Result<MountInfo> {
        let mount_id: MountId = Uuid::parse_str(id).context("Invalid mount ID")?;
        let mount = self.mount_manager.rename(mount_id, new_name)?;
        Ok(MountInfo::from(&mount))
    }

    /// Remove a Mount.
    pub fn remove_mount(&self, id: &str) -> Result<()> {
        let mount_id: MountId = Uuid::parse_str(id).context("Invalid mount ID")?;
        self.mount_manager.remove_mount(mount_id)
    }

    /// Record that a Mount was used (touch activity timestamp).
    ///
    /// Returns `Err` on an invalid mount ID — consistent with the other
    /// mutation methods in this facade (update_enrichment, rename_mount,
    /// remove_mount) which previously were the only ones that surfaced the
    /// parse error.
    pub fn touch_mount(&self, id: &str) -> Result<()> {
        let mount_id: MountId = Uuid::parse_str(id).context("Invalid mount ID")?;
        self.mount_manager.touch(mount_id);
        Ok(())
    }

    /// Re-seed auto_meta from the filesystem (RFC-025 manual rescan).
    pub fn rescan(&self, id: &str) -> Result<MountInfo> {
        let mount_id: MountId = Uuid::parse_str(id).context("Invalid mount ID")?;
        self.mount_manager
            .seed_auto_meta(mount_id)
            .context("Failed to rescan mount")?;
        self.get_mount(id)
            .ok_or_else(|| anyhow::anyhow!("Mount not found after rescan"))
    }

    /// Detect a Mount from a message, returning the matched Mount's info.
    pub fn detect(&self, message: &str) -> Option<MountInfo> {
        use crate::mount::DetectionResult;
        match self.mount_manager.detect(message) {
            DetectionResult::Found(id) => self
                .mount_manager
                .get_mount(id)
                .as_ref()
                .map(MountInfo::from),
            DetectionResult::NoMatch { .. } => None,
        }
    }
}
