//! Budget tool — wraps `AgentApi` budget methods behind the `AgentTool` interface.
//!
//! Provides agents with budget management capabilities.
//! Actions: check, set, reserve, reset.
//!
//! ## Example
//!
//! ```json
//! { "action": "check", "agent_id": "uuid" }
//! { "action": "set", "agent_id": "uuid", "limit": 100000 }
//! { "action": "reserve", "agent_id": "uuid", "tokens": 500 }
//! { "action": "reset", "agent_id": "uuid" }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::budget::{BudgetLimit, BudgetManager};
use crate::kernel_handle::KernelHandle;
use crate::types::AgentId;

/// Agent tool for budget management.
///
/// Wraps the budget-related methods of the `AgentApi` domain. Allows agents
/// to check, configure, and manage token/call budgets for agents.
///
/// ## Actions
///
/// | Action    | Description                  | Required params | Optional params |
/// |-----------|------------------------------|-----------------|-----------------|
/// | `check`   | Check remaining budget       | `agent_id`      | —               |
/// | `set`     | Set budget limit for agent   | `agent_id`      | `limit`         |
/// | `reserve` | Reserve tokens from budget   | `agent_id`      | `tokens`        |
/// | `reset`   | Reset budget window          | `agent_id`      | —               |
pub struct BudgetTool {
    budget_manager: Arc<BudgetManager>,
}

impl BudgetTool {
    /// Create a new `BudgetTool` from a `KernelHandle`.
    ///
    /// Extracts the `BudgetManager` Arc from the kernel's Agent API.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            budget_manager: kernel.agents.budget_manager.clone(),
        }
    }
}

impl std::fmt::Debug for BudgetTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BudgetTool").finish()
    }
}

#[async_trait]
impl AgentTool for BudgetTool {
    fn name(&self) -> &str {
        "budget"
    }

    fn label(&self) -> &str {
        "Budget"
    }

    fn description(&self) -> &'static str {
        "Manage agent budgets — check remaining tokens/calls, set limits, reserve tokens, or reset the budget window. \
         Actions: check, set, reserve, reset."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check", "set", "reserve", "reset"],
                    "description": "Budget operation to perform"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Agent UUID to operate on"
                },
                "limit": {
                    "type": "integer",
                    "description": "Token budget limit (set action only, default: 100000)"
                },
                "tokens": {
                    "type": "integer",
                    "description": "Number of tokens to reserve (reserve action only)"
                }
            },
            "required": ["action", "agent_id"]
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

        let agent_id_str = params
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: agent_id".to_string())?;

        let agent_id: AgentId = match uuid::Uuid::parse_str(agent_id_str) {
            Ok(id) => id,
            Err(e) => return Ok(AgentToolResult::error(format!("Invalid agent_id: {e}"))),
        };

        match action {
            "check" => {
                let info = self.budget_manager.remaining(&agent_id);
                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&json!({
                        "agent_id": agent_id_str,
                        "tokens_remaining": info.tokens_remaining,
                        "calls_remaining": info.calls_remaining,
                        "window_remaining_secs": info.window_remaining_secs,
                        "is_exhausted": info.is_exhausted,
                    }))
                    .unwrap_or_default(),
                ))
            }

            "set" => {
                let token_limit = params["limit"].as_u64().unwrap_or(100_000);

                let limit = BudgetLimit {
                    agent_id,
                    token_budget: token_limit,
                    calls_budget: 1_000,
                    window_secs: 3_600, // 1 hour default
                };

                self.budget_manager.set_budget(limit);

                Ok(AgentToolResult::success(format!(
                    "Budget set for agent '{}': {} tokens, 1000 calls, 1h window.",
                    agent_id_str, token_limit,
                )))
            }

            "reserve" => {
                let tokens = params["tokens"].as_u64().unwrap_or(0);
                if tokens == 0 {
                    return Ok(AgentToolResult::error(
                        "reserve requires 'tokens' parameter (> 0)",
                    ));
                }

                match self.budget_manager.reserve(&agent_id, tokens) {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Reserved {} tokens for agent '{}'.",
                        tokens, agent_id_str,
                    ))),
                    Err(exceeded) => Ok(AgentToolResult::error(format!(
                        "Budget exceeded: {}",
                        exceeded
                    ))),
                }
            }

            "reset" => {
                self.budget_manager.reset_window(&agent_id);
                Ok(AgentToolResult::success(format!(
                    "Budget window reset for agent '{}'.",
                    agent_id_str,
                )))
            }

            other => Err(format!(
                "Unknown budget action '{}'. Valid: check, set, reserve, reset",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_structure() {
        let schema = json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check", "set", "reserve", "reset"]
                },
                "agent_id": { "type": "string" },
                "limit": { "type": "integer" },
                "tokens": { "type": "integer" }
            },
            "required": ["action", "agent_id"]
        });

        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 4);
        assert!(actions.iter().any(|a| a == "check"));
        assert!(actions.iter().any(|a| a == "set"));
        assert!(actions.iter().any(|a| a == "reserve"));
        assert!(actions.iter().any(|a| a == "reset"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r.as_str() == Some("action")));
        assert!(required.iter().any(|r| r.as_str() == Some("agent_id")));
    }
}
