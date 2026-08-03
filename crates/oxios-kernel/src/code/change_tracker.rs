//! File change tracking — snapshots before/after agent edits, computes diffs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use similar::TextDiff;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAction {
    Create,
    Modify,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub action: ChangeAction,
    pub original_content: Option<String>,
    pub new_content: Option<String>,
    pub diff: String,
    pub timestamp: DateTime<Utc>,
    pub accepted: bool,
    pub tool_call_id: Option<String>,
}

pub struct ChangeTracker {
    project_root: PathBuf,
    changes: HashMap<PathBuf, FileChange>,
}

impl ChangeTracker {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            changes: HashMap::new(),
        }
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        }
    }

    /// Snapshot original content before a write (idempotent — only first call records).
    pub fn snapshot_before_write(&mut self, path: &Path) -> anyhow::Result<()> {
        let abs = self.resolve(path);
        if self.changes.contains_key(&abs) {
            return Ok(());
        }
        let original = if abs.exists() {
            Some(std::fs::read_to_string(&abs).unwrap_or_default())
        } else {
            None
        };
        self.changes.insert(
            abs.clone(),
            FileChange {
                path: abs,
                action: if original.is_some() {
                    ChangeAction::Modify
                } else {
                    ChangeAction::Create
                },
                original_content: original,
                new_content: None,
                diff: String::new(),
                timestamp: Utc::now(),
                accepted: false,
                tool_call_id: None,
            },
        );
        Ok(())
    }

    /// Record the new content after a write and compute the diff.
    pub fn record_write(
        &mut self,
        path: &Path,
        new_content: &str,
        tool_call_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let abs = self.resolve(path);
        self.snapshot_before_write(path)?;
        let change = self.changes.get_mut(&abs).unwrap();
        change.new_content = Some(new_content.to_string());
        change.timestamp = Utc::now();
        change.tool_call_id = tool_call_id.map(|s| s.to_string());
        let old = change.original_content.as_deref().unwrap_or("");
        change.diff = compute_unified_diff(&abs.to_string_lossy(), old, new_content);
        Ok(())
    }

    /// Record a file deletion.
    pub fn record_delete(&mut self, path: &str, tool_call_id: Option<&str>) -> anyhow::Result<()> {
        let abs = self.resolve(Path::new(path));
        let original = if abs.exists() {
            Some(std::fs::read_to_string(&abs).unwrap_or_default())
        } else {
            None
        };
        self.changes.insert(
            abs.clone(),
            FileChange {
                path: abs,
                action: ChangeAction::Delete,
                original_content: original,
                new_content: None,
                diff: String::new(),
                timestamp: Utc::now(),
                accepted: false,
                tool_call_id: tool_call_id.map(|s| s.to_string()),
            },
        );
        Ok(())
    }

    pub fn list_changes(&self) -> Vec<&FileChange> {
        self.changes.values().collect()
    }

    pub fn get_change(&self, path: &Path) -> Option<&FileChange> {
        let abs = self.resolve(path);
        self.changes.get(&abs)
    }

    /// Accept a change — file keeps new content.
    pub fn accept(&mut self, path: &Path) -> anyhow::Result<()> {
        let abs = self.resolve(path);
        if let Some(change) = self.changes.get_mut(&abs) {
            change.accepted = true;
        }
        Ok(())
    }

    /// Reject a change — revert file to original content.
    pub fn reject(&mut self, path: &Path) -> anyhow::Result<()> {
        let abs = self.resolve(path);
        if let Some(change) = self.changes.get(&abs) {
            match (&change.action, &change.original_content) {
                (ChangeAction::Create, None) => {
                    if abs.exists() {
                        std::fs::remove_file(&abs)?;
                    }
                }
                (_, Some(original)) => {
                    std::fs::write(&abs, original)?;
                }
                _ => {}
            }
        }
        self.changes.remove(&abs);
        Ok(())
    }

    pub fn reject_all(&mut self) -> anyhow::Result<()> {
        let paths: Vec<PathBuf> = self.changes.keys().cloned().collect();
        for path in &paths {
            self.reject(path)?;
        }
        Ok(())
    }

    pub fn accept_all(&mut self) {
        for change in self.changes.values_mut() {
            change.accepted = true;
        }
    }

    pub fn pending_count(&self) -> usize {
        self.changes.values().filter(|c| !c.accepted).count()
    }
}

/// Compute a unified diff using the `similar` crate.
fn compute_unified_diff(filename: &str, old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut output = String::new();
    output.push_str(&format!("--- a/{filename}\n+++ b/{filename}\n"));

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push_str(&hunk.to_string());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_computation() {
        let diff = compute_unified_diff("test.rs", "line1\nline2\n", "line1\nmodified\n");
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
    }

    #[test]
    fn test_change_tracker_create_modify() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "original").unwrap();

        let mut tracker = ChangeTracker::new(tmp.path().to_path_buf());
        tracker.snapshot_before_write(&path).unwrap();
        tracker.record_write(&path, "modified", None).unwrap();

        let changes = tracker.list_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].action, ChangeAction::Modify);
        assert!(changes[0].diff.contains("-original"));
        assert!(changes[0].diff.contains("+modified"));
    }

    #[test]
    fn test_reject_reverts_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "original").unwrap();

        let mut tracker = ChangeTracker::new(tmp.path().to_path_buf());
        tracker.snapshot_before_write(&path).unwrap();
        std::fs::write(&path, "modified").unwrap();
        tracker.record_write(&path, "modified", None).unwrap();

        tracker.reject(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    }
}
