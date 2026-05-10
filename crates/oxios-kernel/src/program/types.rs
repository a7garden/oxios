//! Program types — metadata, tool definitions, and installation sources.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Program metadata — the OS-level "executable header"
/// Like an ELF header or PE header, but for AI programs
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramHostRequirements {
    /// Required on host (checked at startup)
    pub required: Vec<String>,
    /// Optional on host (checked when needed)
    pub optional: Vec<String>,
}

/// MCP server configuration parsed from `[mcp]` in program.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub env: HashMap<String, String>,
    /// Whether the server is enabled by default.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Definition of a tool exposed by a program.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub path: std::path::PathBuf,
    /// Content of the SKILL.md instruction file
    pub skill_content: String,
    /// Whether the program is enabled
    pub enabled: bool,
}

/// Installation source for a program.
pub enum InstallSource {
    /// Install from a local directory path.
    Local(std::path::PathBuf),
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

/// Result of checking host requirements
#[derive(Debug, Clone, Serialize)]
pub struct HostRequirementsCheck {
    /// Name of the program checked
    pub program_name: String,
    /// Required tools that are missing on the host
    pub missing_required: Vec<String>,
    /// Availability status of optional tools
    pub optional_available: HashMap<String, bool>,
}
