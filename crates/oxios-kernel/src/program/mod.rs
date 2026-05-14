//! Programs: OS-level installable applications for AI agents.
//!
//! A program is the OS-level concept of an installable application.
//! Like Unix has /bin programs, Oxios has programs that agents can "execute"
//! to gain capabilities through their SKILL.md instruction files.
//!
//! # Structure
//!
//! A program directory contains:
//! - `program.toml` - metadata (name, version, description, tools, dependencies)
//! - `SKILL.md` - instruction file (like a man page)
//! - optional `bin/` - executables
//! - optional `config/` - configuration files
//!
//! # Philosophy
//!
//! Programs are READ-ONLY instruction sets. They don't execute themselves;
//! they provide guidelines and tools that agents consume. Think of them as
//! man pages that come with metadata for discovery.

mod installer;
mod parser;
mod types;

pub use types::{
    ArgumentDef, HostRequirementsCheck, InstallSource, McpServerConfig, Program,
    ProgramHostRequirements, ProgramMeta, ProgramState, ToolDef,
};

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::sync::RwLock;

use crate::host_tools::HostToolValidator;

use installer::copy_dir_all;

/// Program manager — handles installation, uninstallation, and discovery
pub struct ProgramManager {
    /// Directory where programs are installed
    programs_dir: PathBuf,
    /// In-memory cache of installed programs
    installed: RwLock<HashMap<String, Program>>,
}

impl ProgramManager {
    /// Create a new program manager
    pub fn new(programs_dir: PathBuf) -> Self {
        Self {
            programs_dir,
            installed: RwLock::new(HashMap::new()),
        }
    }

    /// Get the programs directory path
    pub fn programs_dir(&self) -> &Path {
        &self.programs_dir
    }

    /// Initialize the program manager, loading all installed programs
    pub async fn init(&self) -> Result<()> {
        self.load_all().await
    }

    /// Load all programs from the programs directory.
    /// If the directory is empty, bootstrap from the `.programs/` directory in the oxios repo root.
    async fn load_all(&self) -> Result<()> {
        if !self.programs_dir.exists() {
            fs::create_dir_all(&self.programs_dir)?;
        }

        // Count installed programs
        let count = fs::read_dir(&self.programs_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count();

        // Bootstrap from `.programs/` (oxios repo root) if directory is empty.
        // This allows `.programs/` in the repo to serve as the default program source.
        if count == 0 {
            Self::bootstrap_defaults(&self.programs_dir).await?;
        }

        let mut installed = self.installed.write().await;
        for entry in fs::read_dir(&self.programs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Ok(program) = self.load_program(&path) {
                    installed.insert(program.meta.name.clone(), program);
                }
            }
        }

        Ok(())
    }

    /// Bootstrap default programs from the `.programs/` directory in the oxios repo root.
    /// This copies (not symlinks) so the programs directory is self-contained.
    async fn bootstrap_defaults(target_dir: &Path) -> Result<()> {
        let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".programs");
        if !source_dir.exists() {
            tracing::info!("No .programs/ directory found, skipping bootstrap");
            return Ok(());
        }

        tracing::info!(source = %source_dir.display(), "Bootstrapping default programs");

        for entry in fs::read_dir(&source_dir)? {
            let entry = entry?;
            let src = entry.path();
            if src.is_dir() {
                let name = match src.file_name().map(|n| n.to_string_lossy().into_owned()) {
                    Some(n) if !n.is_empty() => n,
                    _ => continue,
                };
                let dest = target_dir.join(&*name);

                // Only copy if not already present
                if !dest.exists() {
                    copy_dir_all(&src, &dest)?;
                    tracing::info!(program = %name, "Bootstrapped default program");
                }
            }
        }

