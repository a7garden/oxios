//! Persona API — multi-persona management.

use crate::persona::Persona;
use crate::persona_manager::PersonaManager;
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
    /// List all personas.
    pub fn list(&self) -> Vec<Persona> {
        self.persona_manager.store().list_all()
    }

    /// Get persona by ID.
    pub fn get(&self, id: &str) -> Option<Persona> {
        self.persona_manager.store().get(id)
    }

    /// Create a new persona.
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

    /// Set active persona.
    pub fn set_active(&self, id: &str) -> anyhow::Result<()> {
        self.persona_manager.set_active_persona(id)
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
