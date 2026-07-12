//! MCP integration — adapters between `oxios-mcp` and the kernel.
//!
//! Re-exports all types from the `oxios-mcp` crate and provides conversion
//! functions between MCP-native types and Oxios kernel types (`ToolDef`).

pub use oxios_mcp::{
    BLOCKED_MCP_SHELLS,
    ClientInfo,
    InitializeParams,
    InitializeResult,
    MappedResource,
    McpBridge,
    McpCapabilities,
    McpClient,
    McpContentBlock,
    McpError,
    McpRequest,
    McpResponse,
    McpServer,
    McpServerConfig,
    McpTool,
    McpToolCallResult,
    McpToolsResult,
    ServerInfo,
    // Spawn-validation chokepoint (audit F-1): command blocklist + env sanitize.
    sanitize_env,
    validate_mcp_command,
};

use crate::tools::tool_types::{ArgumentDef, ToolDef};

/// Convert an MCP tool to an Oxios `ToolDef`.
///
/// Parses the `input_schema` as a JSON Schema object, extracting
/// properties from the top-level `"properties"` key.
pub fn mcp_tool_to_tool_def(tool: &McpTool) -> ToolDef {
    let arguments = if let Some(properties) = tool
        .input_schema()
        .get("properties")
        .and_then(|p| p.as_object())
    {
        let required_list: Vec<&str> = tool
            .input_schema()
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        properties
            .iter()
            .map(|(name, schema)| {
                let description = schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("No description")
                    .to_string();
                let required =
                    required_list.iter().any(|r| *r == name) && schema.get("default").is_none();

                ArgumentDef {
                    name: name.clone(),
                    description,
                    required,
                    default: schema
                        .get("default")
                        .and_then(|d| d.as_str().map(String::from)),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    ToolDef {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        arguments,
        command: String::new(),
    }
}

/// List all MCP tools as Oxios `ToolDef`s (existing API compatibility).
pub async fn list_tool_defs(bridge: &McpBridge) -> anyhow::Result<Vec<ToolDef>> {
    let tools = bridge.list_tools().await?;
    Ok(tools.iter().map(mcp_tool_to_tool_def).collect())
}

/// Get cached MCP tools as Oxios `ToolDef`s for a specific server.
pub async fn cached_tool_defs(bridge: &McpBridge, server_name: &str) -> Option<Vec<ToolDef>> {
    bridge
        .cached_tools(server_name)
        .await
        .map(|tools| tools.iter().map(mcp_tool_to_tool_def).collect())
}

/// Refresh and return MCP tools as Oxios `ToolDef`s for a specific server.
pub async fn refresh_tool_defs(
    bridge: &McpBridge,
    server_name: &str,
) -> anyhow::Result<Vec<ToolDef>> {
    let tools = bridge.refresh_tools(server_name).await?;
    Ok(tools.iter().map(mcp_tool_to_tool_def).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_to_tool_def() {
        let mcp_tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg1": {
                        "type": "string",
                        "description": "First argument"
                    },
                    "arg2": {
                        "type": "number",
                        "description": "Second argument",
                        "default": "42"
                    }
                },
                "required": ["arg1"]
            }),
        };

        let tool_def = mcp_tool_to_tool_def(&mcp_tool);

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

    #[test]
    fn test_mcp_tool_to_tool_def_no_properties() {
        let mcp_tool = McpTool {
            name: "simple".to_string(),
            description: "No args".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };

        let tool_def = mcp_tool_to_tool_def(&mcp_tool);
        assert!(tool_def.arguments.is_empty());
        assert_eq!(tool_def.command, "");
    }
}
