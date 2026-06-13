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
        if let Ok(names) = state_store.list_category(category).await
            && !names.is_empty()
        {
            manifest.sections.push(BackupSection {
                name: category.to_string(),
                entry_count: names.len(),
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_manifest_serialization() {
        let manifest = BackupManifest {
            version: 1,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            oxios_version: "0.1.0".to_string(),
            sections: vec![
                BackupSection {
                    name: "seeds".to_string(),
                    entry_count: 42,
                },
                BackupSection {
                    name: "memory/facts".to_string(),
                    entry_count: 100,
                },
            ],
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let restored: BackupManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.version, 1);
        assert_eq!(restored.oxios_version, "0.1.0");
        assert_eq!(restored.sections.len(), 2);
        assert_eq!(restored.sections[0].name, "seeds");
        assert_eq!(restored.sections[0].entry_count, 42);
        assert_eq!(restored.sections[1].name, "memory/facts");
        assert_eq!(restored.sections[1].entry_count, 100);
    }

    #[test]
    fn test_backup_section_ordering() {
        let sections = vec![
            BackupSection {
                name: "a".into(),
                entry_count: 1,
            },
            BackupSection {
                name: "b".into(),
                entry_count: 2,
            },
        ];
        let manifest = BackupManifest {
            version: 1,
            created_at: String::new(),
            oxios_version: String::new(),
            sections,
        };
        assert_eq!(manifest.sections[0].name, "a");
        assert_eq!(manifest.sections[1].name, "b");
    }

    #[test]
    fn test_backup_manifest_empty_sections() {
        let manifest = BackupManifest {
            version: 1,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            oxios_version: "0.1.0".to_string(),
            sections: vec![],
        };
        assert!(manifest.sections.is_empty());

        let json = serde_json::to_string(&manifest).unwrap();
        let restored: BackupManifest = serde_json::from_str(&json).unwrap();
        assert!(restored.sections.is_empty());
    }

    #[tokio::test]
    async fn test_copy_dir_recursive_basic() {
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        // Create source files
        tokio::fs::write(src_dir.path().join("file1.txt"), "hello")
            .await
            .unwrap();
        tokio::fs::create_dir_all(src_dir.path().join("subdir"))
            .await
            .unwrap();
        tokio::fs::write(src_dir.path().join("subdir/file2.txt"), "world")
            .await
            .unwrap();

        copy_dir_recursive(src_dir.path(), dest_dir.path())
            .await
            .unwrap();

        assert!(dest_dir.path().join("file1.txt").exists());
        assert!(dest_dir.path().join("subdir/file2.txt").exists());

        let content = tokio::fs::read_to_string(dest_dir.path().join("file1.txt"))
            .await
            .unwrap();
        assert_eq!(content, "hello");
    }
}
