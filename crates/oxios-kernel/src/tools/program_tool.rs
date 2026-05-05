//! Program-defined tool with automatic host/container routing.
//!
//! Each `[[tools]]` entry in `program.toml` becomes a `ProgramTool` registered
//! in the ToolRegistry at Tier 3. When executed, the tool automatically routes
//! to `host_exec` (if the binary is in host requirements) or `container_exec`
//! (otherwise), delegating the actual execution to the Tier 2 tools.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult, ToolError};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::container_exec::ContainerExecTool;
use super::host_exec_tool::HostExecTool;
use crate::config::ContainerConfig;
use crate::program::{ProgramHostRequirements, ToolDef};

/// A tool defined by a Program, with automatic execution routing.
///
/// Routes to `host_exec` if the command binary is listed in the program's
/// `host_requirements` or the global `ContainerConfig` host tools.
/// Otherwise routes to `container_exec` (which falls back to local `sh -c`
/// when no container is active).
pub struct ProgramTool {
    /// Full namespaced name: `"program:{program_name}:{tool_name}"`
    full_name: String,
    /// Tool description for the LLM (stored as owned String for `&str` return)
    description: String,
    /// Binary to execute (first word of command)
    binary: String,
    /// Default arguments from the command definition
    default_args: Vec<String>,
    /// Whether to route to host_exec
    runs_on_host: bool,
    /// Execution delegates — actual execution is delegated to Tier 2 tools
    container_exec: Arc<ContainerExecTool>,
    host_exec: Arc<HostExecTool>,
}

impl ProgramTool {
    /// Create a ProgramTool from a program's tool definition.
    ///
    /// The execution location is determined by checking if the command binary
    /// appears in either:
    /// - The program's `host_requirements` (required + optional)
    /// - The global `ContainerConfig` host tools (required_host_tools + optional_host_tools)
    pub fn from_definition(
        program_name: &str,
        tool_def: &ToolDef,
        host_requirements: &ProgramHostRequirements,
        container_config: &ContainerConfig,
        container_exec: Arc<ContainerExecTool>,
        host_exec: Arc<HostExecTool>,
    ) -> Self {
        // Parse command: first word is binary, rest are default args
        let parts: Vec<&str> = tool_def.command.split_whitespace().collect();
        let binary = parts.first().unwrap_or(&"").to_string();
        let default_args = parts.iter().skip(1).map(|s| s.to_string()).collect();

        // Determine execution location
        let is_host_tool = |name: &str| -> bool {
            host_requirements
                .required
                .iter()
                .chain(host_requirements.optional.iter())
                .chain(container_config.required_host_tools.iter())
                .chain(container_config.optional_host_tools.iter())
                .any(|t| t == name)
        };

        let runs_on_host = is_host_tool(&binary);

        Self {
            full_name: format!("program:{}:{}", program_name, tool_def.name),
            description: tool_def.description.clone(),
            binary,
            default_args,
            runs_on_host,
            container_exec,
            host_exec,
        }
    }
}

impl std::fmt::Debug for ProgramTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramTool")
            .field("full_name", &self.full_name)
            .field("binary", &self.binary)
            .field("runs_on_host", &self.runs_on_host)
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
    ) -> Result<AgentToolResult, ToolError> {
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

        if self.runs_on_host {
            // Host exec route: binary + default_args + user_args
            let all_args: Vec<String> = self
                .default_args
                .iter()
                .chain(user_args.iter())
                .cloned()
                .collect();
            let host_params = json!({
                "binary": self.binary,
                "args": all_args,
            });
            self.host_exec
                .execute(&format!("pg:{}", self.full_name), host_params, signal)
                .await
        } else {
            // Container exec route: full command string
            let full_cmd = {
                let mut parts: Vec<String> = std::iter::once(self.binary.clone())
                    .chain(self.default_args.iter().cloned())
                    .chain(user_args)
                    .collect();
                parts.join(" ")
            };
            let container_params = json!({
                "command": full_cmd,
            });
            self.container_exec
                .execute(&format!("pg:{}", self.full_name), container_params, signal)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that ProgramTool correctly determines host routing based on
    /// program-level host_requirements.
    #[test]
    fn test_host_routing_from_program_requirements() {
        // Simulate a tool where "gh" is in host_requirements.required
        let tool_def = ToolDef {
            name: "create_pr".to_string(),
            description: "Create a PR".to_string(),
            arguments: vec![],
            command: "gh pr create".to_string(),
        };
        let host_reqs = ProgramHostRequirements {
            required: vec!["gh".to_string()],
            optional: vec![],
        };
        let config = ContainerConfig::default();

        // We need real Tier 2 tools for Arc — use minimal stubs
        let container_exec = Arc::new(ContainerExecTool::new(None));
        let host_bridge = Arc::new(
            crate::host_exec::HostExecBridge::new(std::env::temp_dir(), vec!["gh".to_string()]),
        );
        let host_exec = Arc::new(HostExecTool::new(host_bridge));

        let tool = ProgramTool::from_definition(
            "github",
            &tool_def,
            &host_reqs,
            &config,
            container_exec,
            host_exec,
        );

        assert_eq!(tool.full_name, "program:github:create_pr");
        assert_eq!(tool.binary, "gh");
        assert_eq!(tool.default_args, vec!["pr", "create"]);
        assert!(tool.runs_on_host);
    }

    /// Verify that a tool without host requirements routes to container.
    #[test]
    fn test_container_routing_when_not_host_tool() {
        let tool_def = ToolDef {
            name: "parse".to_string(),
            description: "Parse JSON".to_string(),
            arguments: vec![],
            command: "jq".to_string(),
        };
        let host_reqs = ProgramHostRequirements {
            required: vec![],
            optional: vec![],
        };
        let config = ContainerConfig::default();

        let container_exec = Arc::new(ContainerExecTool::new(None));
        let host_bridge = Arc::new(
            crate::host_exec::HostExecBridge::new(std::env::temp_dir(), vec![]),
        );
        let host_exec = Arc::new(HostExecTool::new(host_bridge));

        let tool = ProgramTool::from_definition(
            "jq",
            &tool_def,
            &host_reqs,
            &config,
            container_exec,
            host_exec,
        );

        assert_eq!(tool.full_name, "program:jq:parse");
        assert_eq!(tool.binary, "jq");
        assert!(tool.default_args.is_empty());
        assert!(!tool.runs_on_host);
    }

    /// Verify global config host tools are considered in routing.
    #[test]
    fn test_global_host_tools_routing() {
        let tool_def = ToolDef {
            name: "log".to_string(),
            description: "Show git log".to_string(),
            arguments: vec![],
            command: "git log".to_string(),
        };
        // Program doesn't declare git as host requirement,
        // but global config has it in required_host_tools
        let host_reqs = ProgramHostRequirements {
            required: vec![],
            optional: vec![],
        };
        let mut config = ContainerConfig::default();
        config.required_host_tools = vec!["git".to_string()];

        let container_exec = Arc::new(ContainerExecTool::new(None));
        let host_bridge = Arc::new(
            crate::host_exec::HostExecBridge::new(std::env::temp_dir(), vec!["git".to_string()]),
        );
        let host_exec = Arc::new(HostExecTool::new(host_bridge));

        let tool = ProgramTool::from_definition(
            "git-tools",
            &tool_def,
            &host_reqs,
            &config,
            container_exec,
            host_exec,
        );

        assert!(tool.runs_on_host, "git should route to host via global config");
    }
}
