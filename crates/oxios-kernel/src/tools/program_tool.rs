//! Program-defined tool with automatic host/container routing.
//!
//! Each `[[tools]]` entry in `program.toml` becomes a `ProgramTool` registered
//! in the ToolRegistry at Tier 3. When executed, the tool routes to `host_exec`
//! for command execution. Since Oxios runs inside a container, all execution
//! goes through the host_exec bridge which provides the execution environment.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::host_exec_tool::HostExecTool;
use crate::config::ContainerConfig;
use crate::program::{ProgramHostRequirements, ToolDef};

/// A tool defined by a Program, with automatic execution routing.
///
/// All program tools now route to `host_exec` since Oxios runs inside
/// a container and `host_exec` provides the execution bridge.
pub struct ProgramTool {
    /// Full namespaced name: `"program:{program_name}:{tool_name}"`
    full_name: String,
    /// Tool description for the LLM (stored as owned String for `&str` return)
    description: String,
    /// Binary to execute (first word of command)
    binary: String,
    /// Default arguments from the command definition
    default_args: Vec<String>,
    /// Execution delegates — actual execution is delegated to Tier 2 tools
    host_exec: Arc<HostExecTool>,
}

impl ProgramTool {
    /// Create a ProgramTool from a program's tool definition.
    ///
    /// All program tools now route to `host_exec` since Oxios runs inside
    /// a container and `host_exec` provides the execution bridge.
    pub fn from_definition(
        program_name: &str,
        tool_def: &ToolDef,
        _host_requirements: &ProgramHostRequirements,
        _container_config: &ContainerConfig,
        host_exec: Arc<HostExecTool>,
    ) -> Self {
        // Parse command: first word is binary, rest are default args
        let parts: Vec<&str> = tool_def.command.split_whitespace().collect();
        let binary = parts.first().unwrap_or(&"").to_string();
        let default_args = parts.iter().skip(1).map(|s| s.to_string()).collect();

        Self {
            full_name: format!("program:{}:{}", program_name, tool_def.name),
            description: tool_def.description.clone(),
            binary,
            default_args,
            host_exec,
        }
    }
}

impl std::fmt::Debug for ProgramTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramTool")
            .field("full_name", &self.full_name)
            .field("binary", &self.binary)
            .finish()
    }
}

#[async_trait]
impl AgentTool for ProgramTool {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn label(&self) -> &str {
        &self.full_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Additional arguments to pass to the command"
                }
            }
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        signal: Option<oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, String> {
        // Extract user-provided args
        let user_args: Vec<String> = params
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Build full command: binary + default_args + user_args
        let all_args: Vec<String> = self
            .default_args
            .iter()
            .chain(user_args.iter())
            .cloned()
            .collect();

        // Route to host_exec
        let host_params = json!({
            "binary": self.binary,
            "args": all_args,
        });
        self.host_exec
            .execute(&format!("pg:{}", self.full_name), host_params, signal)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that ProgramTool correctly parses command definitions.
    #[test]
    fn test_command_parsing() {
        let tool_def = ToolDef {
            name: "create_pr".to_string(),
            description: "Create a PR".to_string(),
            arguments: vec![],
            command: "gh pr create".to_string(),
        };
        let host_reqs = ProgramHostRequirements::default();
        let config = ContainerConfig::default();

        // Create a minimal HostExecTool for testing
        let host_exec_bridge = Arc::new(
            crate::host_exec::HostExecBridge::new(std::env::temp_dir(), vec!["gh".to_string()])
                .expect("non-empty allowlist required"),
        );
        let host_exec = Arc::new(HostExecTool::new(host_exec_bridge));

        let tool = ProgramTool::from_definition(
            "github",
            &tool_def,
            &host_reqs,
            &config,
            host_exec,
        );

        assert_eq!(tool.full_name, "program:github:create_pr");
        assert_eq!(tool.binary, "gh");
        assert_eq!(tool.default_args, vec!["pr", "create"]);
    }

    /// Verify that single-word commands work correctly.
    #[test]
    fn test_single_word_command() {
        let tool_def = ToolDef {
            name: "status".to_string(),
            description: "Show git status".to_string(),
            arguments: vec![],
            command: "git".to_string(),
        };
        let host_reqs = ProgramHostRequirements::default();
        let config = ContainerConfig::default();

        let host_exec_bridge = Arc::new(
            crate::host_exec::HostExecBridge::new(std::env::temp_dir(), vec!["git".to_string()])
                .expect("non-empty allowlist required"),
        );
        let host_exec = Arc::new(HostExecTool::new(host_exec_bridge));

        let tool = ProgramTool::from_definition(
            "git-tools",
            &tool_def,
            &host_reqs,
            &config,
            host_exec,
        );

        assert_eq!(tool.full_name, "program:git-tools:status");
        assert_eq!(tool.binary, "git");
        assert!(tool.default_args.is_empty());
    }

    /// Verify that commands with flags work correctly.
    #[test]
    fn test_command_with_flags() {
        let tool_def = ToolDef {
            name: "fetch".to_string(),
            description: "Fetch from remote".to_string(),
            arguments: vec![],
            command: "git fetch --all --prune".to_string(),
        };
        let host_reqs = ProgramHostRequirements::default();
        let config = ContainerConfig::default();

        let host_exec_bridge = Arc::new(
            crate::host_exec::HostExecBridge::new(std::env::temp_dir(), vec!["git".to_string()])
                .expect("non-empty allowlist required"),
        );
        let host_exec = Arc::new(HostExecTool::new(host_exec_bridge));

        let tool = ProgramTool::from_definition(
            "git-tools",
            &tool_def,
            &host_reqs,
            &config,
            host_exec,
        );

        assert_eq!(tool.binary, "git");
        assert_eq!(tool.default_args, vec!["fetch", "--all", "--prune"]);
    }
}