        Ok(())
    }

    /// Load a single program from a directory
    fn load_program(&self, path: &Path) -> Result<Program> {
        let meta = ProgramMeta::load_from_dir(path)?;

        let skill_path = path.join("SKILL.md");
        let skill_content = if skill_path.exists() {
            fs::read_to_string(&skill_path).unwrap_or_default()
        } else {
            String::new()
        };

        Ok(Program {
            meta,
            path: path.to_path_buf(),
            skill_content,
            enabled: self.load_program_state(path).map(|s| s.enabled).unwrap_or(true),
        })
    }

    /// List all installed programs
    pub async fn list_programs(&self) -> Vec<Program> {
        let installed = self.installed.read().await;
        installed.values().cloned().collect()
    }

    /// List all enabled programs
    pub async fn list_enabled(&self) -> Vec<Program> {
        let installed = self.installed.read().await;
        installed.values().filter(|p| p.enabled).cloned().collect()
    }

    /// Get a specific program by name
    pub async fn get_program(&self, name: &str) -> Option<Program> {
        let installed = self.installed.read().await;
        installed.get(name).cloned()
    }

    /// Install a program from a directory
    pub async fn install(&self, source_path: &Path) -> Result<Program> {
        // Load the source program
        let source_meta = ProgramMeta::load_from_dir(source_path)?;
        let source_skill = source_path.join("SKILL.md");
        let skill_content = if source_skill.exists() {
            fs::read_to_string(&source_skill)?
        } else {
            String::new()
        };

        // Create the destination path
        let dest_path = self.programs_dir.join(&source_meta.name);
        if dest_path.exists() {
            anyhow::bail!("Program '{}' is already installed", source_meta.name);
        }

        // Copy the directory recursively
        copy_dir_all(source_path, &dest_path)?;

        // Create initial state file
        let state = ProgramState::new();
        let state_json = serde_json::to_string_pretty(&state)?;
        fs::write(dest_path.join("state.json"), state_json)?;

        // Create the program entry
        let program = Program {
            meta: source_meta,
            path: dest_path,
            skill_content,
            enabled: true,
        };

        // Add to the installed map
        let mut installed = self.installed.write().await;
        installed.insert(program.meta.name.clone(), program.clone());

        Ok(program)
    }

    /// Install a program from an [InstallSource].
    pub async fn install_from(&self, source: InstallSource) -> Result<Program> {
        match source {
            InstallSource::Local(path) => self.install_from_local(&path).await,
            InstallSource::Git { url, branch } => self.install_from_git(&url, branch.as_deref()).await,
            InstallSource::Tarball { url } => self.install_from_tarball(&url).await,
        }
    }

    /// Install a program from a local directory path.
    async fn install_from_local(&self, source_path: &Path) -> Result<Program> {
        self.install(source_path).await
    }

    /// Install a program by cloning a git repository, then installing the result.
    async fn install_from_git(&self, url: &str, branch: Option<&str>) -> Result<Program> {
        // Create a temporary directory for the clone.
        let temp_dir = tempfile::tempdir().map_err(|e| anyhow::anyhow!("tempfile: {}", e))?;
        let clone_path = temp_dir.path();

        // Clone the repository.
        tracing::info!(url, branch = ?branch, "Cloning git repository");
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("clone");
        if let Some(branch) = branch {
            cmd.arg("--branch").arg(branch);
        }
        cmd.arg("--depth").arg("1");
        cmd.arg(url);
        cmd.arg(clone_path);

        let output = cmd.output().await
            .with_context(|| format!("Failed to run git clone for '{}'", url))?;

        if !output.status.success() {
            anyhow::bail!(
                "git clone failed (exit {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Find the cloned program directory (single top-level directory expected).
        let entries: Vec<_> = std::fs::read_dir(clone_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let program_dir = if entries.len() == 1 {
            entries
                .into_iter()
                .next()
                .map(|e| e.path())
                .unwrap_or_else(|| clone_path.to_path_buf())
        } else {
            clone_path.to_path_buf()
        };

        let program = self.install(&program_dir).await?;

        tracing::info!(name = %program.meta.name, "Program installed from git");
        Ok(program)
    }

    /// Install a program by downloading and extracting a tarball.
    async fn install_from_tarball(&self, url: &str) -> Result<Program> {
        // Create a temporary directory for download and extraction.
        let temp_dir = tempfile::tempdir().map_err(|e| anyhow::anyhow!("tempfile: {}", e))?;
        let download_path = temp_dir.path().join("program.tar.gz");
        let extract_base = temp_dir.path().join("extracted");

        // Download the tarball with curl.
        tracing::info!(url, "Downloading tarball");
        let curl = tokio::process::Command::new("curl")
            .arg("-fsSL")
            .arg("-o")
            .arg(&download_path)
            .arg(url)
            .output()
            .await
            .with_context(|| format!("Failed to run curl for '{}'", url))?;

        if !curl.status.success() {
            anyhow::bail!(
                "curl failed (exit {}): {}",
                curl.status,
                String::from_utf8_lossy(&curl.stderr)
            );
        }

        // Extract the tarball.
        tracing::info!("Extracting tarball");
        let tar = tokio::process::Command::new("tar")
            .arg("-xzf")
            .arg(&download_path)
            .arg("-C")
            .arg(&extract_base)
            .output()
            .await
            .with_context(|| "Failed to run tar to extract tarball")?;

        if !tar.status.success() {
            anyhow::bail!(
                "tar extraction failed (exit {}): {}",
                tar.status,
                String::from_utf8_lossy(&tar.stderr)
            );
        }

        // Find the extracted program directory.
        let entries: Vec<_> = std::fs::read_dir(&extract_base)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let program_dir = if entries.len() == 1 {
            entries
                .into_iter()
                .next()
                .map(|e| e.path())
                .unwrap_or_else(|| extract_base.to_path_buf())
        } else {
            extract_base.to_path_buf()
        };

        let program = self.install(&program_dir).await?;

        tracing::info!(name = %program.meta.name, "Program installed from tarball");
        Ok(program)
    }

    /// Uninstall a program
    pub async fn uninstall(&self, name: &str) -> Result<()> {
        let mut installed = self.installed.write().await;

        let program = installed.remove(name)
            .ok_or_else(|| anyhow::anyhow!("Program '{}' not found", name))?;

        // Remove the directory
        if program.path.exists() {
            fs::remove_dir_all(&program.path)?;
        }

        Ok(())
    }

    /// Enable or disable a program (persisted across restarts).
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut installed = self.installed.write().await;

        let program = installed.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Program '{}' not found", name))?;

        program.enabled = enabled;

        // Persist to state.json
        self.persist_state(&program.path, enabled)?;

        tracing::info!(name, enabled, "Program enabled state persisted");
        Ok(())
    }

    /// Check if host requirements are met for a program
    pub async fn check_host_requirements(&self, name: &str) -> Result<HostRequirementsCheck> {
        let installed = self.installed.read().await;

        let program = installed.get(name)
            .ok_or_else(|| anyhow::anyhow!("Program '{}' not found", name))?;

        let validator = HostToolValidator::new(
            program.meta.host_requirements.required.clone(),
            program.meta.host_requirements.optional.clone(),
        );

        let missing_required = validator.validate_required();
        let optional_status = validator.check_optional();

        Ok(HostRequirementsCheck {
            program_name: name.to_string(),
            missing_required,
            optional_available: optional_status,
        })
    }

    /// Get tool schemas for all enabled programs
    pub async fn all_tool_schemas(&self) -> Vec<ToolDef> {
        let installed = self.installed.read().await;
        installed.values()
            .filter(|p| p.enabled)
            .flat_map(|p| p.meta.tools.clone())
            .collect()
    }

    /// Get skill content for a specific program
    pub async fn get_skill_content(&self, name: &str) -> Option<String> {
        let installed = self.installed.read().await;
        installed.get(name).map(|p| p.skill_content.clone())
    }

    /// Upgrade an existing program, or install if not present.
    ///
    /// Compares versions using SemVer. If the new version is the same,
    /// returns the existing program (no-op). If different, performs
    /// atomic replace: uninstall old → install new, preserving the
    /// enabled state.
    pub async fn upgrade(&self, source_path: &Path) -> Result<Program> {
        let source_meta = ProgramMeta::load_from_dir(source_path)?;

        // Check if already installed
        let existing = self.get_program(&source_meta.name).await;

        if let Some(ref old) = existing {
            let cmp = compare_versions(&source_meta.version, &old.meta.version);
            match cmp {
                VersionCmp::Equal => {
                    tracing::info!(
                        name = %source_meta.name,
                        version = %source_meta.version,
                        "Program already at same version — no upgrade needed"
                    );
                    return Ok(old.clone());
                }
                VersionCmp::Older => {
                    tracing::warn!(
                        name = %source_meta.name,
                        old = %old.meta.version,
                        new = %source_meta.version,
                        "Downgrade requested — proceeding"
                    );
                }
                VersionCmp::Newer => {
                    tracing::info!(
                        name = %source_meta.name,
                        old = %old.meta.version,
                        new = %source_meta.version,
                        "Upgrading program"
                    );
                }
            }

            // Preserve enabled state across upgrade
            let was_enabled = old.enabled;

            // Atomic replace: uninstall old then install new
            self.uninstall(&source_meta.name).await?;
            let mut program = self.install(source_path).await?;

            // Restore enabled state
            if !was_enabled {
                program.enabled = false;
                self.persist_state(&program.path, false)?;
                // Also update in-memory
                let mut installed = self.installed.write().await;
                if let Some(p) = installed.get_mut(&source_meta.name) {
                    p.enabled = false;
                }
            }

            tracing::info!(
                name = %program.meta.name,
                version = %program.meta.version,
                enabled = program.enabled,
                "Program upgraded"
            );

            Ok(program)
        } else {
            // Not installed — just install
            tracing::info!(
                name = %source_meta.name,
                "Program not installed — performing fresh install"
            );
            self.install(source_path).await
        }
    }

    // ── State Persistence Helpers ──

    /// Load program state from state.json in the program directory.
    fn load_program_state(&self, path: &Path) -> Result<ProgramState> {
        let state_path = path.join("state.json");
        if !state_path.exists() {
            return Ok(ProgramState::default());
        }
        let json = fs::read_to_string(&state_path)?;
        let state: ProgramState = serde_json::from_str(&json)?;
        Ok(state)
    }

    /// Persist enabled state to state.json in the program directory.
    fn persist_state(&self, program_path: &Path, enabled: bool) -> Result<()> {
        let mut state = self.load_program_state(program_path).unwrap_or_default();
        state.enabled = enabled;
        state.last_modified = chrono::Utc::now().to_rfc3339();
        let state_json = serde_json::to_string_pretty(&state)?;
        fs::write(program_path.join("state.json"), state_json)?;
        Ok(())
    }
}

// ── Version Comparison ─────────────────────────────────────────────────────────

/// Result of comparing two SemVer version strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VersionCmp {
    /// Versions are identical.
    Equal,
    /// First version is newer.
    Newer,
    /// First version is older.
    Older,
}

