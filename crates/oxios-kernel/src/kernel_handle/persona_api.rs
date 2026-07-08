//! Persona API — multi-persona management.
//!
//! `PersonaApi` is the public surface over `PersonaManager`. RFC-039 adds
//! `set_active_with_persist` + `persist`, which also flushes the full
//! registry via the shared `StateApi`, and `manager()` for callers that
//! need the underlying `Arc<PersonaManager>` (e.g. boot-time
//! `load_from_state_store` / `apply_config`).

use crate::persona::Persona;
use crate::persona::PersonaManager;
use std::sync::Arc;

/// Persona management system calls.
pub struct PersonaApi {
    pub(crate) persona_manager: Arc<PersonaManager>,
    /// RFC-039: optional callback invoked after a successful activate/persist
    /// to re-seed the intent engine's system_prompt.
    reseed_callback: Option<Arc<dyn Fn(Option<String>) + Send + Sync>>,
}

impl PersonaApi {
    /// Create a new PersonaApi.
    pub fn new(persona_manager: Arc<PersonaManager>) -> Self {
        Self {
            persona_manager,
            reseed_callback: None,
        }
    }

    /// Set the callback that re-seeds the intent engine's system_prompt.
    /// Called automatically by `set_active_with_persist`.
    pub fn set_reseed_callback(&mut self, cb: Option<Arc<dyn Fn(Option<String>) + Send + Sync>>) {
        self.reseed_callback = cb;
    }

    /// Underlying `Arc<PersonaManager>` for boot-time wiring.
    pub fn manager(&self) -> Arc<PersonaManager> {
        Arc::clone(&self.persona_manager)
    }

    /// List all personas.
    pub fn list(&self) -> Vec<Persona> {
        self.persona_manager.store().list_all()
    }

    /// Get persona by ID.
    pub fn get(&self, id: &str) -> Option<Persona> {
        self.persona_manager.store().get(id)
    }

    /// Create a new persona (in-memory only; persistence requires an
    /// explicit `persist` call after this — see `handle_persona_create`).
    pub fn create(&self, persona: Persona) {
        self.persona_manager.store().register(persona);
    }

    /// Update a persona (in-memory only; call `persist` after).
    pub fn update(&self, id: &str, persona: Persona) -> anyhow::Result<()> {
        self.persona_manager.store().update(id, persona)
    }

    /// Delete a persona (in-memory only; call `persist` after).
    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.persona_manager.store().delete(id)
    }

    /// Get active persona.
    pub fn active(&self) -> Option<Persona> {
        self.persona_manager.get_active_persona()
    }

    /// Get active persona ID.
    pub fn active_id(&self) -> Option<String> {
        self.persona_manager.active_persona_id()
    }

    /// Legacy in-memory `set_active` (no persistence).
    /// Prefer `set_active_with_persist` for new callers.
    pub fn set_active(&self, id: &str) -> anyhow::Result<()> {
        self.persona_manager.set_active_persona(id)
    }

    /// RFC-039: set active persona + persist the full registry via `StateApi`.
    /// Returns the new system_prompt so the caller can re-seed the intent
    /// engine (kernel <-> ouroboros dependency direction avoidance).
    pub async fn set_active_with_persist(
        &self,
        id: &str,
        state_api: &crate::kernel_handle::StateApi,
    ) -> anyhow::Result<Option<String>> {
        self.persona_manager.set_active(id, None).await?;
        self.persist(state_api).await?;
        let prompt = self
            .persona_manager
            .get_active_persona()
            .map(|p| p.system_prompt);
        // RFC-039: auto re-seed intent engine if callback is set.
        if let Some(ref cb) = self.reseed_callback {
            cb(prompt.clone());
        }
        Ok(prompt)
    }

    /// RFC-039: persist the full persona registry to `StateStore`.
    /// Call this after every mutation (`create`/`update`/`delete`) so the
    /// in-memory state matches the on-disk state. Otherwise the next
    /// restart will lose the change.
    pub async fn persist(&self, state_api: &crate::kernel_handle::StateApi) -> anyhow::Result<()> {
        let snapshot = crate::persona::persistence::PersonaSnapshot {
            schema_version: 1,
            active_persona_id: self.persona_manager.active_persona_id(),
            personas: self.persona_manager.store().list_all(),
        };
        state_api.save("personas", "index", &snapshot).await
    }

    /// Get persona count.
    pub fn count(&self) -> usize {
        self.persona_manager.store().len()
    }

    /// List enabled personas.
    pub fn list_enabled(&self) -> Vec<Persona> {
        self.persona_manager.store().list_enabled()
    }
}
