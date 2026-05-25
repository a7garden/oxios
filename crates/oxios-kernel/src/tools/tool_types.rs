//! Generic tool definition types.
//!
//! `ToolDef` and `ArgumentDef` are shared across MCP adapters and other
//! tool-bridging code. They originally lived in the `program` module but
//! were extracted here as part of the RFC-009 Skill unification.

use serde::{Deserialize, Serialize};

/// Definition of a tool exposed by an external source (MCP server, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    /// Tool name (unique within the source).
    pub name: String,
    /// Brief description of what the tool does.
    pub description: String,
    /// Expected arguments.
    pub arguments: Vec<ArgumentDef>,
    /// Command to execute (first word = binary, rest = default args).
    #[serde(default)]
    pub command: String,
}

/// Argument definition for a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentDef {
    /// Argument name.
    pub name: String,
    /// Description of the argument.
    pub description: String,
    /// Whether this argument is required.
    pub required: bool,
    /// Default value if not provided.
    pub default: Option<String>,
}