/// Compare two SemVer version strings (major.minor.patch).
///
/// Handles `v` prefix and missing components gracefully.
fn compare_versions(a: &str, b: &str) -> VersionCmp {
    let parse = |v: &str| -> Vec<u32> {
        v.strip_prefix('v')
            .unwrap_or(v)
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    for i in 0..va.len().max(vb.len()) {
        let na = va.get(i).unwrap_or(&0);
        let nb = vb.get(i).unwrap_or(&0);
        match na.cmp(nb) {
            std::cmp::Ordering::Greater => return VersionCmp::Newer,
            std::cmp::Ordering::Less => return VersionCmp::Older,
            std::cmp::Ordering::Equal => continue,
        }
    }
    VersionCmp::Equal
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- ProgramMeta tests ---

    #[test]
    fn test_program_meta_load_minimal() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        // Create a minimal program.toml
        let toml_content = r#"
[program]
name = "test-program"
version = "1.0.0"
description = "A test program"
author = "Test Author"

[tools]
my_tool = { description = "A test tool" }
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();
        fs::write(program_dir.join("SKILL.md"), "# Test Program\n\nThis is a test.").unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert_eq!(meta.name, "test-program");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.description, "A test program");
        assert_eq!(meta.author, "Test Author");
        assert_eq!(meta.tools.len(), 1);
        assert_eq!(meta.tools[0].name, "my_tool");
        assert_eq!(meta.tools[0].description, "A test tool");
        assert!(meta.tools[0].arguments.is_empty());
    }

    #[test]
    fn test_program_meta_with_tools_and_args() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        let toml_content = r#"
