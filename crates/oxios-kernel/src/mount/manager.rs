//! MountManager: CRUD + detection for Mounts (RFC-025).
//!
//! Mirrors `ProjectManager`'s structure for consistency. Mounts are persisted
//! in the `mounts` SQLite table (same `memory.db`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use chrono::Utc;
use parking_lot::RwLock;

use oxios_memory::memory::sqlite::MemoryDatabase;

use super::mount_db;
use super::{DetectionResult, Mount, MountId, MountMeta, MountSource, detect_mounts};
use crate::event_bus::{EventBus, KernelEvent};

/// Errors from MountManager operations.
#[derive(thiserror::Error, Debug)]
pub enum MountManagerError {
    /// Mount not found.
    #[error("Mount not found: {0}")]
    NotFound(MountId),
    /// Mount name already taken.
    #[error("Mount name already exists: {0}")]
    DuplicateName(String),
    /// Invalid operation.
    #[error("Invalid operation: {0}")]
    Invalid(String),
}

/// Manages Mounts: CRUD, lookup, and detection.
///
/// Mounts are persisted in the `mounts` SQLite table
/// (same `memory.db` as memories and the legacy `projects` table).
pub struct MountManager {
    /// In-memory index of all Mounts (loaded at startup).
    mounts: RwLock<HashMap<MountId, Mount>>,
    /// Name → ID index for fast name lookup.
    name_index: RwLock<HashMap<String, MountId>>,
    /// SQLite database for persistence.
    db: Arc<MemoryDatabase>,
    /// Event bus for publishing Mount events.
    event_bus: Option<EventBus>,
}

impl MountManager {
    /// Create a new MountManager, loading existing Mounts from SQLite.
    pub fn new(db: Arc<MemoryDatabase>, event_bus: Option<EventBus>) -> Result<Self> {
        // Ensure the schema exists (idempotent).
        mount_db::ensure_mount_schema(&db.conn())?;

        let mut mounts = HashMap::new();
        let mut name_index = HashMap::new();
        for mount in mount_db::list_mounts(&db.conn())? {
            name_index.insert(mount.name.clone(), mount.id);
            mounts.insert(mount.id, mount);
        }

        tracing::info!(count = mounts.len(), "MountManager initialized");

        Ok(Self {
            mounts: RwLock::new(mounts),
            name_index: RwLock::new(name_index),
            db,
            event_bus,
        })
    }

    /// List all Mounts.
    pub fn list_mounts(&self) -> Vec<Mount> {
        self.mounts.read().values().cloned().collect()
    }

    /// Get a Mount by ID.
    pub fn get_mount(&self, id: MountId) -> Option<Mount> {
        self.mounts.read().get(&id).cloned()
    }

    /// Get a Mount by name.
    pub fn get_mount_by_name(&self, name: &str) -> Option<Mount> {
        let name_index = self.name_index.read();
        let id = name_index.get(name)?;
        self.mounts.read().get(id).cloned()
    }

    /// Get several Mounts by ID, preserving the request order. Missing IDs
    /// are skipped (they may have been deleted).
    pub fn get_mounts_ordered(&self, ids: &[MountId]) -> Vec<Mount> {
        let mounts = self.mounts.read();
        ids.iter()
            .filter_map(|id| mounts.get(id).cloned())
            .collect()
    }

    /// Create a new Mount with the minimal RFC-025 input (name + paths).
    pub fn create_mount(
        &self,
        name: String,
        paths: Vec<PathBuf>,
        source: MountSource,
    ) -> Result<Mount> {
        {
            let name_index = self.name_index.read();
            if name_index.contains_key(&name) {
                return Err(MountManagerError::DuplicateName(name).into());
            }
        }
        if paths.is_empty() {
            return Err(MountManagerError::Invalid(
                "a Mount requires at least one path".to_string(),
            )
            .into());
        }

        let mut mount = Mount::new(&name, source);
        mount.paths = paths;

        mount_db::save_mount(&self.db.conn(), &mount)?;

        {
            let mut mounts = self.mounts.write();
            let mut name_index = self.name_index.write();
            name_index.insert(mount.name.clone(), mount.id);
            mounts.insert(mount.id, mount.clone());
        }

        if let Some(ref event_bus) = self.event_bus {
            let _ = event_bus.publish(KernelEvent::ProjectCreated {
                // Reuse ProjectCreated for now; a MountCreated variant can be
                // added when the frontend needs to distinguish them.
                project_id: mount.id,
                name: mount.name.clone(),
                source: source.to_string(),
            });
        }

        tracing::info!(name = %mount.name, id = %mount.id, "Mount created");
        Ok(mount)
    }

