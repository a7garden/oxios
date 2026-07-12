//! Persona API — multi-persona management.
//!
//! `PersonaApi` is the thin public surface over `PersonaManager`.
//! RFC-039 completion: the manager owns persistence (`StateStore`) and
//! intent-engine re-seeding (callback) internally, so this API is a
//! pass-through — no caller can accidentally skip persist or re-seed.

use crate::persona::Persona;
use crate::persona::PersonaManager;
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

    /// Create a new persona (in-memory only; call `persist` after).
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

    /// Set active persona — persists to disk and re-seeds the intent engine
    /// automatically (via `PersonaManager::set_active`).
    pub async fn set_active(&self, id: &str) -> anyhow::Result<Option<String>> {
        self.persona_manager.set_active(id).await
    }

    /// Persist the full persona registry to `StateStore`.
    /// Call this after `create`/`update`/`delete` mutations so the in-memory
    /// state matches the on-disk state.
    pub async fn persist(&self) -> anyhow::Result<()> {
        self.persona_manager.persist().await
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
