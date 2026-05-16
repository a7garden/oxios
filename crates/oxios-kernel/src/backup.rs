//! Backup and restore for Oxios state.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Backup manifest.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupManifest {
    /// Manifest format version.
    pub version: u32,
    /// Timestamp when the backup was created.
    pub created_at: String,
    /// Oxios version that created the backup.
    pub oxios_version: String,
    /// Sections included in the backup.
    pub sections: Vec<BackupSection>,
}

/// A single section in a backup manifest.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupSection {
    /// Section name (e.g., "seeds", "memory/facts").
    pub name: String,
    /// Number of entries in this section.
    pub entry_count: usize,
}

/// Create a backup by copying the state directory.
pub async fn create_backup(
    state_store: &crate::state_store::StateStore,
    output_path: &Path,
) -> Result<BackupManifest> {
    let mut manifest = BackupManifest {
        version: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
        oxios_version: env!("CARGO_PKG_VERSION").to_string(),
        sections: Vec::new(),
    };

    let categories = [
        "seeds",
        "evals",
        "memory/conversations",
        "memory/sessions",
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "sessions",
        "agent_groups",
    ];

    for category in &categories {
        if let Ok(names) = state_store.list_category(category).await {
            if !names.is_empty() {
                manifest.sections.push(BackupSection {
                    name: category.to_string(),
                    entry_count: names.len(),
                });
            }
        }
    }

    // Copy the entire state directory
    let src = &state_store.base_path;
    if output_path.exists() {
        tokio::fs::remove_dir_all(output_path).await?;
    }
    copy_dir_recursive(src, output_path).await?;

    // Write manifest into backup
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    tokio::fs::write(output_path.join("manifest.json"), manifest_json).await?;

    tracing::info!(path = %output_path.display(), sections = manifest.sections.len(), "Backup created");
    Ok(manifest)
}

/// Restore state from a backup directory.
pub async fn restore_backup(
    state_store: &crate::state_store::StateStore,
    backup_path: &Path,
) -> Result<BackupManifest> {
    let manifest_data = tokio::fs::read_to_string(backup_path.join("manifest.json"))
        .await
        .context("Backup missing manifest.json")?;
    let manifest: BackupManifest = serde_json::from_str(&manifest_data)?;

    // Copy backup into state directory
    copy_dir_recursive(backup_path, &state_store.base_path).await?;

    tracing::info!(path = %backup_path.display(), sections = manifest.sections.len(), "Backup restored");
    Ok(manifest)
}

fn copy_dir_recursive<'a>(
    src: &'a Path,
    dest: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(dest).await?;
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dest_path).await?;
            } else {
                tokio::fs::copy(&src_path, &dest_path).await?;
            }
        }
        Ok(())
    })
}
