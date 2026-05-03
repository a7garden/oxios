//! MCP (Model Context Protocol) integration layer.
//!
//! MCP is a vertical protocol for communication between AI agents and tools/data sources.
//! See: Anthropic MCP specification.
//!
//! This module provides protocol awareness and integration points for MCP.
//! Full MCP integration requires the MCP spec details; this is a stub that
//! documents the protocol and provides the integration structure.
//!
//! # Protocol Overview
//!
//! MCP defines several message types:
//! - `initialize` - Establish connection with capabilities negotiation
//! - `tools/list` - List available tools from a server
//! - `tools/call` - Execute a tool with arguments
//! - `resources/list` - List available resources
//! - `resources/read` - Read a resource by URI
//!
//! # Architecture
//!
//! ```text
//! Agent → MCPBridge → McpServer (stdin/stdout JSON-RPC)
//!                ↓
//!            ToolDef (converted to oxios format)
//! ```

use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::program::ToolDef;

/// MCP server capability definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Server name (unique identifier)
    pub name: String,
    /// Command to execute (e.g., "npx", "python")
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether this server is enabled
    pub enabled: bool,
}

impl McpServer {
    /// Create a new MCP server configuration
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
            args: Vec::new(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    /// Set command arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }
}

/// MCP protocol message types
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "method", content = "params")]
pub enum McpMessage {
    /// Initialize connection with a server
    Initialize {
        /// Protocol version to use
        protocol_version: String,
        /// Client capabilities
        capabilities: McpCapabilities,
    },

    /// List available tools
    #[default]
    ToolsList,

    /// Call a specific tool
    ToolsCall {
        /// Name of the tool to call
        name: String,
        /// Tool arguments
        arguments: serde_json::Value,
    },

    /// List available resources
    ResourcesList,

    /// Read a resource by URI
    ResourcesRead {
        /// URI of the resource to read
        uri: String,
    },
}

/// MCP server capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpCapabilities {
    /// Whether the server supports tools
    pub tools: bool,
    /// Whether the server supports resources
    pub resources: bool,
    /// Whether the server supports prompts
    pub prompts: bool,
}

/// MCP JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID for correlation
    pub id: usize,
    /// Method name to invoke
    pub method: String,
    /// Optional method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Create a new JSON-RPC request
    pub fn new(id: usize, method: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params: None,
        }
    }

    /// Add parameters to the request
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }
}

/// MCP JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Response ID (matches request ID)
    pub id: usize,
    /// Response result if successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error if request failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP tool list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsResult {
    /// List of available tools
    pub tools: Vec<McpTool>,
}

/// MCP tool definition from a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name (unique within the server)
    pub name: String,
    /// Brief description of what the tool does
    pub description: String,
    /// JSON Schema for tool input arguments
    pub input_schema: serde_json::Value,
}

