//! Storage abstraction for the memory subsystem.
//!
//! The memory crate does not depend on `oxios-kernel`. Instead, it
//! operates against abstract traits. `oxios-kernel` (will) implement
//! these traits for its `StateStore` and `GitLayer`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Persistent storage backend for memory entries.
#[async_trait]
pub trait MemoryStorage: Send + Sync {
    async fn save_json_value(&self, category: &str, key: &str, value: &Value) -> Result<()>;
    async fn load_json_value(&self, category: &str, key: &str) -> Result<Option<Value>>;
    async fn list_category(&self, category: &str) -> Result<Vec<String>>;
    async fn delete_file_value(&self, category: &str, key: &str) -> Result<()>;
}

/// Optional git-backed durability for memory entries.
#[async_trait]
pub trait MemoryGit: Send + Sync {
    async fn commit_file(&self, path: &str, message: &str) -> Result<()>;
    fn is_enabled(&self) -> bool;
}
