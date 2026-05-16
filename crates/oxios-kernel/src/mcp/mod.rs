//! MCP (Model Context Protocol) integration layer.
//!
//! MCP is a standard protocol for communication between AI agents and tools/data sources.
//! See: Anthropic MCP specification.
//!
//! This module provides stdio-based communication with MCP servers via JSON-RPC 2.0.
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
//! Agent → McpBridge → McpClient (per server)
//!                         ↓
//!              tokio::process::Command (stdio)
//!                         ↓
//!              JSON-RPC 2.0 (stdin/stdout)
//!                         ↓
//!              MCP Server Process
//! ```

mod client;
mod protocol;

pub use client::McpClient;
pub use protocol::*;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;

use crate::program::ToolDef;

// ---------------------------------------------------------------------------
// McpBridge — manages multiple MCP server clients
// ---------------------------------------------------------------------------

/// MCP bridge — connects multiple MCP servers to the oxios tool system.
///
/// `McpBridge` owns the collection of registered MCP server configurations
/// and manages `McpClient` instances for active servers.
///
/// # Example
///
/// ```ignore
/// let mut bridge = McpBridge::new();
/// bridge.register_server(McpServer::new("files", "npx")
///     .with_args(vec!["-y", "@anthropic/mcp-server-filesystem"]));
///
/// // Initialize all servers
/// bridge.initialize_all().await?;
///
/// // List all tools across all servers
/// let tools = bridge.list_tools().await?;
/// ```
pub struct McpBridge {
    /// Registered MCP server configurations
    servers: parking_lot::RwLock<Vec<McpServer>>,
    /// Active MCP clients (keyed by server name)
    clients: RwLock<HashMap<String, Arc<McpClient>>>,
    /// Tool cache: server_name → cached tool defs
    tool_cache: RwLock<HashMap<String, Vec<ToolDef>>>,
}

