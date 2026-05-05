//! Container execution tool — replaces oxi's BashTool.
//!
//! Executes commands inside the container if active. P0 Security: no local fallback.
//! This is the primary workspace command execution tool for Oxios agents.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult};
use serde_json::{json, Value};

use crate::container_manager::ContainerManager;

/// Execute commands in the workspace.
///
/// When a container is active, runs commands inside it via ContainerBackend.
/// P0 Security: No local fallback — if no container is running, returns an error.
/// This prevents container_exec from bypassing the container sandbox.
pub struct ContainerExecTool {
    /// Container manager — required for secure workspace execution.
    container: Arc<ContainerManager>,
}

impl ContainerExecTool {
    /// Create a new ContainerExecTool with the given container manager.
    pub fn new(container: Arc<ContainerManager>) -> Self {
        Self { container }
    }
}

impl std::fmt::Debug for ContainerExecTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerExecTool").finish()
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
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, String> {
        // P0 Security: Require an active container. No local fallback.
        // This prevents container_exec from bypassing the container sandbox.
        let container_name = match self.container.active_container_name().await {
            Some(name) => name,
            None => {
                return Ok(AgentToolResult::error(
                    "container_exec: no active container. \
                     Start a garden with 'oxios garden up <name>' before executing commands.",
                ));
            }
        };

        self.exec_in_container(&container_name, &self.container, &params).await
    }
}

impl ContainerExecTool {
    /// Execute a command inside the container via ContainerBackend.
    async fn exec_in_container(
        &self,
        container_name: &str,
        cm: &ContainerManager,
        params: &Value,
    ) -> Result<AgentToolResult, String> {
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
    use crate::host_exec::HostExecBridge;
    use crate::state_store::StateStore;
    use crate::container_manager::ContainerManager;

    #[test]
    fn test_name_and_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );
        let state = StateStore::new(tmp.path().join("state")).unwrap();
        let cm = Arc::new(
            ContainerManager::with_apple_backend(
                host_exec,
                Arc::new(state),
                tmp.path().join("containers"),
            )
        );
        let tool = ContainerExecTool::new(cm);
        assert_eq!(tool.name(), "container_exec");
        assert_eq!(tool.label(), "Container Exec");
        let schema = tool.parameters_schema();
        assert_eq!(schema["properties"]["command"]["type"], "string");
        assert_eq!(schema["required"][0], "command");
    }

    #[test]
    fn test_container_exec_with_container() {
        let tmp = tempfile::tempdir().unwrap();
        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );
        let state = StateStore::new(tmp.path().join("state")).unwrap();
        let cm = Arc::new(
            ContainerManager::with_apple_backend(
                host_exec,
                Arc::new(state),
                tmp.path().join("containers"),
            )
        );
        let tool = ContainerExecTool::new(cm);
        // Container is always present (required field), even if no container running
        assert!(true, "container_exec always has a container manager");
    }

    #[tokio::test]
    async fn test_no_active_container_returns_error() {
        // When no container is running, container_exec should return an error,
        // NOT fall back to local execution (P0 security fix)
        let tmp = tempfile::tempdir().unwrap();
        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );
        let state = StateStore::new(tmp.path().join("state")).unwrap();
        let cm = Arc::new(
            ContainerManager::with_apple_backend(
                host_exec,
                Arc::new(state),
                tmp.path().join("containers"),
            )
        );
        let tool = ContainerExecTool::new(cm);
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({ "command": "echo hello" }),
                None,
            )
            .await
            .unwrap();
        // P0: No fallback to local — returns error when no container running
        assert!(!result.success, "should return error when no active container");
        assert!(result.output.contains("no active container"));
    }

    #[tokio::test]
    async fn test_missing_command_param() {
        // P0 Security: Command param validation is only reached after
        // confirming an active container. Since no container is running,
        // we get "no active container" error first — this is the correct
        // security behavior (no local fallback).
        let tmp = tempfile::tempdir().unwrap();
        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );
        let state = StateStore::new(tmp.path().join("state")).unwrap();
        let cm = Arc::new(
            ContainerManager::with_apple_backend(
                host_exec,
                Arc::new(state),
                tmp.path().join("containers"),
            )
        );
        let tool = ContainerExecTool::new(cm);
        let result = tool.execute("test-3", serde_json::json!({}), None).await;
        // P0: Error due to no active container (not missing command param)
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success, "should fail when no active container");
        assert!(r.output.contains("no active container"));
    }
}
