//! Persona persistence: load/save the full registry + active_persona_id to
//! `StateStore` (RFC-039).
//!
//! File path: `~/.oxios/state/personas/index.json`
//!
//! Schema (schema_version = 1):
//! ```json
//! {
//!   "schema_version": 1,
//!   "active_persona_id": "dev",
//!   "personas": [ { "id": "...", "name": "...", "role": "...",
//!                   "description": "...", "system_prompt": "...",
//!                   "enabled": true, "model": null,
//!                   "personality_traits": [] }, ... ]
//! }
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::Persona;
use crate::state_store::StateStore;

const SCHEMA_VERSION: u32 = 1;

/// Serializable snapshot of the persona registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSnapshot {
    pub schema_version: u32,
    /// Active persona ID. `None` if no persona is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_persona_id: Option<String>,
    /// All personas (enabled and disabled). Empty Vec is allowed.
    pub personas: Vec<Persona>,
}

/// Load the snapshot from `StateStore`. `Ok(None)` if file is absent.
pub async fn load_from_state_store(store: &StateStore) -> Result<Option<PersonaSnapshot>> {
    let raw: Option<PersonaSnapshot> = store
        .load_json("personas", "index")
        .await
        .context("persona: failed to load index.json")?;
    let Some(snap) = raw else { return Ok(None) };
    if snap.schema_version != SCHEMA_VERSION {
        anyhow::bail!(
            "persona: schema_version {} not supported (expected {})",
            snap.schema_version,
            SCHEMA_VERSION
        );
    }
    Ok(Some(snap))
}

/// Save the snapshot to `StateStore`. Atomic via `StateStore::durable_write`.
pub async fn save_to_state_store(
    store: &StateStore,
    snapshot: &PersonaSnapshot,
) -> Result<()> {
    store
        .save_json("personas", "index", snapshot)
        .await
        .context("persona: failed to save index.json")
}