[program]
name = "rich-program"
version = "2.0.0"
description = "A program with rich tools"
author = "Author"

[tools.greet]
description = "Greets a user"
arguments = [
    { name = "name", description = "User name", required = true },
    { name = "loud", description = "Shout", required = false, default = "false" }
]

[tools.farewell]
description = "Says goodbye"
arguments = []
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert_eq!(meta.tools.len(), 2);

        // Find greet by name (order may vary).
        let greet = meta.tools.iter().find(|t| t.name == "greet").expect("greet tool");
        assert_eq!(greet.arguments.len(), 2);

        // Verify arguments exist and have correct structure.
        let name_arg = greet.arguments.iter().find(|a| a.name == "name").expect("name arg");
        assert!(name_arg.required, "name should be required");
        assert_eq!(name_arg.description, "User name");

        let loud_arg = greet.arguments.iter().find(|a| a.name == "loud").expect("loud arg");
        assert_eq!(loud_arg.default, Some("false".to_string()));
    }

    #[test]
    fn test_program_meta_with_dependencies() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        let toml_content = r#"
[program]
name = "test-with-deps"
version = "1.0.0"
description = "A test program with dependencies"
author = "Test Author"

[host_requirements]
required = ["git", "gh"]
optional = ["jq", "curl"]
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert_eq!(meta.host_requirements.required, vec!["git", "gh"]);
        assert_eq!(meta.host_requirements.optional, vec!["jq", "curl"]);
    }

    #[test]
    fn test_program_meta_missing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        // No program.toml — should error.
        let result = ProgramMeta::load_from_dir(program_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_program_meta_empty_optional_sections() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        let toml_content = r#"
[program]
name = "minimal"
version = "1.0.0"
description = "Minimal"
author = "X"
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert!(meta.tools.is_empty());
        assert!(meta.host_requirements.required.is_empty());
        assert!(meta.host_requirements.optional.is_empty());
        assert!(meta.dependencies.is_empty());
    }

    #[test]
    fn test_requires_tools_parsed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        let toml_content = r#"
