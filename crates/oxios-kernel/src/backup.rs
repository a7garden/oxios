//! State backup and restore utilities.
//!
//! Provides functions to create and restore snapshots of the Oxios
//! state store, including sessions, skills, and other persisted data.

use anyhow::{Context, Result};
use std::path::Path;

use crate::state_store::StateStore;

/// Metadata for a backup archive.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupMeta {
    /// Version of the backup format.
    pub version: String,
    /// Timestamp when the backup was created.
    pub created_at: String,
    /// Original workspace path.
    pub workspace_path: String,
}

/// Creates a backup of the state store.
///
/// Copies all files from the state store's base path into a backup
/// directory at the given output path. If `output` is `None`, a
/// timestamped backup is created in `<workspace>/backups/`.
pub async fn create_backup(store: &StateStore, output: Option<&Path>) -> Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_dir = match output {
        Some(p) => p.to_path_buf(),
        None => store.base_path.join("backups").join(timestamp.to_string()),
    };

    tokio::fs::create_dir_all(&backup_dir).await?;

    // Recursively copy the state store directory
    copy_dir_recursive(&store.base_path, &backup_dir).await?;

    // Write backup metadata
    let meta = BackupMeta {
        version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: timestamp.to_string(),
        workspace_path: store.base_path.display().to_string(),
    };
    let meta_path = backup_dir.join("backup_meta.json");
    let meta_json = serde_json::to_string_pretty(&meta)?;
    tokio::fs::write(&meta_path, meta_json).await?;

    tracing::info!(path = %backup_dir.display(), "Backup created");
    println!("Backup created at: {}", backup_dir.display());

    Ok(())
}

/// Restores state from a backup directory.
///
/// Copies all files from the backup directory into the state store's
/// base path, replacing current state.
pub async fn restore_backup(store: &StateStore, input: &Path) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Backup directory does not exist: {}", input.display());
    }

    // Verify it looks like a backup (has backup_meta.json)
    let meta_path = input.join("backup_meta.json");
    if !meta_path.exists() {
        anyhow::bail!("Not a valid backup directory (missing backup_meta.json): {}", input.display());
    }

    // Load and display backup metadata
    let meta_content = tokio::fs::read_to_string(&meta_path).await?;
    let meta: BackupMeta = serde_json::from_str(&meta_content)?;
    tracing::info!(
        version = %meta.version,
        created_at = %meta.created_at,
        "Restoring backup"
    );

    // Clear current state and restore from backup
    // We copy backup contents into the state store, excluding backup_meta.json
    let mut entries = tokio::fs::read_dir(input).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let file_name = src_path.file_name().unwrap_or_default().to_string_lossy();

        // Skip the backup metadata file
        if file_name == "backup_meta.json" {
            continue;
        }

        let dest_path = store.base_path.join(file_name.as_ref());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path).await?;
        } else {
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(&src_path, &dest_path).await?;
        }
    }

    tracing::info!("State restored from backup");
    println!("State restored from: {}", input.display());

    Ok(())
}

/// Recursively copy a directory.
async fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dest).await?;

    let mut entries = tokio::fs::read_dir(src).await
        .with_context(|| format!("Failed to read directory: {}", src.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let file_name = src_path.file_name().unwrap_or_default();
        let dest_path = dest.join(file_name);

        if src_path.is_dir() {
            // Skip the backups directory itself to avoid recursive copies
            if file_name == "backups" {
                continue;
            }
            copy_dir_recursive(&src_path, &dest_path).await?;
        } else {
            tokio::fs::copy(&src_path, &dest_path).await?;
        }
    }

    Ok(())
}
