//! Storage abstraction for the memory subsystem.
//!
//! The memory crate does not depend on `oxios-kernel`. Instead, it
//! operates against abstract traits. `oxios-kernel` implements
//! these traits for its `StateStore` and `GitLayer`.

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Persistent storage backend for memory entries.
///
/// Implemented by `oxios-kernel::StateStore` (file-based) and
/// can be implemented by any other storage backend.
///
/// The core trait methods operate on `serde_json::Value` to remain
/// dyn-compatible. Typed convenience helpers are provided as default
/// methods that serialize/deserialize through `Value`.
#[async_trait]
pub trait MemoryStorage: Send + Sync {
    /// Save a JSON value to a category/key.
    async fn save_json_value(&self, category: &str, key: &str, value: &Value) -> Result<()>;

    /// Load a JSON value from a category/key.
    async fn load_json_value(&self, category: &str, key: &str) -> Result<Option<Value>>;

    /// List all keys in a category.
    async fn list_category(&self, category: &str) -> Result<Vec<String>>;

    /// Delete a file by category/key. Returns true if the file existed.
    async fn delete_file(&self, category: &str, key: &str) -> Result<bool>;
}

/// Typed helpers for `MemoryStorage`.
///
/// These are extension methods that serialize through `Value`, so the
/// trait remains dyn-compatible.
#[async_trait]
pub trait MemoryStorageExt: MemoryStorage {
    /// Save a typed serializable value as JSON.
    async fn save_json<T: Serialize + Send + Sync>(
        &self,
        category: &str,
        key: &str,
        value: &T,
    ) -> Result<()> {
        let v = serde_json::to_value(value)?;
        self.save_json_value(category, key, &v).await
    }

    /// Load a typed deserializable value from JSON.
    async fn load_json<T: DeserializeOwned + Send>(
        &self,
        category: &str,
        key: &str,
    ) -> Result<Option<T>> {
        match self.load_json_value(category, key).await? {
            Some(v) => Ok(Some(serde_json::from_value(v)?)),
            None => Ok(None),
        }
    }
}

impl<S: MemoryStorage + ?Sized> MemoryStorageExt for S {}

/// Optional git-backed durability for memory entries.
#[async_trait]
pub trait MemoryGit: Send + Sync {
    /// Commit a file to git.
    async fn commit_file(&self, path: &str, message: &str) -> Result<()>;
    /// Whether git integration is enabled.
    fn is_enabled(&self) -> bool;
}

/// A single file/directory entry from a markdown knowledge base.
#[derive(Debug, Clone)]
pub struct NoteEntry {
    /// Filename with extension (e.g., `"Rust.md"`).
    pub name: String,
    /// Parent directory (e.g., `"brain"` or `"/"`).
    pub parent_dir: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
}

/// Abstract markdown knowledge source.
///
/// Implemented by `oxios_markdown::KnowledgeBase` in the kernel.
/// Allows `auto_bridge` to read `.md` files without depending on
/// `oxios-markdown` directly.
pub trait MarkdownSource: Send + Sync {
    /// Re-index all markdown files. Returns file count.
    fn index_all(&self) -> Result<usize>;
    /// List files/directories under `dir`.
    fn note_tree(&self, dir: &str) -> Result<Vec<NoteEntry>>;
    /// Read a note's content. Returns `None` if not found.
    fn note_read(&self, path: &str) -> Result<Option<String>>;
    /// Extract markdown headings from content.
    fn extract_headings(&self, content: &str) -> Vec<String>;
}
