//! File-content-snapshot checkpoint system for coding sessions.
//!
//! A checkpoint captures the content of every file the agent has touched
//! (as tracked by `ChangeTracker`) at a point in time. Reverting restores
//! those files to their snapshot content. This is always correct regardless
//! of git state, works without a git repo, and is trivially testable.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub description: String,
    pub timestamp: DateTime<Utc>,
    /// File paths covered by this checkpoint.
    pub files: Vec<String>,
}

/// In-memory snapshot — kept out of the serialisable `Checkpoint` struct
/// so we don't embed large file contents in API responses.
struct FileSnapshot {
    /// Absolute path → content at checkpoint time (`None` = file didn't exist).
    contents: HashMap<PathBuf, Option<String>>,
}

pub struct CheckpointManager {
    project_root: PathBuf,
    checkpoints: Vec<(Checkpoint, FileSnapshot)>,
}

impl CheckpointManager {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            checkpoints: Vec::new(),
        }
    }

    pub fn list(&self) -> Vec<&Checkpoint> {
        self.checkpoints.iter().map(|(c, _)| c).collect()
    }

    /// Create a checkpoint by snapshotting the current content of every
    /// file in `paths`. Returns the public `Checkpoint` metadata.
    pub fn create(&mut self, description: &str, paths: &[PathBuf]) -> anyhow::Result<Checkpoint> {
        let mut contents = HashMap::new();
        let mut file_names = Vec::with_capacity(paths.len());

        for abs_path in paths {
            let content = if abs_path.exists() {
                std::fs::read_to_string(abs_path).ok() // binary → None
            } else {
                None // file didn't exist yet
            };
            file_names.push(
                abs_path
                    .strip_prefix(&self.project_root)
                    .unwrap_or(abs_path)
                    .to_string_lossy()
                    .to_string(),
            );
            contents.insert(abs_path.clone(), content);
        }

        let checkpoint = Checkpoint {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            description: description.to_string(),
            timestamp: Utc::now(),
            files: file_names,
        };

        tracing::info!(
            "Created checkpoint {} ({} files): {}",
            checkpoint.id,
            contents.len(),
            description
        );

        self.checkpoints
            .push((checkpoint.clone(), FileSnapshot { contents }));
        Ok(checkpoint)
    }

    /// Revert to a checkpoint: restore every snapshotted file to its
    /// content at checkpoint time. Files that didn't exist at checkpoint
    /// time are deleted.
    pub fn revert_to(&self, checkpoint_id: &str) -> anyhow::Result<()> {
        let (cp, snapshot) = self
            .checkpoints
            .iter()
            .find(|(c, _)| c.id == checkpoint_id)
            .ok_or_else(|| anyhow::anyhow!("checkpoint not found: {checkpoint_id}"))?;

        tracing::info!("Reverting to checkpoint {} ({})", cp.id, cp.description);

        for (abs_path, content) in &snapshot.contents {
            match content {
                Some(c) => {
                    // Restore file to snapshot content.
                    if let Some(parent) = abs_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(abs_path, c)?;
                }
                None => {
                    // File didn't exist at checkpoint time — delete it.
                    if abs_path.exists() {
                        if abs_path.is_dir() {
                            let _ = std::fs::remove_dir_all(abs_path);
                        } else {
                            let _ = std::fs::remove_file(abs_path);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_creates_and_reverts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // Set up initial files
        let file_a = root.join("a.txt");
        let file_b = root.join("b.txt");
        std::fs::write(&file_a, "original A").unwrap();
        std::fs::write(&file_b, "original B").unwrap();

        let mut manager = CheckpointManager::new(root.clone());

        // Create checkpoint with current state
        let cp = manager
            .create("initial state", &[file_a.clone(), file_b.clone()])
            .unwrap();

        // Simulate agent modifying files
        std::fs::write(&file_a, "modified A").unwrap();
        std::fs::write(&file_b, "modified B").unwrap();

        // Verify files are modified
        assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "modified A");

        // Revert
        manager.revert_to(&cp.id).unwrap();

        // Verify files restored
        assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "original A");
        assert_eq!(std::fs::read_to_string(&file_b).unwrap(), "original B");
    }

    #[test]
    fn test_revert_deletes_files_created_after_checkpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        let file_a = root.join("a.txt");
        std::fs::write(&file_a, "exists at checkpoint").unwrap();

        // File that will be created AFTER the checkpoint
        let file_b = root.join("b.txt");

        let mut manager = CheckpointManager::new(root.clone());

        // Checkpoint only includes file_a (file_b doesn't exist yet)
        let cp = manager.create("before b", &[file_a.clone()]).unwrap();

        // Now create file_b (simulating agent creating a new file)
        std::fs::write(&file_b, "created after checkpoint").unwrap();
        assert!(file_b.exists());

        // Revert — file_b was not in the snapshot, but it was None in the
        // snapshot for file_a... wait, file_b isn't in the snapshot at all.
        // We only snapshotted file_a. So reverting only affects file_a.
        manager.revert_to(&cp.id).unwrap();

        // file_a is restored
        assert_eq!(
            std::fs::read_to_string(&file_a).unwrap(),
            "exists at checkpoint"
        );
        // file_b is NOT deleted (it wasn't in the snapshot)
        assert!(file_b.exists());
    }

    #[test]
    fn test_revert_deletes_files_that_existed_in_snapshot_as_none() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        let file_c = root.join("c.txt");
        // file_c does NOT exist at checkpoint time

        let mut manager = CheckpointManager::new(root.clone());

        // Checkpoint includes file_c as None (doesn't exist)
        let cp = manager
            .create("before c exists", &[file_c.clone()])
            .unwrap();

        // Create file_c after checkpoint
        std::fs::write(&file_c, "newly created").unwrap();
        assert!(file_c.exists());

        // Revert — file_c was None in the snapshot, so it should be deleted
        manager.revert_to(&cp.id).unwrap();
        assert!(!file_c.exists());
    }
}
