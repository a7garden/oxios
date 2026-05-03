//! Filesystem-based state store.
//!
//! All state is persisted as markdown or JSON files organized
//! by category. This is the "filesystem" of Oxios.

use anyhow::{bail, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// A filesystem-based persistent state store.
///
/// Files are organized as `<base_path>/<category>/<name>.md` or
/// `<base_path>/<category>/<name>.json`.
#[derive(Clone)]
pub struct StateStore {
    /// Root directory for all state files.
    pub base_path: PathBuf,
}

impl StateStore {
    /// Creates a new state store, initializing the directory if needed.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        Ok(Self { base_path })
    }

    /// Validate that a category name does not contain path traversal.
    fn validate_category(category: &str) -> Result<()> {
        if category.contains("..") || category.contains('/') || category.contains('\\') {
            bail!("invalid category name: '{}'", category);
        }
        Ok(())
    }

    /// Validate that a file name does not contain path traversal.
    fn validate_name(name: &str) -> Result<()> {
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            bail!("invalid file name: '{}'", name);
        }
        Ok(())
    }

    /// Save a markdown file under the given category.
    pub async fn save_markdown(&self, category: &str, name: &str, content: &str) -> Result<()> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let dir = self.base_path.join(category);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{name}.md"));
        fs::write(path, content).await?;
        Ok(())
    }

    /// Load a markdown file from the given category.
    pub async fn load_markdown(&self, category: &str, name: &str) -> Result<Option<String>> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let path = self.base_path.join(category).join(format!("{name}.md"));
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all markdown files in a category (names without extension).
    pub async fn list_category(&self, category: &str) -> Result<Vec<String>> {
        Self::validate_category(category)?;
        let dir = self.base_path.join(category);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&dir).await?;
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "json" {
                    if let Some(stem) = path.file_stem() {
                        names.push(stem.to_string_lossy().into_owned());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Save a serializable value as JSON under the given category.
    pub async fn save_json<T: Serialize>(&self, category: &str, name: &str, data: &T) -> Result<()> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let dir = self.base_path.join(category);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{name}.json"));
        let content = serde_json::to_string_pretty(data)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Load a deserializable value from JSON in the given category.
    pub async fn load_json<T: DeserializeOwned>(&self, category: &str, name: &str) -> Result<Option<T>> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let path = self.base_path.join(category).join(format!("{name}.json"));
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl std::fmt::Debug for StateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateStore")
            .field("base_path", &self.base_path)
            .finish()
    }
}