[program]
name = "needs-tools"
version = "1.0.0"
description = "A program that requires tools"
author = "Test"

[requires_tools]
names = ["read", "exec"]
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert_eq!(meta.dependencies, vec!["read", "exec"]);
    }

    #[test]
    fn test_requires_tools_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        let toml_content = r#"
[program]
name = "no-reqs"
version = "1.0.0"
description = "No requirements"
author = "Test"

[requires_tools]
names = []
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert!(meta.dependencies.is_empty());
    }

    #[test]
    fn test_requires_tools_single() {
        let temp_dir = tempfile::tempdir().unwrap();
        let program_dir = temp_dir.path();

        let toml_content = r#"
[program]
name = "single-req"
version = "1.0.0"
description = "Single requirement"
author = "Test"

[requires_tools]
names = ["grep"]
"#;

        fs::write(program_dir.join("program.toml"), toml_content).unwrap();

        let meta = ProgramMeta::load_from_dir(program_dir).unwrap();

        assert_eq!(meta.dependencies, vec!["grep"]);
    }

    // --- ProgramManager tests ---

    #[tokio::test]
    async fn test_program_manager_init_creates_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        assert!(programs_dir.exists());
    }

    #[tokio::test]
    async fn test_list_programs_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        let programs = manager.list_programs().await;
        assert!(programs.is_empty());
    }

    #[tokio::test]
    async fn test_get_program_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = ProgramManager::new(temp_dir.path().join("programs"));
        manager.init().await.unwrap();

        assert!(manager.get_program("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_install_program() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source_dir = temp_dir.path().join("source-program");

        // Create source program.
        fs::create_dir_all(&source_dir).unwrap();
        let toml = r#"
[program]
name = "my-program"
version = "1.0.0"
description = "My program"
author = "Test"

[tools.hello]
description = "Says hello"
"#;
        fs::write(source_dir.join("program.toml"), toml).unwrap();
        fs::write(source_dir.join("SKILL.md"), "# My Program\n\nDoes things.").unwrap();

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        let installed = manager.install(&source_dir).await.unwrap();

        assert_eq!(installed.meta.name, "my-program");
        assert_eq!(installed.meta.version, "1.0.0");
        assert!(installed.enabled);
        assert!(!installed.skill_content.is_empty());

        // Verify it's listed.
        let programs = manager.list_programs().await;
        assert_eq!(programs.len(), 1);
        assert_eq!(programs[0].meta.name, "my-program");

        // Verify directory exists.
        assert!(programs_dir.join("my-program").exists());
    }

    #[tokio::test]
    async fn test_install_duplicate_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source1 = temp_dir.path().join("src1");
        let source2 = temp_dir.path().join("src2");

        for src in [&source1, &source2] {
            fs::create_dir_all(src).unwrap();
            let toml = r#"
[program]
name = "dup"
version = "1.0.0"
description = "X"
author = "X"
"#;
            fs::write(src.join("program.toml"), toml).unwrap();
        }

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        manager.install(&source1).await.unwrap();
        let result = manager.install(&source2).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already installed"));
    }

    #[tokio::test]
    async fn test_uninstall_program() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source = temp_dir.path().join("to-uninstall");

        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("program.toml"), r#"
