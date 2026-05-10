//! Unified execution tool for Oxios agents.
//!
//! Provides two execution modes:
//! - **shell** — Execute a raw command string via `bash -c <cmd>`.
//!   Intended for general-purpose workspace commands (compilation, tests, etc.).
//!
//! - **structured** — Execute a binary with explicit args, subject to allowlist
//!   enforcement and shell-metacharacter blocking.
//!   Intended for host-sensitive operations (git, gh, osascript, open) that
//!   need stricter control.
//!
//! ## Security model
//!
//! `shell` mode: runs through `bash -c` — the command string is passed as-is.
//! Access control is enforced upstream by `AccessManager` (RBAC, path sandboxing).
//!
//! `structured` mode: binary must be in the allowlist (from `ExecConfig`),
//! and all arguments are validated against shell metacharacters (`;`, `|`, `$`,
//! backtick, `<`, `>`, etc.) and path traversal (`..`).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{Mutex, oneshot};

use crate::access_manager::AccessManager;
use crate::config::ExecConfig;

// ─── Shell metacharacter blocklist ──────────

/// Characters that are rejected in structured-mode arguments.
const SHELL_METACHARS: &[char] = &[
    '|', '&', ';', '$', '`', '<', '>', '(', ')', '{', '}', '\n', '\r', '\0',
];

// ─── ExecResult ────────────────────────────────────────────────────────────

/// Result of a command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    /// Standard output captured from the process.
    pub stdout: String,
    /// Standard error captured from the process.
    pub stderr: String,
    /// Process exit code (0 = success, -1 = signal / timeout).
    pub exit_code: i32,
    /// Wall-clock execution duration in milliseconds.
    pub duration_ms: u64,
}

// ─── ExecTool ──────────────────────────────────────────────────────────────

/// Unified execution tool for agents.
///
/// Wraps both shell-string and structured binary+args execution behind a
/// single `AgentTool` implementation that uses a `mode` parameter to
/// dispatch to the appropriate method.
pub struct ExecTool {
    /// Execution configuration (allowlist, timeouts).
    config: Arc<ExecConfig>,
    /// Access manager for permission checks.
    access: Arc<Mutex<AccessManager>>,
}

impl ExecTool {
    /// Create a new `ExecTool` with the given config and access manager.
    pub fn new(config: Arc<ExecConfig>, access: Arc<Mutex<AccessManager>>) -> Self {
        Self { config, access }
    }

    /// Execute a raw command string via `bash -c <cmd>`.
    ///
    /// Primary shell execution path.
    /// The entire command string is forwarded to `bash -c`, so pipelines,
    /// redirects, and compound commands all work.
    pub async fn shell_exec(&self, command: &str, timeout_ms: u64) -> Result<ExecResult, String> {
        if command.trim().is_empty() {
            return Err("shell_exec: command must not be empty".to_string());
        }

        let effective_timeout = timeout_ms.clamp(1_000, self.config.max_timeout_secs * 1_000);

        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(effective_timeout),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .env_clear()
                .env("HOME", std::env::var("HOME").unwrap_or_default())
                .env("USER", std::env::var("USER").unwrap_or_default())
                .env("LOGNAME", std::env::var("LOGNAME").unwrap_or_default())
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env(
                    "LANG",
                    std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string()),
                )
                .env("TERM", "dumb")
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => Ok(ExecResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                duration_ms,
            }),
            Ok(Err(e)) => Err(format!("shell execution error: {e}")),
            Err(_) => Err(format!(
                "shell command timed out after {effective_timeout}ms"
            )),
        }
    }

    /// Execute a binary with explicit args, enforcing allowlist + metachar blocking.
    ///
    /// Primary structured execution path.
    /// Security checks:
    /// 1. Binary must be a bare name (no `/` or `..`).
    /// 2. Binary must be in the allowlist (or allowlist is empty = dev mode).
    /// 3. Arguments must not contain shell metacharacters or path traversal.
    pub async fn structured_exec(
        &self,
        binary: &str,
        args: Vec<String>,
        timeout_ms: u64,
    ) -> Result<ExecResult, String> {
        // --- Binary validation ---

        if binary.contains("..") {
            return Err("structured_exec: path traversal in binary name".to_string());
        }
        if binary.contains('/') {
            return Err("structured_exec: binary must be a bare name, not a path".to_string());
        }
        if !self.config.is_binary_allowed(binary) {
            return Err(format!(
                "structured_exec: binary '{binary}' is not in the allowlist"
            ));
        }

        // --- Argument validation ---

        if has_metacharacters(&args) {
            return Err(
                "structured_exec: shell metacharacters or path traversal not allowed in arguments"
                    .to_string(),
            );
        }

        let effective_timeout = timeout_ms.clamp(1_000, self.config.max_timeout_secs * 1_000);

        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(effective_timeout),
            tokio::process::Command::new(binary)
                .args(&args)
                .env_clear()
                .env("HOME", std::env::var("HOME").unwrap_or_default())
                .env("USER", std::env::var("USER").unwrap_or_default())
                .env("LOGNAME", std::env::var("LOGNAME").unwrap_or_default())
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env(
                    "LANG",
                    std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string()),
                )
                .env("TERM", "dumb")
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => Ok(ExecResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                duration_ms,
            }),
            Ok(Err(e)) => Err(format!("structured execution error: {e}")),
            Err(_) => Err(format!(
                "structured command timed out after {effective_timeout}ms"
            )),
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Check whether any argument contains shell metacharacters or `..`.
fn has_metacharacters(args: &[String]) -> bool {
    for arg in args {
        if arg.contains("..") {
            return true;
        }
        if SHELL_METACHARS.iter().any(|&c| arg.contains(c)) {
            return true;
        }
    }
    false
}

