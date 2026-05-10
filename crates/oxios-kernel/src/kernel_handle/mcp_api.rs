//! MCP API — external tool server bridge.

use std::sync::Arc;
use crate::mcp::{McpBridge, McpServer, McpToolCallResult};
use crate::program::ToolDef;

/// MCP system calls.
pub struct McpApi {
    pub(crate) mcp_bridge: Arc<McpBridge>,
}

impl McpApi {
    /// Create a new McpApi.
    pub fn new(mcp_bridge: Arc<McpBridge>) -> Self {
        Self { mcp_bridge }
    }
    /// List registered MCP servers.
    pub fn list_servers(&self) -> Vec<String> {
        self.mcp_bridge.servers()
    }

    /// Get MCP server info.
    pub fn get_server(&self, name: &str) -> Option<McpServer> {
        self.mcp_bridge.get_server(name)
    }

    /// Register an MCP server.
    pub fn register_server(&self, server: McpServer) {
        self.mcp_bridge.register_server(server);
    }

    /// Initialize a specific MCP server.
    pub async fn init_server(&self, name: &str) -> anyhow::Result<()> {
        self.mcp_bridge.initialize_server(name).await
    }

    /// Get MCP client status.
    pub async fn client_status(&self, name: &str) -> Option<bool> {
        if let Some(client) = self.mcp_bridge.client(name).await {
            Some(client.is_initialized().await)
        } else {
            None
        }
    }

    /// List all MCP tools.
    pub async fn list_tools(&self) -> anyhow::Result<Vec<ToolDef>> {
        self.mcp_bridge.list_tools().await
    }

    /// Get cached tools for a server.
    pub async fn cached_tools(&self, server_name: &str) -> Option<Vec<ToolDef>> {
        self.mcp_bridge.cached_tools(server_name).await
    }

    /// Call an MCP tool.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolCallResult> {
        self.mcp_bridge.call_tool(server, tool, arguments).await
    }
}
