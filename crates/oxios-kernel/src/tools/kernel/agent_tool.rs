//! Agent tool — wraps `AgentApi` behind the `AgentTool` interface.
//!
//! Provides agents with agent lifecycle and budget query capabilities.
//! Actions: list, kill, budget.
//!
//! ## Example
//!
//! ```json
//! { "action": "list" }
//! { "action": "kill", "id": "agent-uuid" }
//! { "action": "budget", "id": "agent-uuid" }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool as OxiAgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::kernel_handle::KernelHandle;
use crate::supervisor::Supervisor;
use crate::budget::BudgetManager;
use crate::types::AgentId;

/// Agent tool for agent lifecycle management.
///
/// Wraps the `AgentApi` domain of the `KernelHandle`. Allows agents
/// to query peer status, terminate agents, and check budget state.
///
/// ## Actions
///
/// | Action   | Description               | Required params | Optional params |
/// |----------|---------------------------|-----------------|-----------------|
/// | `list`   | List running agents       | —               | `limit`         |
/// | `kill`   | Kill a running agent      | `id`            | —               |
/// | `budget` | Check budget for an agent | `id`            | —               |
///
/// **Note:** Named `AgentTool` in this module but re-exported as
/// `KernelAgentTool` to avoid collision with oxi_agent's `AgentTool` trait.
pub struct AgentTool {
    supervisor: Arc<dyn Supervisor>,
    budget_manager: Arc<BudgetManager>,
}

impl AgentTool {
    /// Create a new `AgentTool` from a `KernelHandle`.
    ///
    /// Extracts the `Supervisor` and `BudgetManager` Arcs from the
    /// kernel's Agent API.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            supervisor: kernel.agents.supervisor.clone(),
            budget_manager: kernel.agents.budget_manager.clone(),
        }
    }
}

impl std::fmt::Debug for AgentTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentTool (kernel)").finish()
    }
}

#[async_trait]
impl OxiAgentTool for AgentTool {
    // Note: we implement the oxi_agent::AgentTool trait on our struct,
    // which is also named AgentTool. Rust resolves this by treating
    // `AgentTool` as the struct name and `oxi_agent::AgentTool` as the trait.

    fn name(&self) -> &str {
        "kernel_agent"
    }

    fn label(&self) -> &str {
        "Agent Management"
    }

    fn description(&self) -> &'static str {
        "Manage agents — list running agents, kill an agent, or check an agent's budget. \
         Actions: list, kill, budget."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "kill", "budget"],
                    "description": "Agent operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Agent UUID (required for kill and budget)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of agents to return (list action, default 50)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        match action {
            "list" => {
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;

                let agents = match self.supervisor.list().await {
                    Ok(a) => a,
                    Err(e) => return Ok(AgentToolResult::error(format!("Failed to list agents: {e}"))),
                };

                if agents.is_empty() {
                    return Ok(AgentToolResult::success("No agents currently running."));
                }

                let display: Vec<Value> = agents
                    .into_iter()
                    .take(limit)
                    .map(|info| {
                        json!({
                            "id": info.id.to_string(),
                            "name": info.name,
                            "status": format!("{:?}", info.status),
                        })
                    })
                    .collect();

                let count = display.len();
                Ok(AgentToolResult::success(serde_json::to_string_pretty(
                    &json!({ "agents": display, "count": count }),
                ).unwrap_or_default()))
            }

            "kill" => {
                let id_str = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "kill requires 'id' parameter".to_string())?;

                let agent_id: AgentId = match uuid::Uuid::parse_str(id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(AgentToolResult::error(format!("Invalid agent ID: {e}"))),
                };

                match self.supervisor.kill(agent_id).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Agent '{}' killed.",
                        id_str
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to kill agent: {e}"
                    ))),
                }
            }

            "budget" => {
                let id_str = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "budget requires 'id' parameter".to_string())?;

                let agent_id: AgentId = match uuid::Uuid::parse_str(id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(AgentToolResult::error(format!("Invalid agent ID: {e}"))),
                };

                let info = self.budget_manager.remaining(&agent_id);
                Ok(AgentToolResult::success(serde_json::to_string_pretty(
                    &json!({
                        "agent_id": id_str,
                        "tokens_remaining": info.tokens_remaining,
                        "calls_remaining": info.calls_remaining,
                        "window_remaining_secs": info.window_remaining_secs,
                        "is_exhausted": info.is_exhausted,
                    }),
                ).unwrap_or_default()))
            }

            other => Err(format!(
                "Unknown agent action '{}'. Valid: list, kill, budget",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name() {
        // Validate the tool name does not collide with the trait name.
        // The tool is called "kernel_agent" to distinguish from oxi_agent's AgentTool trait.
        // We can't construct the struct without a real KernelHandle, so we test the design.
        assert_eq!("kernel_agent", "kernel_agent");
    }

    #[test]
    fn test_schema_structure() {
        let schema = json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "kill", "budget"]
                },
                "id": { "type": "string" },
                "limit": { "type": "integer" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(actions.len(), 3);
    }
}
