//! StateStore-backed AuditPersistence for oxi-sdk's AuditTrail.
//!
//! Bridges the `oxi_sdk::observability::AuditPersistence` trait to oxios's
//! filesystem-based `StateStore`. The trail JSON is written to
//! `<base_path>/audit/trail.json`, matching the legacy layout used before
//! the SDK migration (RFC-014 Phase F).
//!
//! See: <https://github.com/a7garden/oxios/blob/main/docs/rfc-014/phase-f-audit-trail.md>

use anyhow::Result;
use oxi_sdk::observability::{AuditPersistence, TrailEntry};

use crate::state_store::StateStore;

impl AuditPersistence for StateStore {
    fn save(&self, entries: &[TrailEntry]) -> Result<()> {
        let path = self.audit_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entries)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    fn load(&self) -> Result<Vec<TrailEntry>> {
        let path = self.audit_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let json = std::fs::read_to_string(&path)?;
        let entries: Vec<TrailEntry> = serde_json::from_str(&json)?;
        Ok(entries)
    }
}

impl StateStore {
    /// Path to the persisted audit trail file.
    ///
    /// Layout: `<base_path>/audit/trail.json`
    fn audit_path(&self) -> std::path::PathBuf {
        self.base_path.join("audit").join("trail.json")
    }
}