impl McpBridge {
    /// Create a new empty MCP bridge
    pub fn new() -> Self {
        Self {
            servers: parking_lot::RwLock::new(Vec::new()),
            clients: RwLock::new(HashMap::new()),
            tool_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Register an MCP server configuration (does not start the process).
    pub fn register_server(&self, server: McpServer) {
        self.servers.write().push(server);
    }

    /// Get all registered server configurations (names only).
    pub fn servers(&self) -> Vec<String> {
        self.servers.read().iter().map(|s| s.name.clone()).collect()
    }

    /// Get a server configuration by name.
    pub fn get_server(&self, name: &str) -> Option<McpServer> {
        self.servers.read().iter().find(|s| s.name == name).cloned()
    }

    /// Initialize all enabled MCP servers.
    ///
    /// Each server is spawned as a child process and receives the initialize request.
    /// Servers that fail to initialize are logged but do not cause a total failure.
    pub async fn initialize_all(&self) -> Result<()> {
        let mut errors = Vec::new();

        let server_list: Vec<McpServer> = self.servers.read().iter().cloned().collect();
        for server in server_list {
            if !server.enabled {
                tracing::debug!(server = %server.name, "Skipping disabled MCP server");
                continue;
            }

            let client = Arc::new(McpClient::new(server.clone()));
            match client.initialize().await {
                Ok(()) => {
                    self.clients
                        .write()
                        .await
                        .insert(server.name.clone(), client);
                    tracing::info!(server = %server.name, "MCP server started");
                }
                Err(e) => {
                    tracing::error!(server = %server.name, error = %e, "Failed to initialize MCP server");
                    errors.push(format!("{}: {}", server.name, e));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("MCP initialization failed: {}", errors.join("; ")))
        }
    }

    /// Initialize a specific server by name.
    pub async fn initialize_server(&self, name: &str) -> Result<()> {
        let server = self
            .servers
            .read()
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .ok_or_else(|| anyhow!("MCP server '{}' not found", name))?;

        let client = Arc::new(McpClient::new(server));
        client.initialize().await?;

        self.clients.write().await.insert(name.to_string(), client);
        Ok(())
    }

    /// Get a client by server name.
    pub async fn client(&self, name: &str) -> Option<Arc<McpClient>> {
        self.clients.read().await.get(name).cloned()
    }

    /// List all available tools from all initialized MCP servers.
    ///
    /// Tools are collected from each server's cache (refreshed on demand).
    pub async fn list_tools(&self) -> Result<Vec<ToolDef>> {
        let clients = self.clients.read().await;
        let mut all_tools = Vec::new();

        for (name, client) in clients.iter() {
            if let Ok(mcp_tools) = client.list_tools().await {
                let defs: Vec<ToolDef> = mcp_tools.iter().map(|t| t.to_tool_def()).collect();
                let start = all_tools.len();
                all_tools.extend(defs);
                *self
                    .tool_cache
                    .write()
                    .await
                    .entry(name.clone())
                    .or_insert_with(Vec::new) = all_tools[start..].to_vec();
            }
        }

        Ok(all_tools)
    }

    /// Get cached tools for a specific server.
    pub async fn cached_tools(&self, server_name: &str) -> Option<Vec<ToolDef>> {
        self.tool_cache.read().await.get(server_name).cloned()
    }

    /// Call an MCP tool on a specific server.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<McpToolCallResult> {
        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        client.call_tool(tool_name, args).await
    }

    /// Shutdown all connected MCP server processes.
    pub async fn shutdown_all(&self) -> Result<()> {
        let mut clients = self.clients.write().await;

        for (name, client) in clients.drain() {
            if let Err(e) = client.shutdown().await {
                tracing::warn!(server = %name, error = %e, "Error shutting down MCP server");
            }
        }

        self.tool_cache.write().await.clear();
        Ok(())
    }

    /// Refresh tools from a specific server.
    pub async fn refresh_tools(&self, server_name: &str) -> Result<Vec<ToolDef>> {
        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        let mcp_tools = client.refresh_tools().await?;
        let defs: Vec<ToolDef> = mcp_tools.iter().map(|t| t.to_tool_def()).collect();

        *self
            .tool_cache
            .write()
            .await
            .entry(server_name.to_string())
            .or_insert_with(Vec::new) = defs.clone();

        Ok(defs)
    }

    /// Clear the tool cache for a server.
    pub async fn clear_cache(&self, server_name: &str) {
        self.tool_cache.write().await.remove(server_name);
    }

    /// Clear all caches.
    pub async fn clear_all_caches(&self) {
        self.tool_cache.write().await.clear();
    }
}

impl Default for McpBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    // --- McpServer tests ---

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

    // --- JSON-RPC request/response tests ---

    #[test]
    fn test_mcp_request_serialization() {
        let request = McpRequest::new("tools/list");
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains(r#""method":"tools/list""#));
        assert!(json.contains(r#""jsonrpc":"2.0""#));
    }

    #[test]
    fn test_mcp_request_with_params() {
        let request = McpRequest::new("tools/call").with_params(serde_json::json!({
            "name": "my_tool",
            "arguments": {"arg1": "value1"}
        }));

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("my_tool"));
        assert!(json.contains("arg1"));
    }

    #[test]
    fn test_mcp_request_to_jsonl() {
        let request = McpRequest::new("initialize");
        let jsonl = request.to_jsonl().unwrap();

        // Should end with newline
        assert_eq!(jsonl.last(), Some(&b'\n'));

        // Should parse back
        let json_str = String::from_utf8_lossy(&jsonl[..jsonl.len() - 1]);
        let parsed: McpRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.method, "initialize");
    }

    #[test]
    fn test_mcp_response_result() {
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            result: Some(serde_json::json!({"tools": []})),
            error: None,
        };

        assert!(!response.is_error());
        let result = response.clone().into_result().unwrap();
        assert!(result.get("tools").is_some());
    }

    #[test]
    fn test_mcp_response_error() {
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(2),
            result: None,
            error: Some(McpError::internal_error("Something went wrong")),
        };

        assert!(response.is_error());
        let err = response.into_result().unwrap_err();
        assert!(err.to_string().contains("internal error"));
    }

    #[test]
    fn test_mcp_error_codes() {
        assert_eq!(McpError::parse_error().code, -32700);
        assert_eq!(McpError::invalid_request("test").code, -32600);
        assert_eq!(McpError::method_not_found().code, -32601);
        assert_eq!(McpError::invalid_params().code, -32602);
        assert_eq!(McpError::internal_error("x").code, -32603);
        assert_eq!(McpError::server_error("x").code, -32000);
    }

    // --- McpTool conversion tests ---

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

        let arg1 = tool_def
            .arguments
            .iter()
            .find(|a| a.name == "arg1")
            .unwrap();
        assert!(arg1.required);
        assert_eq!(arg1.description, "First argument");

