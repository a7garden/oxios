//! Test support utilities for the memory subsystem.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use crate::memory::storage::MemoryStorage;

/// In-memory `MemoryStorage` implementation for tests.
#[derive(Default)]
pub struct InMemoryStorage {
    data: Mutex<HashMap<(String, String), Value>>,
}

#[async_trait]
impl MemoryStorage for InMemoryStorage {
    async fn save_json_value(
        &self,
        category: &str,
        key: &str,
        value: &Value,
    ) -> anyhow::Result<()> {
        self.data
            .lock()
            .unwrap()
            .insert((category.to_string(), key.to_string()), value.clone());
        Ok(())
    }

    async fn load_json_value(&self, category: &str, key: &str) -> anyhow::Result<Option<Value>> {
        Ok(self
            .data
            .lock()
            .unwrap()
            .get(&(category.to_string(), key.to_string()))
            .cloned())
    }

    async fn list_category(&self, category: &str) -> anyhow::Result<Vec<String>> {
        let data = self.data.lock().unwrap();
        let mut keys: Vec<String> = data
            .keys()
            .filter(|(cat, _)| cat == category)
            .map(|(_, key)| key.clone())
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn delete_file(&self, category: &str, key: &str) -> anyhow::Result<bool> {
        Ok(self
            .data
            .lock()
            .unwrap()
            .remove(&(category.to_string(), key.to_string()))
            .is_some())
    }
}