impl McpTool {
    /// Convert an MCP tool to an oxios ToolDef
    pub fn to_tool_def(&self) -> ToolDef {
        let arguments = if let Some(obj) = self.input_schema.as_object() {
            obj.iter()
                .map(|(name, schema)| {
                    let description = schema
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("No description")
                        .to_string();
                    let required = schema
                        .get("default").is_none();

                    super::ArgumentDef {
                        name: name.clone(),
                        description,
                        required,
                        default: schema.get("default").and_then(|d| d.as_str().map(String::from)),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        ToolDef {
            name: self.name.clone(),
            description: self.description.clone(),
            arguments,
        }
    }
}

/// MCP bridge — connects MCP servers to the oxios tool system
///
/// Note: This is a stub implementation. Full MCP integration requires:
/// 1. stdio communication with MCP servers
/// 2. JSON-RPC request/response handling
/// 3. Server lifecycle management
/// 4. Tool schema conversion
pub struct McpBridge {
    /// Registered MCP servers
    servers: Vec<McpServer>,
    /// Cache of tool schemas per server
    tool_cache: HashMap<String, Vec<ToolDef>>,
}

impl McpBridge {
    /// Create a new MCP bridge
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            tool_cache: HashMap::new(),
        }
    }

    /// Register an MCP server
    pub fn register_server(&mut self, server: McpServer) {
        self.servers.push(server);
    }

    /// Get all registered servers
    pub fn servers(&self) -> &[McpServer] {
        &self.servers
    }

    /// Get a server by name
    pub fn get_server(&self, name: &str) -> Option<&McpServer> {
        self.servers.iter().find(|s| s.name == name)
    }

    /// Get a mutable server by name
    pub fn get_server_mut(&mut self, name: &str) -> Option<&mut McpServer> {
        self.servers.iter_mut().find(|s| s.name == name)
    }

    /// Get all available tools from registered MCP servers
    ///
    /// Note: This is a stub. Full implementation would:
    /// 1. Spawn each MCP server process
    /// 2. Send tools/list request
    /// 3. Parse and cache the response
    pub async fn list_tools(&self) -> Result<Vec<ToolDef>> {
        // For now, return cached tools if available
        let mut all_tools = Vec::new();

        for (server_name, tools) in &self.tool_cache {
            if let Some(server) = self.servers.iter().find(|s| &s.name == server_name) {
                if server.enabled {
                    all_tools.extend(tools.clone());
                }
            }
        }

        Ok(all_tools)
    }

    /// Call an MCP tool
    ///
    /// Note: This is a stub. Full implementation would:
    /// 1. Look up the server for the tool
    /// 2. Send tools/call request via stdio
    /// 3. Parse and return the result
    pub async fn call_tool(
        &self,
        _server_name: &str,
        _tool_name: &str,
        _args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Stub implementation
        anyhow::bail!("MCP tool calling not yet implemented")
    }

    /// Refresh tools from a specific server
    ///
    /// Note: This is a stub. Full implementation would actually
    /// communicate with the MCP server.
    pub async fn refresh_tools(&mut self, _server_name: &str) -> Result<()> {
        // Stub: In a real implementation, this would:
        // 1. Spawn/connect to the MCP server
        // 2. Send tools/list request
        // 3. Parse response and update cache
        Ok(())
    }

    /// Clear the tool cache for a server
    pub fn clear_cache(&mut self, server_name: &str) {
        self.tool_cache.remove(server_name);
    }

    /// Clear all caches
    pub fn clear_all_caches(&mut self) {
        self.tool_cache.clear();
    }
}

impl Default for McpBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_builder() {
        let server = McpServer::new("test-server", "npx")
            .with_args(vec!["-y".to_string(), "@anthropic/mcp-server".to_string()])
            .with_env("DEBUG", "true");

        assert_eq!(server.name, "test-server");
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "@anthropic/mcp-server"]);
        assert_eq!(server.env.get("DEBUG"), Some(&"true".to_string()));
        assert!(server.enabled);
    }

    #[test]
    fn test_mcp_request() {
        let request = McpRequest::new(1, "tools/list");
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, 1);
        assert_eq!(request.method, "tools/list");
        assert!(request.params.is_none());
    }

    #[test]
    fn test_mcp_tool_conversion() {
        let mcp_tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "arg1": {
                    "type": "string",
                    "description": "First argument"
                },
                "arg2": {
                    "type": "number",
                    "description": "Second argument",
                    "default": "42"
                }
            }),
        };

        let tool_def = mcp_tool.to_tool_def();

        assert_eq!(tool_def.name, "test_tool");
        assert_eq!(tool_def.description, "A test tool");
        assert_eq!(tool_def.arguments.len(), 2);
    }

    #[test]
    fn test_bridge_registration() {
        let mut bridge = McpBridge::new();

        let server = McpServer::new("test", "echo");
        bridge.register_server(server);

        assert_eq!(bridge.servers().len(), 1);
        assert!(bridge.get_server("test").is_some());
    }
}