    /// Update a Mount's auto-enriched fields (agent-driven, RFC-025 Phase 3).
    ///
    /// Only `auto_description` and `auto_meta` are writable here — `name` and
    /// `paths` are user-level and go through [`Self::rename`] / the web API.
    pub fn update_enrichment(
        &self,
        id: MountId,
        auto_description: Option<String>,
        auto_meta: Option<MountMeta>,
    ) -> Result<Mount> {
        let mut mounts = self.mounts.write();
        let mount = mounts.get_mut(&id).ok_or(MountManagerError::NotFound(id))?;

        if let Some(desc) = auto_description {
            // Bounded per RFC-025 cost guard (≤ 500 chars).
            mount.auto_description = desc.chars().take(500).collect();
        }
        if let Some(meta) = auto_meta {
            mount.auto_meta = meta;
        }
        mount.last_enriched_at = Some(Utc::now());
        mount.enrichment_pending = false;
        mount.updated_at = Utc::now();

        let mount_clone = mount.clone();
        drop(mounts);
        mount_db::save_mount(&self.db.conn(), &mount_clone)?;
        tracing::info!(name = %mount_clone.name, id = %id, "Mount enriched");
        Ok(mount_clone)
    }

    /// Rename a Mount.
    pub fn rename(&self, id: MountId, new_name: String) -> Result<Mount> {
        let mut mounts = self.mounts.write();
        let mut name_index = self.name_index.write();
        let mount = mounts.get_mut(&id).ok_or(MountManagerError::NotFound(id))?;

        if new_name != mount.name {
            if name_index.contains_key(&new_name) {
                return Err(MountManagerError::DuplicateName(new_name).into());
            }
            name_index.remove(&mount.name);
            name_index.insert(new_name.clone(), id);
            mount.name = new_name;
            mount.updated_at = Utc::now();
        }

        let mount_clone = mount.clone();
        drop(mounts);
        drop(name_index);
        mount_db::save_mount(&self.db.conn(), &mount_clone)?;
        Ok(mount_clone)
    }

    /// Remove a Mount.
    pub fn remove_mount(&self, id: MountId) -> Result<()> {
        {
            let mut mounts = self.mounts.write();
            let mut name_index = self.name_index.write();
            let mount = mounts.remove(&id).ok_or(MountManagerError::NotFound(id))?;
            name_index.remove(&mount.name);
        }
        mount_db::delete_mount(&self.db.conn(), &id.to_string())?;
        tracing::info!(id = %id, "Mount removed");
        Ok(())
    }

    /// Record that a Mount was used in a session.
    pub fn touch(&self, id: MountId) {
        let to_save = {
            let mut mounts = self.mounts.write();
            if let Some(mount) = mounts.get_mut(&id) {
                mount.touch();
                Some(mount.clone())
            } else {
                None
            }
        };
        if let Some(mount) = to_save {
            let _ = mount_db::save_mount(&self.db.conn(), &mount);
        }
    }

    /// Try to detect a Mount from a user message.
    pub fn detect(&self, message: &str) -> DetectionResult {
        let mounts = self.list_mounts();
        detect_mounts(message, &mounts)
    }

    /// Seed `auto_meta` from the filesystem (RFC-025 §Auto-Meta).
    ///
    /// Cheap heuristic detection on marker files. The agent refines this
    /// during enrichment. Idempotent — safe to call multiple times.
    pub fn seed_auto_meta(&self, id: MountId) -> Result<()> {
        let mount = {
            let mounts = self.mounts.read();
            mounts
                .get(&id)
                .ok_or(MountManagerError::NotFound(id))?
                .clone()
        };
        let Some(primary) = mount.primary_path() else {
            return Ok(()); // nothing to scan
        };
        if !primary.exists() {
            tracing::debug!(path = %primary.display(), "Mount path missing, skip meta seed");
            return Ok(());
        }
        let meta = super::meta_detection::detect_meta(primary);
        self.update_enrichment(id, None, Some(meta))?;
        Ok(())
    }