[program]
name = "removable"
version = "1.0.0"
description = "X"
author = "X"
"#).unwrap();

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        manager.install(&source).await.unwrap();
        assert!(manager.get_program("removable").await.is_some());

        manager.uninstall("removable").await.unwrap();
        assert!(manager.get_program("removable").await.is_none());

        // Directory should be gone.
        assert!(!programs_dir.join("removable").exists());
    }

    #[tokio::test]
    async fn test_uninstall_nonexistent_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = ProgramManager::new(temp_dir.path().join("programs"));
        manager.init().await.unwrap();

        let result = manager.uninstall("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_enabled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source = temp_dir.path().join("toggle-me");

        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("program.toml"), r#"
[program]
name = "toggle-me"
version = "1.0.0"
description = "X"
author = "X"
"#).unwrap();

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        manager.install(&source).await.unwrap();

        let prog = manager.get_program("toggle-me").await.unwrap();
        assert!(prog.enabled);

        manager.set_enabled("toggle-me", false).await.unwrap();
        let prog = manager.get_program("toggle-me").await.unwrap();
        assert!(!prog.enabled);

        manager.set_enabled("toggle-me", true).await.unwrap();
        let prog = manager.get_program("toggle-me").await.unwrap();
        assert!(prog.enabled);
    }

    #[tokio::test]
    async fn test_all_tool_schemas() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        // Install two programs with tools.
        for (name, tool_name) in [("prog-a", "tool-a"), ("prog-b", "tool-b")] {
            let src = temp_dir.path().join(name);
            fs::create_dir_all(&src).unwrap();
            let toml = format!(r#"
[program]
name = "{}"
version = "1.0.0"
description = "X"
author = "X"

[tools.{}]
description = "A tool"
"#, name, tool_name);
            fs::write(src.join("program.toml"), toml).unwrap();
        }

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        // Install prog-a (enabled by default), disable prog-b.
        let src_a = temp_dir.path().join("prog-a");
        let src_b = temp_dir.path().join("prog-b");
        manager.install(&src_a).await.unwrap();
        manager.install(&src_b).await.unwrap();
        manager.set_enabled("prog-b", false).await.unwrap();

        let schemas = manager.all_tool_schemas().await;
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].name, "tool-a");
    }

    #[tokio::test]
    async fn test_get_skill_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source = temp_dir.path().join("skill-test");

        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("program.toml"), r#"
[program]
name = "skill-test"
version = "1.0.0"
description = "X"
author = "X"
"#).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "# Skill Test\n\nUse this program like so.",
        )
        .unwrap();

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        manager.install(&source).await.unwrap();

        let content = manager.get_skill_content("skill-test").await;
        assert!(content.is_some());
        assert!(content.unwrap().contains("Skill Test"));
    }

    #[tokio::test]
    async fn test_check_host_requirements() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source = temp_dir.path().join("req-check");

        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("program.toml"), r#"
[program]
name = "req-check"
version = "1.0.0"
description = "X"
author = "X"

