//! Workspace fixture management — setup and teardown for task isolation.

use crate::task::Fixture;
use std::path::{Path, PathBuf};

/// Manages a temporary workspace for a single task execution.
pub struct FixtureManager {
    /// Temporary directory root.
    dir: tempfile::TempDir,
    /// Workspace path inside the temp dir.
    workspace: PathBuf,
}

impl FixtureManager {
    /// Create a new isolated workspace.
    pub fn new(task_id: &str) -> anyhow::Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix(&format!("oxios-bench-{}-", task_id))
            .tempdir()?;
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace)?;

        Ok(Self { dir, workspace })
    }

    /// Get the workspace path.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Create fixture files in the workspace.
    pub fn setup_fixtures(&self, fixtures: &[Fixture]) -> anyhow::Result<()> {
        for fixture in fixtures {
            let path = self.workspace.join(&fixture.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &fixture.content)?;
            tracing::debug!("Created fixture: {}", fixture.path.display());
        }
        Ok(())
    }

    /// Check if a file exists in the workspace.
    pub fn file_exists(&self, relative_path: &Path) -> bool {
        self.workspace.join(relative_path).exists()
    }

    /// Read a file from the workspace.
    pub fn read_file(&self, relative_path: &Path) -> Option<String> {
        std::fs::read_to_string(self.workspace.join(relative_path)).ok()
    }

    /// Clean up the temporary workspace.
    pub fn cleanup(self) -> anyhow::Result<()> {
        self.dir.close()?;
        Ok(())
    }

    /// Keep the temporary directory (for debugging). Returns the path.
    pub fn persist(self) -> PathBuf {
        self.dir.keep()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_setup() {
        let mgr = FixtureManager::new("test").unwrap();
        let fixtures = vec![
            Fixture {
                path: PathBuf::from("hello.txt"),
                content: "Hello, World!".to_string(),
            },
            Fixture {
                path: PathBuf::from("sub/deep.txt"),
                content: "nested".to_string(),
            },
        ];
        mgr.setup_fixtures(&fixtures).unwrap();

        assert!(mgr.file_exists(Path::new("hello.txt")));
        assert!(mgr.file_exists(Path::new("sub/deep.txt")));
        assert_eq!(mgr.read_file(Path::new("hello.txt")).unwrap(), "Hello, World!");
    }
}
