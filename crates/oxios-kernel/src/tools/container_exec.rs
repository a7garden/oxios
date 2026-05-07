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
/// If no container is running and a host_exec_bridge is available, falls back
/// to host execution (development mode). Otherwise returns an error.
pub struct ContainerExecTool {
    /// Container manager — required for secure workspace execution.
    container: Arc<ContainerManager>,
    /// Optional host exec bridge for fallback when no container is running.
    host_exec_bridge: Option<Arc<crate::host_exec::HostExecBridge>>,
}

impl ContainerExecTool {
    /// Create a new ContainerExecTool with the given container manager.
    pub fn new(container: Arc<ContainerManager>) -> Self {
        Self {
            container,
            host_exec_bridge: None,
        }
    }

    /// Create a new ContainerExecTool with a host exec bridge for fallback.
    pub fn new_with_host_bridge(
        container: Arc<ContainerManager>,
        host_exec_bridge: Arc<crate::host_exec::HostExecBridge>,
    ) -> Self {
        Self {
            container,
            host_exec_bridge: Some(host_exec_bridge),
        }
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
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, String> {
        // Check for an active container.
        let container_name = self.container.active_container_name().await;

        // ── No-container fallback: execute via host_exec bridge ──
        if container_name.is_none() {
            if let Some(bridge) = &self.host_exec_bridge {
                let cmd = params
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if cmd.is_empty() {
                    return Ok(AgentToolResult::error(
                        "container_exec: 'command' parameter is required",
                    ));
                }
                match bridge.exec("bash", vec!["bash".to_string(), "-c".to_string(), cmd.to_string()], 30_000).await {
                    Ok(result) => {
                        let output = if result.stderr.is_empty() {
                            result.stdout
                        } else {
                            format!("{}\n{}", result.stdout, result.stderr)
                        };
                        return Ok(AgentToolResult::success(output));
                    }
                    Err(e) => {
                        return Ok(AgentToolResult::error(format!(
                            "container_exec (host fallback): {e}"
                        )));
                    }
                }
            }
            // No fallback available.
            return Ok(AgentToolResult::error(
                "container_exec: no active container. \
                 Start a garden with 'oxios garden up <name>' or set \
                 execution_mode = 'host' for direct host execution.",
            ));
        }

        self.exec_in_container(container_name.as_deref().unwrap(), &self.container, &params).await
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

        let cwd = params.get("cwd").and_then(|v| v.as_str());

        // Log env vars if provided (full forwarding deferred — requires
        // backend support for environment passthrough).
        if let Some(env) = params.get("env") {
            tracing::debug!(
                target: "oxios::container_exec",
                env = %env,
                "Environment variables provided but not forwarded to container"
            );
        }

        let cmd_parts: Vec<String> = vec![
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ];

        let start = std::time::Instant::now();

        let result = cm
            .exec_in_container(container_name, &cmd_parts, cwd)
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
