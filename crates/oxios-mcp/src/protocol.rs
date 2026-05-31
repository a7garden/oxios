//! JSON-RPC 2.0 protocol types and MCP domain types.
//!
//! This module defines the wire types for MCP communication (JSON-RPC requests,
//! responses, errors) and the domain-specific types for tools, capabilities,
//! and initialization negotiation.
//!
//! This module is fully independent of the Oxios kernel. It defines MCP-native
//! types only. Conversion to Oxios-specific types (like `ToolDef`) is handled
//! by the kernel's adapter layer.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Unique ID generator for JSON-RPC requests
// ---------------------------------------------------------------------------

static REQUEST_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

pub(crate) fn next_request_id() -> usize {
    REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// MCP Server Configuration
// ---------------------------------------------------------------------------

/// Type alias for backwards compatibility — use [McpServer] directly.
pub type McpServerConfig = McpServer;

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

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 Protocol Types
// ---------------------------------------------------------------------------

/// MCP JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID for correlation
    pub id: serde_json::Value,
    /// Method name to invoke
    pub method: String,
    /// Optional method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Create a new JSON-RPC request with an auto-generated ID
    pub fn new(method: &str) -> Self {
        Self::with_id(next_request_id(), method)
    }

    /// Create a new JSON-RPC request with a specific ID
    pub fn with_id(id: usize, method: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(id),
            method: method.to_string(),
            params: None,
        }
    }

    /// Add parameters to the request
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }

    /// Serialize to a JSON-line (JSONL) bytes ready for stdio write
    pub fn to_jsonl(&self) -> Result<Vec<u8>> {
        let mut buf = serde_json::to_vec(self)?;
        buf.push(b'\n');
        Ok(buf)
    }
}

/// MCP JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Response ID (matches request ID)
    pub id: serde_json::Value,
    /// Response result if successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error if request failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP JSON-RPC error structure
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

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error_type = match self.code {
            -32700 => "parse error",
            -32600 => "invalid request",
            -32601 => "method not found",
            -32602 => "invalid params",
            -32603 => "internal error",
            -32099..=-32000 => "server error",
            _ => "unknown error",
        };
        write!(f, "{} (code {}): {}", error_type, self.code, self.message)
    }
}

impl McpError {
    /// Create a new MCP error
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }

    /// JSON-RPC parse error (-32700)
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    /// JSON-RPC invalid request (-32600)
    pub fn invalid_request(msg: &str) -> Self {
        Self::new(-32600, msg)
    }

    /// JSON-RPC method not found (-32601)
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }

    /// JSON-RPC invalid params (-32602)
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }

    /// JSON-RPC internal error (-32603)
    pub fn internal_error(msg: &str) -> Self {
        Self::new(-32603, msg)
    }

    /// Server error (codes -32000 to -32099)
    pub fn server_error(msg: &str) -> Self {
        Self::new(-32000, msg)
    }
}

impl McpResponse {
    /// Check if this response contains an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Extract the result value, erroring if there is one
    pub fn into_result(self) -> Result<serde_json::Value> {
        if let Some(err) = self.error {
            return Err(anyhow!("{err}"));
        }
        Ok(self.result.unwrap_or(serde_json::Value::Null))
    }
}

// ---------------------------------------------------------------------------
// MCP Capability Negotiation
// ---------------------------------------------------------------------------

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

/// Initialize request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    /// Protocol version string.
    pub protocol_version: String,
    /// Client capabilities.
    pub capabilities: McpCapabilities,
    /// Information about the connecting client.
    pub client_info: ClientInfo,
}

/// Client info sent during initialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name.
    pub name: String,
    /// Client version.
    pub version: String,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpCapabilities::default(),
            client_info: ClientInfo {
                name: "oxios".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }
}

/// Initialize response from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    /// Protocol version agreed upon.
    pub protocol_version: String,
    /// Server capabilities.
    pub capabilities: McpCapabilities,
    /// Information about the server.
    pub server_info: ServerInfo,
}

/// Server info from initialize response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server name.
    pub name: String,
    /// Server version.
    pub version: String,
}

// ---------------------------------------------------------------------------
// MCP Tool Types
// ---------------------------------------------------------------------------

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
    /// Get the tool name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the tool description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the tool input JSON Schema.
    pub fn input_schema(&self) -> &serde_json::Value {
        &self.input_schema
    }
}

/// MCP tools/list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsResult {
    /// Available tools from the server.
    pub tools: Vec<McpTool>,
}

/// MCP tools/call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    /// Content blocks returned by the tool.
    pub content: Vec<McpContentBlock>,
    /// Whether the result is an error.
    pub is_error: Option<bool>,
}

/// Content block in a tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContentBlock {
    /// Plain text content.
    #[serde(rename = "text")]
    Text {
        /// The text content.
        text: String,
    },
    /// Base64-encoded image data.
    #[serde(rename = "image")]
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image.
        mime_type: Option<String>,
    },
    /// Embedded resource reference.
    #[serde(rename = "resource")]
    Resource {
        /// The referenced resource.
        resource: MappedResource,
    },
}

/// Resource reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedResource {
    /// URI of the resource.
    pub uri: String,
    /// MIME type of the resource.
    pub mime_type: Option<String>,
}
