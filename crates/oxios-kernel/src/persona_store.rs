//! In-memory store for persona registry.
//!
//! PersonaStore manages CRUD operations for personas in memory.
//! Persists personas to the state store on save.

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::persona::Persona;

/// Thread-safe in-memory persona registry.
#[derive(Debug, Default)]
pub struct PersonaStore {
    personas: RwLock<HashMap<String, Persona>>,
}

impl PersonaStore {
    /// Creates a new empty persona store.
    pub fn new() -> Self {
        Self {
            personas: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a new persona, replacing any existing one with the same ID.
    pub fn register(&self, persona: Persona) {
        let mut personas = self.personas.write();
        personas.insert(persona.id.clone(), persona);
    }

    /// Gets a persona by ID.
    pub fn get(&self, id: &str) -> Option<Persona> {
        let personas = self.personas.read();
        personas.get(id).cloned()
    }

    /// Returns all enabled personas.
    pub fn list_enabled(&self) -> Vec<Persona> {
        let personas = self.personas.read();
        personas.values().filter(|p| p.enabled).cloned().collect()
    }

    /// Returns all personas (enabled and disabled).
    pub fn list_all(&self) -> Vec<Persona> {
        let personas = self.personas.read();
        personas.values().cloned().collect()
    }

    /// Sets the enabled state of a persona.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let mut personas = self.personas.write();
        match personas.get_mut(id) {
            Some(p) => {
                p.enabled = enabled;
                Ok(())
            }
            None => Err(anyhow!("Persona '{id}' not found")),
        }
    }

    /// Deletes a persona by ID.
    pub fn delete(&self, id: &str) -> Result<()> {
        let mut personas = self.personas.write();
        if personas.remove(id).is_some() {
            Ok(())
        } else {
            Err(anyhow!("Persona '{id}' not found"))
        }
    }

    /// Updates an existing persona.
    pub fn update(&self, id: &str, updated: Persona) -> Result<()> {
        let mut personas = self.personas.write();
        if personas.contains_key(id) {
            personas.insert(id.to_string(), updated);
            Ok(())
        } else {
            Err(anyhow!("Persona '{id}' not found"))
        }
    }

    /// Returns the count of registered personas.
    pub fn len(&self) -> usize {
        let personas = self.personas.read();
        personas.len()
    }

    /// Returns true if there are no registered personas.
    pub fn is_empty(&self) -> bool {
        let personas = self.personas.read();
        personas.is_empty()
    }

    /// Loads personas from a serializable slice.
    pub fn load_from_slice(&self, personas: &[Persona]) {
        let mut store = self.personas.write();
        for p in personas {
            store.insert(p.id.clone(), p.clone());
        }
    }
}

/// A handle to the persona store for sharing across components.
#[derive(Clone)]
pub struct PersonaStoreHandle {
    store: Arc<PersonaStore>,
}

impl PersonaStoreHandle {
    /// Creates a new handle wrapping the given store.
    pub fn from_store(store: Arc<PersonaStore>) -> Self {
        Self { store }
    }

    /// Returns a clone of the inner store.
    pub fn inner(&self) -> Arc<PersonaStore> {
        Arc::clone(&self.store)
    }
}

impl Default for PersonaStoreHandle {
    fn default() -> Self {
        Self {
            store: Arc::new(PersonaStore::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let store = PersonaStore::new();
        let persona = Persona::new("Test", "assistant", "A test persona", "You are a test.");
        store.register(persona.clone());
        assert_eq!(store.get(&persona.id).unwrap().name, "Test");
    }

    #[test]
    fn test_list_enabled() {
        let store = PersonaStore::new();
        let mut p1 = Persona::new("A", "dev", "Desc A", "Prompt A");
        p1.enabled = true;
        let mut p2 = Persona::new("B", "dev", "Desc B", "Prompt B");
        p2.enabled = false;
        store.register(p1);
        store.register(p2);

        let enabled = store.list_enabled();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "A");
    }

    #[test]
    fn test_set_enabled() {
        let store = PersonaStore::new();
        let persona = Persona::new("Test", "dev", "Desc", "Prompt");
        store.register(persona.clone());
        store.set_enabled(&persona.id, false).unwrap();
        assert!(store.list_enabled().is_empty());

        store.set_enabled(&persona.id, true).unwrap();
        assert_eq!(store.list_enabled().len(), 1);
    }

    #[test]
    fn test_delete() {
        let store = PersonaStore::new();
        let persona = Persona::new("Test", "dev", "Desc", "Prompt");
        let id = persona.id.clone();
        store.register(persona);
        store.delete(&id).unwrap();
        assert!(store.get(&id).is_none());
    }
}