    /// Check marker-file drift and set `enrichment_pending` (RFC-025 §Enrichment).
    ///
    /// Compares current marker mtimes against the stored snapshot. Returns
    /// `true` if any marker drifted (and the flag was set). Cheap: a handful
    /// of `stat()` calls.
    pub fn check_drift(&self, id: MountId) -> Result<bool> {
        let mut mounts = self.mounts.write();
        let mount = mounts.get_mut(&id).ok_or(MountManagerError::NotFound(id))?;
        let Some(primary) = mount.primary_path().cloned() else {
            return Ok(false);
        };
        let current = super::meta_detection::snapshot_markers(&primary);
        let drifted = markers_drifted(&mount.last_marker_snapshot, &current);
        if drifted {
            mount.enrichment_pending = true;
            mount.updated_at = Utc::now();
        }
        // Always refresh the snapshot so the next comparison is accurate.
        mount.last_marker_snapshot = current.into_iter().collect();
        let mount_clone = mount.clone();
        drop(mounts);
        let _ = mount_db::save_mount(&self.db.conn(), &mount_clone);
        Ok(drifted)
    }

    /// Check drift for all Mounts (Dream-time refresh, RFC-025).
    ///
    /// Returns the IDs of Mounts whose content drifted.
    pub fn check_all_drift(&self) -> Vec<MountId> {
        let ids: Vec<MountId> = self.mounts.read().keys().copied().collect();
        ids.into_iter()
            .filter(|id| self.check_drift(*id).unwrap_or(false))
            .collect()
    }
}

/// Compare a stored marker snapshot against the current state.
/// Returns `true` if any marker was added, removed, or changed mtime.
fn markers_drifted(
    stored: &HashMap<PathBuf, SystemTime>,
    current: &[(std::path::PathBuf, SystemTime)],
) -> bool {
    if stored.len() != current.len() {
        return true; // marker added or removed
    }
    for (path, mtime) in current {
        match stored.get(path) {
            Some(stored_time) if stored_time == mtime => continue,
            _ => return true, // new, removed, or changed
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_manager() -> MountManager {
        let db = Arc::new(MemoryDatabase::open_in_memory(64).expect("db"));
        MountManager::new(db, None).expect("manager")
    }

    #[test]
    fn test_create_and_get() {
        let mgr = open_manager();
        let m = mgr
            .create_mount(
                "oxios".to_string(),
                vec![PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios")],
                MountSource::Manual,
            )
            .expect("create");
        assert_eq!(mgr.get_mount(m.id).unwrap().name, "oxios");
        assert_eq!(mgr.get_mount_by_name("oxios").unwrap().id, m.id);
    }

    #[test]
    fn test_duplicate_name_rejected() {
        let mgr = open_manager();
        mgr.create_mount(
            "oxios".to_string(),
            vec![PathBuf::from("/a")],
            MountSource::Manual,
        )
        .expect("first");
        let err = mgr
            .create_mount(
                "oxios".to_string(),
                vec![PathBuf::from("/b")],
                MountSource::Manual,
            )
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_empty_paths_rejected() {
        let mgr = open_manager();
        let err = mgr
            .create_mount("x".to_string(), vec![], MountSource::Manual)
            .unwrap_err();
        assert!(err.to_string().contains("at least one path"));
    }

    #[test]
    fn test_update_enrichment_bounds_description() {
        let mgr = open_manager();
        let m = mgr
            .create_mount(
                "oxios".to_string(),
                vec![PathBuf::from("/a")],
                MountSource::Manual,
            )
            .expect("create");
        let long = "x".repeat(800);
        let updated = mgr
            .update_enrichment(m.id, Some(long.clone()), None)
            .expect("update");
        assert_eq!(updated.auto_description.chars().count(), 500);
        assert!(updated.last_enriched_at.is_some());
        assert!(!updated.enrichment_pending);
    }

    #[test]
    fn test_remove_mount() {
        let mgr = open_manager();
        let m = mgr
            .create_mount(
                "temp".to_string(),
                vec![PathBuf::from("/t")],
                MountSource::Manual,
            )
            .expect("create");
        mgr.remove_mount(m.id).expect("remove");
        assert!(mgr.get_mount(m.id).is_none());
        assert!(mgr.get_mount_by_name("temp").is_none());
    }

    #[test]
    fn test_get_mounts_ordered_skips_missing() {
        let mgr = open_manager();
        let m1 = mgr
            .create_mount(
                "a".to_string(),
                vec![PathBuf::from("/a")],
                MountSource::Manual,
            )
            .unwrap();
        let m2 = mgr
            .create_mount(
                "b".to_string(),
                vec![PathBuf::from("/b")],
                MountSource::Manual,
            )
            .unwrap();
        let missing = MountId::new_v4();
        let got = mgr.get_mounts_ordered(&[m1.id, missing, m2.id]);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name, "a");
        assert_eq!(got[1].name, "b");
    }
}
