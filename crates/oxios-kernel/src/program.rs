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
}

/// Host tool requirements for a program
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProgramHostRequirements {
    /// Required on host (checked at startup)
    pub required: Vec<String>,
    /// Optional on host (checked when needed)
    pub optional: Vec<String>,
}

/// Tool definition exposed by a program
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDef {
    /// Tool name (unique within the program)
    pub name: String,
    /// Brief description of what the tool does
    pub description: String,
    /// Expected arguments
    pub arguments: Vec<ArgumentDef>,
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

        Ok(ProgramMeta {
            name: toml.program.name,
            version: toml.program.version,
            description: toml.program.description,
            author: toml.program.author,
            tools,
            dependencies: Vec::new(),
            host_requirements,
        })
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

    /// Load all programs from the programs directory
    async fn load_all(&self) -> Result<()> {
        let mut installed = self.installed.write().await;

        if !self.programs_dir.exists() {
            fs::create_dir_all(&self.programs_dir)?;
            return Ok(());
        }

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

    #[test]
    fn test_program_meta_load() {
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
}