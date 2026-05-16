//! Program TOML parsing — load program metadata from program.toml files.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::types::{ArgumentDef, McpServerConfig, ProgramHostRequirements, ProgramMeta, ToolDef};

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

        let tools = toml
            .tools
            .map(|t| {
                t.into_iter()
                    .map(|(name, tool)| {
                        let arguments = tool
                            .arguments
                            .unwrap_or_default()
                            .into_iter()
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
            })
            .unwrap_or_default();

        let host_requirements = toml
            .host_requirements
            .map(|hr| ProgramHostRequirements {
                required: hr.required.unwrap_or_default(),
                optional: hr.optional.unwrap_or_default(),
            })
            .unwrap_or_default();

        let dependencies = toml.requires_tools.map(|rt| rt.names).unwrap_or_default();

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
