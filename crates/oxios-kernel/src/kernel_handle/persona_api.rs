//! Persona API — multi-persona management.
//!
//! `PersonaApi` is the public surface over `PersonaManager`. RFC-039 adds
//! `set_active_with_persist`, which also flushes the full registry to the
//! shared `StateStore`, and `manager()` for callers that need the underlying
//! `Arc<PersonaManager>` (e.g. boot-time `load_from_state_store` /
//! `apply_config`).

use crate::persona::Persona;
use crate::persona::PersonaManager;
use crate::state_store::StateStore;
use std::sync::Arc;

/// Persona management system calls.
pub struct PersonaApi {
    pub(crate) persona_manager: Arc<PersonaManager>,
}

impl PersonaApi {
    /// Create a new PersonaApi.
    pub fn new(persona_manager: Arc<PersonaManager>) -> Self {
        Self { persona_manager }
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

    /// Create a new persona (in-memory only; persistence requires an explicit
    /// `set_active_with_persist` or `manager.persist`).
    pub fn create(&self, persona: Persona) {
        self.persona_manager.store().register(persona);
    }

    /// Update a persona.
    pub fn update(&self, id: &str, persona: Persona) -> anyhow::Result<()> {
        self.persona_manager.store().update(id, persona)
    }

    /// Delete a persona.
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

    /// RFC-039: set active persona + persist the full registry to `StateStore`.
    /// Returns the new `system_prompt` so the caller can re-seed the intent
    /// engine (kernel ↔ ouroboros 의존성 방향 회피).
    pub async fn set_active_with_persist(
        &self,
        id: &str,
        store: &crate::state_store::StateStore,
    ) -> anyhow::Result<Option<String>> {
        self.persona_manager.set_active(id, Some(store)).await
    }

    /// List enabled personas.
    pub fn list_enabled(&self) -> Vec<Persona> {
        self.persona_manager.store().list_enabled()
    }
}
