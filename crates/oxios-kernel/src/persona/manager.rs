//! Persona manager: coordinates persona-aware execution.
//!
//! The PersonaManager manages persona lifecycle and provides
//! the active persona for orchestrator and agent runtime.

use anyhow::Result;
use parking_lot::RwLock;

use super::store::PersonaStore;
use super::{Persona, default_personas};

/// Manages persona lifecycle and coordinates persona-aware execution.
#[derive(Debug)]
pub struct PersonaManager {
    store: PersonaStore,
    active_persona_id: RwLock<Option<String>>,
}

impl PersonaManager {
    /// Creates a new persona manager with default personas.
    pub fn new() -> Self {
        let store = PersonaStore::new();
        let manager = Self {
            store,
            active_persona_id: RwLock::new(None),
        };
        manager.create_default_personas();
        manager
    }

    /// Creates a new persona manager, optionally loading from existing data.
    pub fn with_defaults(personas: Vec<Persona>) -> Self {
        let store = PersonaStore::new();
        store.load_from_slice(&personas);
        let this = Self {
            store,
            active_persona_id: RwLock::new(None),
        };
        // Set the first enabled persona as active by default.
        if let Some(first) = this.store.list_enabled().into_iter().next() {
            *this.active_persona_id.write() = Some(first.id);
        }
        this
    }

    /// Returns the current active persona, if any.
    pub fn get_active_persona(&self) -> Option<Persona> {
        let active_id = self.active_persona_id.read().clone();
        active_id.and_then(|id| self.store.get(&id))
    }

    /// Sets the active persona by ID.
    pub fn set_active_persona(&self, id: &str) -> Result<()> {
        // Verify the persona exists and is enabled.
        let persona = self
            .store
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Persona '{id}' not found"))?;
        if !persona.enabled {
            anyhow::bail!("Persona '{id}' is disabled");
        }
        *self.active_persona_id.write() = Some(id.to_string());
        tracing::info!(persona_id = %id, name = %persona.name, "Active persona set");
        Ok(())
    }

    /// Returns the system prompt for the active persona.
    /// Falls back to a default prompt if no active persona.
    pub fn active_system_prompt(&self) -> String {
        self.get_active_persona()
            .map(|p| p.system_prompt.clone())
            .unwrap_or_else(|| {
                "You are a helpful AI assistant that follows the Ouroboros methodology: \
                 specify before you build, evaluate before you ship."
                    .to_string()
            })
    }

    /// Creates the three default personas (Dev, Review, Research).
    pub fn create_default_personas(&self) {
        let defaults = default_personas();
        for persona in defaults {
            // Only register if not already present.
            if self.store.get(&persona.id).is_none() {
                self.store.register(persona);
            }
        }
        // Set first persona as active if none is set.
        {
            let mut active = self.active_persona_id.write();
            if active.is_none() {
                *active = Some("dev".to_string());
            }
        }
        tracing::info!("Default personas initialized");
    }

    /// Returns the first enabled persona, for wiring into OuroborosEngine.
    pub fn first_enabled(&self) -> Option<Persona> {
        self.store.list_enabled().into_iter().next()
    }

    /// Returns the persona store for direct access.
    pub fn store(&self) -> &PersonaStore {
        &self.store
    }

    /// Returns the ID of the active persona.
    pub fn active_persona_id(&self) -> Option<String> {
        self.active_persona_id.read().clone()
    }
}

impl Default for PersonaManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PersonaManager {
    fn clone(&self) -> Self {
        let personas: Vec<Persona> = self.store.list_all();
        let store = PersonaStore::new();
        store.load_from_slice(&personas);
        Self {
            store,
            active_persona_id: RwLock::new(self.active_persona_id.read().clone()),
        }
    }
}
