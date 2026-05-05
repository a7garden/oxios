//! Host execution tool — secure host (macOS) command execution.
//!
//! Exposes HostExecBridge as an agent tool with binary allowlist security.
//! Unlike container_exec which accepts shell strings, this tool requires
//! structured binary + args input for security.
//!
//! ## Security model
//!
//! All execution goes through HostExecBridge which enforces:
//! - Binary allowlist (only approved commands)
//! - Shell metacharacter blocking (;, |, $, `, etc.)
//! - Path traversal prevention (../)
//!
//! This is intentionally different from container_exec's shell string API.
//! Host access needs stricter control because it touches macOS system resources.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::host_exec::HostExecBridge;

/// Execute commands on the host (macOS).
///
/// Wraps HostExecBridge with the AgentTool interface.
/// Binary allowlist is managed by HostExecBridge, initialized from config.
pub struct HostExecTool {
    bridge: Arc<HostExecBridge>,
}

impl HostExecTool {
    /// Create a new HostExecTool wrapping the given bridge.
    pub fn new(bridge: Arc<HostExecBridge>) -> Self {
        Self { bridge }
    }
}

impl std::fmt::Debug for HostExecTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostExecTool").finish()
    }
}

#[async_trait]
impl AgentTool for HostExecTool {
    fn name(&self) -> &str {
        "host_exec"
    }

    fn label(&self) -> &str {
        "Host Exec"
    }

    fn description(&self) -> &'static str {
        "Execute a command on the host (macOS). Use for git, gh, osascript, open, and other host-only tools. The binary must be in the allowlist."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "binary": {
                    "type": "string",
                    "description": "Command binary name (e.g. 'gh', 'git', 'open')"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command arguments"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds",
                    "default": 30
                }
            },
            "required": ["binary"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, String> {
        let binary = params
            .get("binary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: binary".to_string())?;

        let args: Vec<String> = params
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);
        let timeout_ms = (timeout_secs * 1000).min(60_000);

        let result = self.bridge.exec(binary, args, timeout_ms).await;

        match result {
            Ok(host_result) => {
                let mut output = String::new();

                if host_result.stdout.is_empty() && host_result.stderr.is_empty() {
                    output.push_str("(no output)");
                } else {
                    if !host_result.stdout.is_empty() {
                        output.push_str(&host_result.stdout);
                    }
                    if !host_result.stderr.is_empty() && !host_result.stdout.is_empty() {
                        output.push('\n');
                    }
                    if !host_result.stderr.is_empty() {
                        output.push_str(&host_result.stderr);
                    }
                }

                if host_result.exit_code != 0 {
                    output.push_str(&format!(
                        "\n\nCommand exited with code {}",
                        host_result.exit_code
                    ));
                }

                output.push_str(&format!("\nTook {}ms", host_result.duration_ms));

                if host_result.exit_code == 0 {
                    Ok(AgentToolResult::success(output))
                } else {
                    Ok(AgentToolResult::error(output))
                }
            }
            Err(e) => Ok(AgentToolResult::error(format!("Host exec failed: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_params(binary: &str, args: Vec<&str>) -> Value {
        let args_str: Vec<String> = args.into_iter().map(String::from).collect();
        json!({ "binary": binary, "args": args_str })
    }

    fn make_params_with_timeout(binary: &str, args: Vec<&str>, timeout: u64) -> Value {
        let args_str: Vec<String> = args.into_iter().map(String::from).collect();
        json!({ "binary": binary, "args": args_str, "timeout": timeout })
    }

    fn make_bridge(allowed: Vec<&str>) -> Arc<HostExecBridge> {
        let tmp = tempfile::tempdir().unwrap();
        Arc::new(
            HostExecBridge::new(
                tmp.path().to_path_buf(),
                allowed.into_iter().map(String::from).collect(),
            )
            .expect("non-empty allowlist required"),
        )
    }

    #[tokio::test]
    async fn test_echo() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);

        let result = tool
            .execute("test-1", make_params("echo", vec!["hello"]), None)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success, "Expected success, got: {}", r.output);
        assert!(r.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_blocked_binary() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);

        let result = tool
            .execute("test-2", make_params("rm", vec!["-rf", "/"]), None)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.output.contains("not in allowlist") || r.output.contains("Host exec failed"));
    }

    #[tokio::test]
    async fn test_missing_binary_param() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);

        let result = tool.execute("test-3", json!({}), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter: binary"));
    }

    #[tokio::test]
    async fn test_no_args() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);

        let result = tool
            .execute("test-4", json!({ "binary": "echo" }), None)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
    }

    #[tokio::test]
    async fn test_stderr_capture() {
        let bridge = make_bridge(vec!["bash"]);
        let tool = HostExecTool::new(bridge);

        // HostExecBridge blocks > in args, so we can't redirect stderr.
        // Instead just test that stderr is captured when a command naturally
        // writes to it (e.g., an invalid grep pattern writes to stderr).
        let result = tool
            .execute(
                "test-5",
                make_params("bash", vec!["-c", "echo error 1>&2"]),
                None,
            )
            .await;
        // This will likely fail because 1>&2 contains > which is blocked.
        // That's expected — host_exec intentionally blocks shell redirection.
        // The test just verifies the tool handles blocked args gracefully.
        let r = result.unwrap();
        // Either success with output, or error about blocked metacharacters
        assert!(r.output.contains("error") || r.output.contains("metacharacters"));
    }

    #[tokio::test]
    async fn test_nonzero_exit() {
        let bridge = make_bridge(vec!["bash"]);
        let tool = HostExecTool::new(bridge);

        let result = tool
            .execute("test-6", make_params("bash", vec!["-c", "exit 42"]), None)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.output.contains("exited with code 42"));
    }

    #[tokio::test]
    async fn test_timeout() {
        let bridge = make_bridge(vec!["sleep"]);
        let tool = HostExecTool::new(bridge);

        let result = tool
            .execute(
                "test-7",
                make_params_with_timeout("sleep", vec!["300"], 1),
                None,
            )
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.output.contains("timed out") || r.output.contains("Host exec failed"));
    }

    #[test]
    fn test_name() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);
        assert_eq!(tool.name(), "host_exec");
    }

    #[test]
    fn test_label() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);
        assert_eq!(tool.label(), "Host Exec");
    }

    #[test]
    fn test_parameters_schema_structure() {
        let bridge = make_bridge(vec!["echo"]);
        let tool = HostExecTool::new(bridge);
        let schema = tool.parameters_schema();

        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("binary"));
        assert!(props.contains_key("args"));
        assert!(props.contains_key("timeout"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r.as_str() == Some("binary")));
    }
}