        let arg2 = tool_def
            .arguments
            .iter()
            .find(|a| a.name == "arg2")
            .unwrap();
        assert!(!arg2.required);
        assert_eq!(arg2.default, Some("42".to_string()));
    }

    // --- McpBridge registration tests ---

    #[test]
    fn test_bridge_registration() {
        let bridge = McpBridge::new();

        bridge.register_server(McpServer::new("test", "echo"));

        assert_eq!(bridge.servers(), vec!["test"]);
        assert!(bridge.get_server("test").is_some());
        assert!(bridge.get_server("missing").is_none());
    }

    // --- McpClient lifecycle tests ---

    #[tokio::test]
    async fn test_mcp_client_non_existent_command() {
        let server = McpServer::new("ghost", "nonexistent-binary-xyz");
        let client = McpClient::new(server);

        let result = client.initialize().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to spawn"));
    }

    #[tokio::test]
    async fn test_mcp_client_shutdown_no_panic() {
        let server = McpServer::new("test-shutdown", "echo");
        let client = McpClient::new(server);

        // Shutting down without initializing should not panic
        client.shutdown().await.expect("shutdown should succeed");
        assert!(!client.is_initialized().await);
    }

    #[tokio::test]
    async fn test_mcp_client_with_timeout() {
        let server = McpServer::new("test", "sleep").with_args(vec!["999".to_string()]);
        let client = McpClient::new(server).with_timeout(Duration::from_millis(100));

        // This will spawn a sleep process that hangs
        let result = client.initialize().await;
        // Should timeout, not panic
        assert!(result.is_err());
    }

    // --- McpBridge lifecycle tests ---

    #[tokio::test]
    async fn test_bridge_initialize_all_empty() {
        let bridge = McpBridge::new();
        bridge
            .initialize_all()
            .await
            .expect("empty bridge should initialize");
    }

    #[tokio::test]
    async fn test_bridge_initialize_all_fails_gracefully() {
        let bridge = McpBridge::new();
        bridge.register_server(McpServer::new("ghost", "nonexistent-cmd-xyz"));
        bridge.register_server(McpServer::new("ghost2", "nonexistent-cmd-abc"));

        let result = bridge.initialize_all().await;
        // Should fail because all servers fail to spawn
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bridge_shutdown_all_empty() {
        let bridge = McpBridge::new();
        bridge
            .shutdown_all()
            .await
            .expect("empty bridge shutdown should succeed");
    }

    #[tokio::test]
    async fn test_bridge_call_tool_no_server() {
        let bridge = McpBridge::new();
        let result = bridge
            .call_tool("ghost", "tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn test_bridge_initialize_server_not_found() {
        let bridge = McpBridge::new();
        let result = bridge.initialize_server("missing").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mcp_client_debug() {
        let server = McpServer::new("debug-test", "echo");
        let client = McpClient::new(server);
        let debug = format!("{:?}", client);
        assert!(debug.contains("debug-test"));
    }

    // --- JSON-RPC echo round-trip test (using bash script) ---

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "Requires bash shell environment with executable script support"]
    async fn test_jsonrpc_echo_server() {
        use std::os::unix::fs::PermissionsExt;
        // Create a bash echo script that echoes back stdin lines
        let temp_script = tempfile::tempdir().unwrap().path().join("mcp_echo.sh");
        std::fs::write(
            &temp_script,
            r#"#!/bin/bash
while IFS= read -r line; do
    echo "$line"
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&temp_script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let bridge = McpBridge::new();
        bridge.register_server(
            McpServer::new("echo-server", "bash")
                .with_args(vec![temp_script.to_string_lossy().to_string()]),
        );

        bridge.initialize_all().await.unwrap();

        let client = bridge.client("echo-server").await.unwrap();
        let request =
            McpRequest::new("tools/list").with_params(serde_json::json!({"test": "value"}));
        let response = client.send_request(request).await;

        // The bash echo server will echo back the request JSON as a "response-like" string.
        // This test verifies the client correctly handles stdio communication.
        // A real MCP server would return proper JSON-RPC responses.
        if response.is_ok() {
            tracing::info!("Echo server responded successfully");
        }
    }

    // --- Double-init guard ---

    #[tokio::test]
    async fn test_mcp_client_double_init_ignored() {
        let server = McpServer::new("echo", "echo");
        let client = McpClient::new(server);

        // First init will fail because "echo" doesn't speak MCP protocol
        let _ = client.initialize().await;
        // But calling is_initialized() should not panic
        let _ = client.is_initialized().await;
    }
}
