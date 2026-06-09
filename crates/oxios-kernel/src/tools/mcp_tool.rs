//! MCP tool wrapper — exposes MCP server tools as AgentTool implementations.
//!
//! MCP tools from external servers (via the Model Context Protocol) are wrapped
//! to implement the `AgentTool` trait. The full tool name is namespaced as
//! `mcp:{server_name}:{tool_name}` to avoid collisions with Tier 1-2 tools.

use async_trait::async_trait;
use std::sync::Arc;

use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::Value;

use crate::mcp::{McpBridge, McpContentBlock};

/// Wraps an MCP tool from a specific server as an `AgentTool`.
pub struct McpToolWrapper {
    /// The bridge — used to route tool calls to the correct server.
    bridge: Arc<McpBridge>,
    /// Full tool name `mcp:{server}:{tool}`.
    full_name: String,
    /// Name of the MCP server (used for routing).
    server_name: String,
    /// Tool name within the server.
    tool_name: String,
    /// Human-readable description from the MCP server.
    description: String,
    /// JSON Schema for the tool's input parameters.
    input_schema: Value,
}

impl McpToolWrapper {
    /// Create a new MCP tool wrapper.
    pub fn new(
        bridge: Arc<McpBridge>,
        server_name: &str,
        tool_name: &str,
        description: String,
        input_schema: Value,
    ) -> Self {
        let full_name = format!("mcp:{server_name}:{tool_name}");
        Self {
            bridge,
            full_name,
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            description,
            input_schema,
        }
    }

    /// Create an `McpToolWrapper` from a [`KernelHandle`] for a specific MCP tool.
    ///
    /// Extracts the MCP bridge from the kernel's MCP facade.
    pub fn from_kernel(
        kernel: &crate::kernel_handle::KernelHandle,
        server_name: &str,
        tool_name: &str,
        description: String,
        input_schema: Value,
    ) -> Self {
        Self::new(
            kernel.mcp.bridge().clone(),
            server_name,
            tool_name,
            description,
            input_schema,
        )
    }
}

impl std::fmt::Debug for McpToolWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpToolWrapper")
            .field("full_name", &self.full_name)
            .finish()
    }
}

/// Format an MCP content block for display.
fn format_content_block(block: &McpContentBlock) -> String {
    match block {
        McpContentBlock::Text { text } => text.clone(),
        McpContentBlock::Image { data, mime_type } => {
            format!(
                "[Image ({}): {} bytes]",
                mime_type.as_deref().unwrap_or("?"),
                data.len()
            )
        }
        McpContentBlock::Resource { resource } => {
            format!("[Resource: {}]", resource.uri)
        }
    }
}

#[async_trait]

impl AgentTool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn label(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError>
     {
        match self
            .bridge
            .call_tool(&self.server_name, &self.tool_name, params)
            .await
        {
            Ok(result) => {
                let output = if result.content.is_empty() {
                    "(no output)".to_string()
                } else {
                    result
                        .content
                        .iter()
                        .map(format_content_block)
                        .collect::<Vec<_>>()
                        .join("\n")
                };

                let is_error = result.is_error.unwrap_or(false);
                if is_error {
                    Ok(AgentToolResult::error(output))
                } else {
                    Ok(AgentToolResult::success(output))
                }
            }
            Err(e) => {
                tracing::error!(
                    server = %self.server_name,
                    tool = %self.tool_name,
                    error = %e,
                    "MCP tool call failed"
                );
                Ok(AgentToolResult::error(format!(
                    "MCP tool '{}/{}' failed: {}",
                    self.server_name, self.tool_name, e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_wrapper_debug() {
        let wrapper = McpToolWrapper::new(
            Arc::new(McpBridge::new()),
            "test-server",
            "test_tool",
            "A test tool".to_string(),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "arg": {
                        "type": "string",
                        "description": "An argument"
                    }
                }
            }),
        );
        let debug = format!("{:?}", wrapper);
        assert!(debug.contains("test-server"));
        assert!(debug.contains("test_tool"));
    }

    #[test]
    fn test_name_format() {
        let wrapper = McpToolWrapper::new(
            Arc::new(McpBridge::new()),
            "github",
            "create_pr",
            "Create a PR".to_string(),
            serde_json::json!({"type": "object", "properties": {}}),
        );
        assert_eq!(wrapper.name(), "mcp:github:create_pr");
        assert_eq!(wrapper.label(), "create_pr");
        assert_eq!(wrapper.description(), "Create a PR");
    }
}