[host_requirements]
required = ["git"]
optional = ["echo", "nonexistent-tool-xyz"]
"#).unwrap();

        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();

        manager.install(&source).await.unwrap();

        let check = manager.check_host_requirements("req-check").await.unwrap();
        assert_eq!(check.program_name, "req-check");
        // git should be available on most systems.
        assert!(check.missing_required.is_empty());
        // Optional tools status.
        assert!(check.optional_available["echo"]);
        assert!(!check.optional_available["nonexistent-tool-xyz"]);
    }

    #[tokio::test]
    async fn test_check_host_requirements_program_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = ProgramManager::new(temp_dir.path().join("programs"));
        manager.init().await.unwrap();

        let result = manager.check_host_requirements("ghost").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_copy_dir_all() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src = temp_dir.path().join("src");
        let dst = temp_dir.path().join("dst");

        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("file.txt"), "content").unwrap();
        fs::write(src.join("subdir").join("nested.txt"), "nested").unwrap();

        copy_dir_all(&src, &dst).unwrap();

        assert!(dst.join("file.txt").exists());
        assert!(dst.join("subdir").join("nested.txt").exists());
        assert_eq!(fs::read_to_string(dst.join("file.txt")).unwrap(), "content");
    }

    // ── Version Comparison Tests ──

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), VersionCmp::Equal);
        assert_eq!(compare_versions("2.3.4", "2.3.4"), VersionCmp::Equal);
    }

    #[test]
    fn test_compare_versions_newer() {
        assert_eq!(compare_versions("2.0.0", "1.0.0"), VersionCmp::Newer);
        assert_eq!(compare_versions("1.1.0", "1.0.0"), VersionCmp::Newer);
        assert_eq!(compare_versions("1.0.1", "1.0.0"), VersionCmp::Newer);
        assert_eq!(compare_versions("1.0.0", "0.9.9"), VersionCmp::Newer);
    }

    #[test]
    fn test_compare_versions_older() {
        assert_eq!(compare_versions("1.0.0", "2.0.0"), VersionCmp::Older);
        assert_eq!(compare_versions("1.0.0", "1.1.0"), VersionCmp::Older);
    }

    #[test]
    fn test_compare_versions_with_v_prefix() {
        assert_eq!(compare_versions("v1.0.0", "1.0.0"), VersionCmp::Equal);
        assert_eq!(compare_versions("v2.0.0", "v1.0.0"), VersionCmp::Newer);
    }

    #[test]
    fn test_compare_versions_missing_components() {
        assert_eq!(compare_versions("1.0", "1.0.0"), VersionCmp::Equal);
        assert_eq!(compare_versions("2", "1.0.0"), VersionCmp::Newer);
    }

    // ── State Persistence Tests ──

    #[test]
    fn test_program_state_default() {
        let state = ProgramState::default();
        assert!(state.enabled);
        assert!(!state.installed_at.is_empty());
        assert!(!state.last_modified.is_empty());
    }

    #[test]
    fn test_program_state_with_enabled() {
        let state = ProgramState::new().with_enabled(false);
        assert!(!state.enabled);
    }

    #[tokio::test]
    async fn test_state_json_created_on_install() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source_dir = temp_dir.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("program.toml"), r#"
[program]
name = "state-test"
version = "1.0.0"
description = "Test"
author = "Test"
"#).unwrap();
        fs::write(source_dir.join("SKILL.md"), "# Test").unwrap();

        let manager = ProgramManager::new(programs_dir);
        manager.init().await.unwrap();
        let program = manager.install(&source_dir).await.unwrap();

        // state.json should exist
        let state_path = program.path.join("state.json");
        assert!(state_path.exists());

        let state: ProgramState =
            serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
        assert!(state.enabled);
    }

    #[tokio::test]
    async fn test_set_enabled_persists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source_dir = temp_dir.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("program.toml"), r#"
[program]
name = "toggle-test"
version = "1.0.0"
description = "Test"
author = "Test"
"#).unwrap();
        fs::write(source_dir.join("SKILL.md"), "# Test").unwrap();

        let manager = ProgramManager::new(programs_dir);
        manager.init().await.unwrap();
        let program = manager.install(&source_dir).await.unwrap();

        // Disable
        manager.set_enabled("toggle-test", false).await.unwrap();

        // Verify state.json reflects disabled
        let state: ProgramState =
            serde_json::from_str(&fs::read_to_string(program.path.join("state.json")).unwrap()).unwrap();
        assert!(!state.enabled);

        // Re-enable
        manager.set_enabled("toggle-test", true).await.unwrap();
        let state: ProgramState =
            serde_json::from_str(&fs::read_to_string(program.path.join("state.json")).unwrap()).unwrap();
        assert!(state.enabled);
    }

    #[tokio::test]
    async fn test_enabled_state_survives_reload() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");
        let source_dir = temp_dir.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("program.toml"), r#"
