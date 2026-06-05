//! Extension trait providing typed helpers for [`MemoryStorage`].
//!
//! The base [`MemoryStorage`] trait only takes/returns `serde_json::Value`
//! (to stay object-safe). This module provides a blanket-implemented
//! extension trait that adds typed helpers, so callers can do
//! `storage.save_typed(&entry).await?` without manual JSON conversion.

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::memory::storage::MemoryStorage;

/// Extension methods for [`MemoryStorage`] providing typed access.
#[allow(async_fn_in_trait)]
pub trait MemoryStorageExt {
    /// Save a typed value to storage.
    async fn save_typed<T: Serialize + ?Sized>(
        &self,
        category: &str,
        key: &str,
        value: &T,
    ) -> Result<()>;

    /// Load a typed value from storage.
    async fn load_typed<T: DeserializeOwned>(&self, category: &str, key: &str)
        -> Result<Option<T>>;
}

impl<T: MemoryStorage + ?Sized> MemoryStorageExt for T {
    async fn save_typed<U: Serialize + ?Sized>(
        &self,
        category: &str,
        key: &str,
        value: &U,
    ) -> Result<()> {
        let json_value = serde_json::to_value(value)?;
        self.save_json_value(category, key, &json_value).await
    }

    async fn load_typed<U: DeserializeOwned>(
        &self,
        category: &str,
        key: &str,
    ) -> Result<Option<U>> {
        match self.load_json_value(category, key).await? {
            Some(value) => Ok(Some(serde_json::from_value(value)?)),
            None => Ok(None),
        }
    }
}
