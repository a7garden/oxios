//! Suite loading — discovers and loads TOML task definitions from the filesystem.

use crate::task::Task;
use anyhow::Context;
use std::path::{Path, PathBuf};

/// A collection of tasks loaded from a directory.
#[derive(Debug, Clone)]
pub struct Suite {
    /// Suite name (directory name).
    pub name: String,
    /// Directory where tasks are defined.
    pub path: PathBuf,
    /// Loaded tasks.
    pub tasks: Vec<Task>,
}

impl Suite {
    /// Load all suites from a base directory.
    ///
    /// Expected structure:
    /// ```text
    /// base_dir/
    ///   ouroboros/
    ///     simple.toml
    ///     interview.toml
    ///   agent/
    ///     fork.toml
    ///   ...
    /// ```
    pub fn load_all(base_dir: &Path) -> anyhow::Result<Vec<Suite>> {
        let mut suites = Vec::new();

        if !base_dir.exists() {
            anyhow::bail!("Suites directory not found: {}", base_dir.display());
        }

        let mut entries: Vec<_> = std::fs::read_dir(base_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let suite_name = entry
                .file_name()
                .to_str()
                .unwrap_or("unknown")
                .to_string();
            let suite = Suite::load(&entry.path(), &suite_name)?;
            if !suite.tasks.is_empty() {
                suites.push(suite);
            }
        }

        Ok(suites)
    }

    /// Load a single suite from a directory.
    pub fn load(dir: &Path, name: &str) -> anyhow::Result<Suite> {
        let mut tasks = Vec::new();

        let mut entries: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "toml")
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Reading {}", path.display()))?;

            match crate::task::TaskToml::parse(&content) {
                Ok(toml) => match toml.into_task(None) {
                    Ok(task) => {
                        tracing::debug!("Loaded task: {} ({})", task.id, task.name);
                        tasks.push(task);
                    }
                    Err(e) => {
                        tracing::warn!("Skipping {} — parse error: {}", path.display(), e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Skipping {} — TOML error: {}", path.display(), e);
                }
            }
        }

        Ok(Suite {
            name: name.to_string(),
            path: dir.to_path_buf(),
            tasks,
        })
    }

    /// Get the default suites directory (relative to the crate root).
    pub fn default_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("suites")
    }
}

/// Filter tasks based on CLI options.
pub fn filter_tasks(
    suites: &[Suite],
    tier: Option<crate::Tier>,
    suite_name: Option<&str>,
    tag: Option<&str>,
    task_id: Option<&str>,
) -> Vec<Task> {
    let mut tasks = Vec::new();

    for suite in suites {
        // Filter by suite name
        if let Some(name) = suite_name {
            if suite.name != name {
                continue;
            }
        }

        for task in &suite.tasks {
            // Filter by tier
            if let Some(t) = tier {
                if task.tier != t {
                    continue;
                }
            }

            // Filter by tag
            if let Some(tag) = tag {
                let tag_clean = tag.trim_start_matches('@');
                if !task.tags.iter().any(|t| t == tag_clean) {
                    continue;
                }
            }

            // Filter by task ID
            if let Some(id) = task_id {
                if task.id != id {
                    continue;
                }
            }

            tasks.push(task.clone());
        }
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let suite = Suite::load(dir.path(), "empty").unwrap();
        assert!(suite.tasks.is_empty());
    }

    #[test]
    fn test_load_single_task() {
        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
[task]
id = "test_task"
name = "Test Task"
tier = "integration"
suite = "test"
prompt = "hello"

[expect]
phase_reached = "Execute"
"#;
        fs::write(dir.path().join("test.toml"), toml).unwrap();
        let suite = Suite::load(dir.path(), "test").unwrap();
        assert_eq!(suite.tasks.len(), 1);
        assert_eq!(suite.tasks[0].id, "test_task");
    }

    #[test]
    fn test_filter_by_suite() {
        let tasks = vec![Task {
            id: "a".to_string(),
            name: "A".to_string(),
            tier: crate::Tier::Integration,
            suite: "ouroboros".to_string(),
            tags: vec!["smoke".to_string()],
            timeout_secs: 60,
            prompt: Some("test".to_string()),
            turns: vec![],
            fixtures: vec![],
            context_file: None,
            assertions: vec![],
        }];
        let suite = Suite {
            name: "ouroboros".to_string(),
            path: PathBuf::new(),
            tasks,
        };

        let filtered = filter_tasks(&[suite], None, Some("ouroboros"), None, None);
        assert_eq!(filtered.len(), 1);

        let filtered = filter_tasks(
            &[Suite {
                name: "ouroboros".to_string(),
                path: PathBuf::new(),
                tasks: vec![],
            }],
            None,
            Some("agent"),
            None,
            None,
        );
        assert!(filtered.is_empty());
    }
}