[program]
name = "persist-test"
version = "1.0.0"
description = "Test"
author = "Test"
"#).unwrap();
        fs::write(source_dir.join("SKILL.md"), "# Test").unwrap();

        // First manager: install and disable
        let manager = ProgramManager::new(programs_dir.clone());
        manager.init().await.unwrap();
        manager.install(&source_dir).await.unwrap();
        manager.set_enabled("persist-test", false).await.unwrap();
        drop(manager);

        // Second manager: reload from disk — state should be disabled
        let manager2 = ProgramManager::new(programs_dir);
        manager2.init().await.unwrap();
        let reloaded = manager2.get_program("persist-test").await.unwrap();
        assert!(!reloaded.enabled, "disabled state should survive restart");
    }

    // ── Upgrade Tests ──

    fn make_program_dir(parent: &Path, name: &str, version: &str) -> PathBuf {
        let dir = parent.join(name);
        fs::create_dir_all(&dir).unwrap();
        let toml = format!(
            "[program]\nname = \"{}\"\nversion = \"{}\"\ndescription = \"Test\"\nauthor = \"Test\"\n",
            name, version,
        );
        fs::write(dir.join("program.toml"), toml).unwrap();
        fs::write(dir.join("SKILL.md"), "# Test").unwrap();
        dir
    }

    #[tokio::test]
    async fn test_upgrade_same_version_is_noop() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        let manager = ProgramManager::new(programs_dir);
        manager.init().await.unwrap();

        // Install v1.0.0
        let v1_dir = make_program_dir(temp_dir.path(), "up-test", "1.0.0");
        manager.install(&v1_dir).await.unwrap();

        // Upgrade to same v1.0.0 — should be no-op
        let v1_dir2 = make_program_dir(&temp_dir.path().join("v1copy"), "up-test", "1.0.0");
        let result = manager.upgrade(&v1_dir2).await.unwrap();
        assert_eq!(result.meta.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_upgrade_newer_version() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        let manager = ProgramManager::new(programs_dir);
        manager.init().await.unwrap();

        // Install v1.0.0
        let v1_dir = make_program_dir(temp_dir.path(), "up-test", "1.0.0");
        manager.install(&v1_dir).await.unwrap();

        // Upgrade to v2.0.0
        let v2_dir = make_program_dir(&temp_dir.path().join("v2"), "up-test", "2.0.0");
        let result = manager.upgrade(&v2_dir).await.unwrap();
        assert_eq!(result.meta.version, "2.0.0");
    }

    #[tokio::test]
    async fn test_upgrade_preserves_enabled_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        let manager = ProgramManager::new(programs_dir);
        manager.init().await.unwrap();

        // Install v1.0.0 and disable it
        let v1_dir = make_program_dir(temp_dir.path(), "up-test", "1.0.0");
        manager.install(&v1_dir).await.unwrap();
        manager.set_enabled("up-test", false).await.unwrap();

        // Upgrade to v2.0.0 — should stay disabled
        let v2_dir = make_program_dir(&temp_dir.path().join("v2"), "up-test", "2.0.0");
        let result = manager.upgrade(&v2_dir).await.unwrap();
        assert_eq!(result.meta.version, "2.0.0");
        assert!(!result.enabled, "disabled state should be preserved across upgrade");
    }

    #[tokio::test]
    async fn test_upgrade_installs_if_not_present() {
        let temp_dir = tempfile::tempdir().unwrap();
        let programs_dir = temp_dir.path().join("programs");

        let manager = ProgramManager::new(programs_dir);
        manager.init().await.unwrap();

        // Upgrade without prior install — should just install
        let v1_dir = make_program_dir(temp_dir.path(), "fresh-test", "1.0.0");
        let result = manager.upgrade(&v1_dir).await.unwrap();
        assert_eq!(result.meta.name, "fresh-test");
        assert_eq!(result.meta.version, "1.0.0");
        assert!(result.enabled);
    }
}
