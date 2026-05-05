//! Container execution tool — replaces oxi's BashTool.
//!
//! Executes commands inside the container if active, otherwise locally via BashTool.
//! This is the primary workspace command execution tool for Oxios agents.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult, BashTool, ToolError};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::container_manager::ContainerManager;

/// Execute commands in the workspace.
///
/// When a container is active, runs commands inside it via ContainerBackend.
/// When no container is active, delegates to oxi's BashTool for local `sh -c` execution.
pub struct ContainerExecTool {
    /// oxi BashTool for local fallback
    bash: BashTool,
    /// Container manager. None = always local.
    container: Option<Arc<ContainerManager>>,
}

impl ContainerExecTool {
    pub fn new(container: Option<Arc<ContainerManager>>) -> Self {
        Self {
            bash: BashTool::new(),
            container,
        }
    }
}

impl std::fmt::Debug for ContainerExecTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerExecTool")
            .field("has_container", &self.container.is_some())
            .finish()
    }
}

#[async_trait]
impl AgentTool for ContainerExecTool {
    fn name(&self) -> &str {
        "container_exec"
    }

    fn label(&self) -> &str {
        "Container Exec"
    }

    fn description(&self) -> &str {
        "Execute a command in the workspace. Runs inside the container if active, otherwise locally. Use for compilation, tests, package management, and any workspace command."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds",
                    "default": 120
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory"
                },
                "env": {
                    "type": "object",
                    "description": "Environment variables as key-value pairs",
                    "additionalProperties": { "type": "string" }
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        tool_call_id: &str,
        params: Value,
        signal: Option<oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, ToolError> {
        // Check if we have an active container
        if let Some(cm) = &self.container {
            if let Some(name) = cm.active_container_name().await {
                return self.exec_in_container(&name, cm, &params).await;
            }
        }

        // No container — delegate to oxi's BashTool
        self.bash.execute(tool_call_id, params, signal).await
    }
}

impl ContainerExecTool {
    /// Execute a command inside the container via ContainerBackend.
    async fn exec_in_container(
        &self,
        container_name: &str,
        cm: &ContainerManager,
        params: &Value,
    ) -> Result<AgentToolResult, ToolError> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: command".to_string())?;

        let cmd_parts: Vec<String> = vec![
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ];

        let start = std::time::Instant::now();

        let result = cm
            .exec_in_container(container_name, &cmd_parts, None)
            .await
            .map_err(|e| format!("Container exec failed: {}", e))?;

        let elapsed = start.elapsed();

        // Format output consistent with BashTool
        let mut output = String::new();

        if result.stdout.is_empty() && result.stderr.is_empty() {
            output.push_str("(no output)");
        } else {
            if !result.stdout.is_empty() {
                output.push_str(&result.stdout);
            }
            if !result.stderr.is_empty() && !result.stdout.is_empty() {
                output.push('\n');
            }
            if !result.stderr.is_empty() {
                output.push_str(&result.stderr);
            }
        }

        if result.exit_code != 0 {
            output.push_str(&format!(
                "\n\nCommand exited with code {}",
                result.exit_code
            ));
        }

        // Append timing (matching BashTool format)
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();
        if secs >= 60 {
            let mins = secs / 60;
            let remain_secs = secs % 60;
            output.push_str(&format!(
                "\n\nTook {}m {:.1}s",
                mins,
                remain_secs as f64 + millis as f64 / 1000.0
            ));
        } else {
            output.push_str(&format!(
                "\n\nTook {:.1}s",
                secs as f64 + millis as f64 / 1000.0
            ));
        }

        if result.exit_code == 0 {
            Ok(AgentToolResult::success(output))
        } else {
            Ok(AgentToolResult::error(output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_and_schema() {
        let tool = ContainerExecTool::new(None);
        assert_eq!(tool.name(), "container_exec");
        assert_eq!(tool.label(), "Container Exec");
        let schema = tool.parameters_schema();
        assert_eq!(schema["properties"]["command"]["type"], "string");
        assert_eq!(schema["required"][0], "command");
    }

    #[test]
    fn test_no_container_uses_local() {
        let tool = ContainerExecTool::new(None);
        assert!(tool.container.is_none());
    }

    #[tokio::test]
    async fn test_local_execution_via_bash() {
        let tool = ContainerExecTool::new(None);
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({ "command": "echo hello_from_container_exec" }),
                None,
            )
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("hello_from_container_exec"));
    }

    #[tokio::test]
    async fn test_local_execution_failure() {
        let tool = ContainerExecTool::new(None);
        let result = tool
            .execute(
                "test-2",
                serde_json::json!({ "command": "exit 42" }),
                None,
            )
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.output.contains("42"));
    }

    #[tokio::test]
    async fn test_missing_command_param() {
        let tool = ContainerExecTool::new(None);
        let result = tool.execute("test-3", serde_json::json!({}), None).await;
        assert!(result.is_err());
    }
}