/// Format an `ExecResult` into a human-readable output string (matching the
/// format consistent with agent expectations).
fn format_exec_output(result: &ExecResult) -> String {
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

    let secs = result.duration_ms / 1000;
    let millis = result.duration_ms % 1000;

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

    output
}

// ─── Debug ─────────────────────────────────────────────────────────────────

impl std::fmt::Debug for ExecTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecTool").finish()
    }
}

// ─── AgentTool implementation ──────────────────────────────────────────────

#[async_trait]
impl AgentTool for ExecTool {
    fn name(&self) -> &str {
        "exec"
    }

    fn label(&self) -> &str {
        "Exec"
    }

    fn description(&self) -> &'static str {
        "Execute a command. Use mode='shell' for raw shell strings (pipelines, redirects) or mode='structured' for a specific binary+args with allowlist security."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["shell", "structured"],
                    "description": "Execution mode: 'shell' for bash -c <command>, 'structured' for binary+args with allowlist enforcement"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command string (mode='shell' only)"
                },
                "binary": {
                    "type": "string",
                    "description": "Binary name (mode='structured' only, must be in allowlist)"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Binary arguments (mode='structured' only)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds",
                    "default": 120
                }
            },
            "required": ["mode"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, String> {
        let mode = params
            .get("mode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: mode (expected 'shell' or 'structured')".to_string())?;

        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(self.config.default_timeout_secs);
        let timeout_ms = (timeout_secs * 1000).min(self.config.max_timeout_secs * 1000);

        match mode {
            "shell" => {
                let command = match params.get("command").and_then(|v| v.as_str()) {
                    Some(c) => c,
                    None => return Ok(AgentToolResult::error("shell mode requires 'command' parameter")),
                };

                match self.shell_exec(command, timeout_ms).await {
                    Ok(result) => {
                        let output = format_exec_output(&result);
                        if result.exit_code == 0 {
                            Ok(AgentToolResult::success(output))
                        } else {
                            Ok(AgentToolResult::error(output))
                        }
                    }
                    Err(e) => Ok(AgentToolResult::error(format!("exec (shell): {e}"))),
                }
            }

            "structured" => {
                let binary = match params.get("binary").and_then(|v| v.as_str()) {
                    Some(b) => b,
                    None => return Ok(AgentToolResult::error("structured mode requires 'binary' parameter")),
                };

                let args: Vec<String> = params
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                match self.structured_exec(binary, args, timeout_ms).await {
                    Ok(result) => {
                        let output = format_exec_output(&result);
                        if result.exit_code == 0 {
                            Ok(AgentToolResult::success(output))
                        } else {
                            Ok(AgentToolResult::error(output))
                        }
                    }
                    Err(e) => Ok(AgentToolResult::error(format!("exec (structured): {e}"))),
                }
            }

            other => Err(format!(
                "Invalid mode '{other}': expected 'shell' or 'structured'"
            )),
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build an `ExecTool` with default config and empty access manager.
    fn make_tool(allowed_commands: Vec<&str>) -> ExecTool {
        let mut config = ExecConfig::default();
        config.allowed_commands = allowed_commands.into_iter().map(String::from).collect();
        ExecTool::new(Arc::new(config), Arc::new(Mutex::new(AccessManager::new())))
    }

    fn make_tool_with_config(config: ExecConfig) -> ExecTool {
        ExecTool::new(Arc::new(config), Arc::new(Mutex::new(AccessManager::new())))
    }

    // ─── shell_exec ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_shell_exec_echo() {
        let tool = make_tool(vec![]);
        let result = tool.shell_exec("echo hello", 5_000).await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
        assert!(r.duration_ms < 5_000);
    }

    #[tokio::test]
    async fn test_shell_exec_pipeline() {
        let tool = make_tool(vec![]);
        let result = tool.shell_exec("echo foo | tr f b", 5_000).await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("boo"));
    }

    #[tokio::test]
    async fn test_shell_exec_nonzero_exit() {
        let tool = make_tool(vec![]);
        let result = tool.shell_exec("exit 42", 5_000).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().exit_code, 42);
    }

    #[tokio::test]
    async fn test_shell_exec_empty_command() {
        let tool = make_tool(vec![]);
        let result = tool.shell_exec("   ", 5_000).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not be empty"));
    }

    #[tokio::test]
    async fn test_shell_exec_timeout() {
        let tool = make_tool(vec![]);
        let result = tool.shell_exec("sleep 300", 1_000).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    // ─── structured_exec ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_structured_exec_echo() {
        let tool = make_tool(vec!["echo"]);
        let result = tool
            .structured_exec("echo", vec!["hello".into()], 5_000)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_structured_exec_blocked_binary() {
        let tool = make_tool(vec!["echo"]);
        let result = tool
            .structured_exec("rm", vec!["-rf".into(), "/".into()], 5_000)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the allowlist"));
    }

    #[tokio::test]
    async fn test_structured_exec_path_binary() {
        let tool = make_tool(vec![]);
        let result = tool
            .structured_exec("/usr/bin/echo", vec![], 5_000)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bare name"));
    }

    #[tokio::test]
    async fn test_structured_exec_traversal_binary() {
        let tool = make_tool(vec![]);
        let result = tool
            .structured_exec("../bin/evil", vec![], 5_000)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path traversal"));
    }

    #[tokio::test]
    async fn test_structured_exec_metachar_args() {
        let tool = make_tool(vec!["echo"]);
        let result = tool
            .structured_exec("echo", vec!["foo; rm -rf /".into()], 5_000)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("metacharacters"));
    }

    #[tokio::test]
    async fn test_structured_exec_path_traversal_args() {
        let tool = make_tool(vec!["cat"]);
        let result = tool
            .structured_exec("cat", vec!["../etc/passwd".into()], 5_000)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("metacharacters"));
    }

    #[tokio::test]
    async fn test_structured_exec_clean_args() {
        let tool = make_tool(vec!["echo"]);
        let result = tool
            .structured_exec("echo", vec!["hello".into(), "world".into()], 5_000)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello world"));
    }

    // ─── AgentTool interface ─────────────────────────────────────────

    #[test]
    fn test_name_and_label() {
        let tool = make_tool(vec![]);
        assert_eq!(tool.name(), "exec");
        assert_eq!(tool.label(), "Exec");
    }

    #[test]
    fn test_parameters_schema() {
        let tool = make_tool(vec![]);
        let schema = tool.parameters_schema();

        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("mode"));
        assert!(props.contains_key("command"));
        assert!(props.contains_key("binary"));
        assert!(props.contains_key("args"));
        assert!(props.contains_key("timeout"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r.as_str() == Some("mode")));
    }

    #[tokio::test]
    async fn test_agent_tool_shell_mode() {
        let tool = make_tool(vec![]);

        let result = tool
            .execute(
                "test-1",
                json!({ "mode": "shell", "command": "echo hello" }),
                None,
            )
            .await;

        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success, "Expected success, got: {}", r.output);
        assert!(r.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_agent_tool_structured_mode() {
        let tool = make_tool(vec!["echo"]);

        let result = tool
            .execute(
                "test-2",
                json!({ "mode": "structured", "binary": "echo", "args": ["hi"] }),
                None,
            )
            .await;

        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success, "Expected success, got: {}", r.output);
        assert!(r.output.contains("hi"));
    }

    #[tokio::test]
    async fn test_agent_tool_missing_mode() {
        let tool = make_tool(vec![]);
        let result = tool.execute("test-3", json!({ "command": "echo hi" }), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter: mode"));
    }

    #[tokio::test]
    async fn test_agent_tool_invalid_mode() {
        let tool = make_tool(vec![]);
        let result = tool
            .execute("test-4", json!({ "mode": "docker" }), None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid mode"));
    }

    #[tokio::test]
    async fn test_agent_tool_shell_missing_command() {
        let tool = make_tool(vec![]);
        let result = tool
            .execute("test-5", json!({ "mode": "shell" }), None)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.output.contains("shell mode requires 'command' parameter"));
    }

    #[tokio::test]
    async fn test_agent_tool_structured_missing_binary() {
        let tool = make_tool(vec![]);
        let result = tool
            .execute("test-6", json!({ "mode": "structured" }), None)
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.output.contains("structured mode requires 'binary' parameter"));
    }

    #[tokio::test]
    async fn test_agent_tool_nonzero_exit() {
        let tool = make_tool(vec![]);

        let result = tool
            .execute(
                "test-7",
                json!({ "mode": "shell", "command": "exit 7" }),
                None,
            )
            .await;

        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.output.contains("exited with code 7"));
    }

    // ─── format_exec_output ──────────────────────────────────────────

    #[test]
    fn test_format_exec_output_success() {
        let result = ExecResult {
            stdout: "hello".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 1_500,
        };
        let output = format_exec_output(&result);
        assert!(output.contains("hello"));
        assert!(output.contains("Took 1.5s"));
        assert!(!output.contains("exited with code"));
    }

    #[test]
    fn test_format_exec_output_failure() {
        let result = ExecResult {
            stdout: String::new(),
            stderr: "error!".to_string(),
            exit_code: 1,
            duration_ms: 500,
        };
        let output = format_exec_output(&result);
        assert!(output.contains("error!"));
        assert!(output.contains("exited with code 1"));
    }

    #[test]
    fn test_format_exec_output_no_output() {
        let result = ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 100,
        };
        let output = format_exec_output(&result);
        assert!(output.contains("(no output)"));
    }

    #[test]
    fn test_format_exec_output_minutes() {
        let result = ExecResult {
            stdout: "done".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 125_000, // 2m 5s
        };
        let output = format_exec_output(&result);
        assert!(output.contains("Took 2m 5.0s"));
    }

    // ─── has_metacharacters ──────────────────────────────────────────

    #[test]
    fn test_has_metacharacters_clean() {
        assert!(!has_metacharacters(&["hello".into(), "world".into()]));
    }

    #[test]
    fn test_has_metacharacters_semicolon() {
        assert!(has_metacharacters(&["foo;bar".into()]));
    }

    #[test]
    fn test_has_metacharacters_pipe() {
        assert!(has_metacharacters(&["a | b".into()]));
    }

    #[test]
    fn test_has_metacharacters_dollar() {
        assert!(has_metacharacters(&["$(whoami)".into()]));
    }

    #[test]
    fn test_has_metacharacters_backtick() {
        assert!(has_metacharacters(&["`id`".into()]));
    }

    #[test]
    fn test_has_metacharacters_traversal() {
        assert!(has_metacharacters(&["../etc/passwd".into()]));
    }
}
