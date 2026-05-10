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

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::host_tools::HostToolValidator;

/// Program metadata — the OS-level "executable header"
/// Like an ELF header or PE header, but for AI programs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProgramMeta {
    /// Program name (unique identifier)
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Human-readable description
    pub description: String,
    /// Author name
    pub author: String,
    /// Tools this program provides (maps tool name → description)
    pub tools: Vec<ToolDef>,
    /// Other programs this program depends on
    pub dependencies: Vec<String>,
    /// Host tools this program requires to function
    pub host_requirements: ProgramHostRequirements,
    /// MCP servers this program connects to (parsed from [mcp] table)
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Host tool requirements for a program
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProgramHostRequirements {
    /// Required on host (checked at startup)
    pub required: Vec<String>,
    /// Optional on host (checked when needed)
    pub optional: Vec<String>,
}

/// MCP server configuration parsed from `[mcp]` in program.toml.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpServerConfig {
    /// Server name identifier.
    pub name: String,
    /// Command to launch the MCP server.
    pub command: String,
    /// Command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the server process.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Whether the server is enabled by default.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}
/// Definition of a tool exposed by a program.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDef {
    /// Tool name (unique within the program)
    pub name: String,
    /// Brief description of what the tool does
    pub description: String,
    /// Expected arguments
    pub arguments: Vec<ArgumentDef>,
    /// Command to execute (first word = binary, rest = default args)
    #[serde(default)]
    pub command: String,
}

/// Argument definition for a tool
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArgumentDef {
    /// Argument name
    pub name: String,
    /// Description of the argument
    pub description: String,
    /// Whether this argument is required
    pub required: bool,
    /// Default value if not provided
    pub default: Option<String>,
}

/// Program installed in the OS
#[derive(Debug, Clone)]
pub struct Program {
    /// Program metadata
    pub meta: ProgramMeta,
    /// Path to the program directory
    pub path: PathBuf,
    /// Content of the SKILL.md instruction file
    pub skill_content: String,
    /// Whether the program is enabled
    pub enabled: bool,
}

/// Parsed program.toml structure
#[derive(Debug, Clone, serde::Deserialize)]
struct TomlProgram {
    program: TomlProgramInfo,
    tools: Option<HashMap<String, TomlTool>>,
    #[serde(rename = "host_requirements")]
    host_requirements: Option<TomlHostRequirements>,
    #[serde(rename = "requires_tools")]
    requires_tools: Option<TomlRequiresTools>,
    #[serde(rename = "mcp", default)]
    mcp: Option<Vec<McpServerConfig>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TomlProgramInfo {
    name: String,
    version: String,
    description: String,
    author: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TomlTool {
    description: String,
    /// Command to execute (first word = binary, rest = default args)
    #[serde(default)]
    command: String,
    arguments: Option<Vec<TomlArgument>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TomlArgument {
    name: String,
    description: String,
    required: Option<bool>,
    default: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TomlHostRequirements {
    required: Option<Vec<String>>,
    optional: Option<Vec<String>>,
}

/// Required tools for a program to function.
#[derive(Debug, Clone, serde::Deserialize)]
struct TomlRequiresTools {
    names: Vec<String>,
}

impl ProgramMeta {
    /// Load program metadata from a directory
    pub fn load_from_dir(path: &Path) -> Result<Self> {
        let toml_path = path.join("program.toml");
        let content = fs::read_to_string(&toml_path)
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;

        let toml: TomlProgram = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", toml_path.display()))?;

        let tools = toml.tools.map(|t| {
            t.into_iter()
                .map(|(name, tool)| {
                    let arguments = tool.arguments.unwrap_or_default().into_iter()
                        .map(|arg| ArgumentDef {
                            name: arg.name,
                            description: arg.description,
                            required: arg.required.unwrap_or(true),
                            default: arg.default,
                        })
                        .collect();
                    ToolDef {
                        name,
                        description: tool.description,
                        arguments,
                        command: tool.command,
                    }
                })
                .collect()
        }).unwrap_or_default();

        let host_requirements = toml.host_requirements
            .map(|hr| ProgramHostRequirements {
                required: hr.required.unwrap_or_default(),
                optional: hr.optional.unwrap_or_default(),
            })
            .unwrap_or_default();

        let dependencies = toml.requires_tools
            .map(|rt| rt.names)
            .unwrap_or_default();

        let mcp_servers = toml.mcp.unwrap_or_default();

        Ok(ProgramMeta {
            name: toml.program.name,
            version: toml.program.version,
            description: toml.program.description,
            author: toml.program.author,
            tools,
            dependencies,
            host_requirements,
            mcp_servers,
        })
    }
}

/// Installation source for a program.
pub enum InstallSource {
    /// Install from a local directory path.
    Local(PathBuf),
    /// Install from a git repository.
    Git {
        /// Git repository URL.
        url: String,
        /// Optional branch to checkout.
        branch: Option<String>,
    },
    /// Install from a tarball URL.
    Tarball {
        /// Tarball URL (http/https).
        url: String,
    },
}

impl ProgramManager {
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
        let mut cmd = Command::new("git");
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
        let curl = Command::new("curl")
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
        let tar = Command::new("tar")
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
}

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
            enabled: true,
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

    /// Enable or disable a program
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut installed = self.installed.write().await;

        let program = installed.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Program '{}' not found", name))?;

        program.enabled = enabled;
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
}

/// Result of checking host requirements
#[derive(Debug, Clone, serde::Serialize)]
pub struct HostRequirementsCheck {
    /// Name of the program checked
    pub program_name: String,
    /// Required tools that are missing on the host
    pub missing_required: Vec<String>,
    /// Availability status of optional tools
    pub optional_available: HashMap<String, bool>,
}

/// Copy directory recursively
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }

    Ok(())
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
}