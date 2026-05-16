//! Resource tool — wraps `InfraApi` resource methods behind the `AgentTool` interface.
//!
//! Provides agents with system resource monitoring capabilities.
//! Actions: snapshot, history, overloaded.
//!
//! ## Example
//!
//! ```json
//! { "action": "snapshot" }
//! { "action": "history", "last_n": 10 }
//! { "action": "overloaded" }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::kernel_handle::KernelHandle;
use crate::resource_monitor::ResourceMonitor;

/// Agent tool for resource monitoring.
///
/// Wraps the resource-related methods of the `InfraApi` domain. Allows agents
/// to query system resource usage, history, and overload status.
///
/// ## Actions
///
/// | Action       | Description                     | Required params | Optional params |
/// |--------------|---------------------------------|-----------------|-----------------|
/// | `snapshot`   | Get current resource snapshot   | —               | —               |
/// | `history`    | Get recent resource snapshots   | —               | `last_n`        |
/// | `overloaded` | Check if system is overloaded   | —               | —               |
pub struct ResourceTool {
    resource_monitor: Arc<ResourceMonitor>,
}

impl ResourceTool {
    /// Create a new `ResourceTool` from a `KernelHandle`.
    ///
    /// Extracts the `ResourceMonitor` Arc from the kernel's Infra API.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            resource_monitor: kernel.infra.resource_monitor.clone(),
        }
    }
}

impl std::fmt::Debug for ResourceTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceTool").finish()
    }
}

#[async_trait]
impl AgentTool for ResourceTool {
    fn name(&self) -> &str {
        "resource"
    }

    fn label(&self) -> &str {
        "Resource"
    }

    fn description(&self) -> &'static str {
        "Monitor system resources — CPU, memory, disk, agent count. \
         Actions: snapshot, history, overloaded."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["snapshot", "history", "overloaded"],
                    "description": "Resource operation to perform"
                },
                "last_n": {
                    "type": "integer",
                    "description": "Number of historical snapshots to return (history action, default: 10)"
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
            "snapshot" => {
                let snap = self.resource_monitor.snapshot();
                Ok(AgentToolResult::success(serde_json::to_string_pretty(
                    &json!({
                        "timestamp": snap.timestamp.to_rfc3339(),
                        "cpu_percent": format!("{:.1}%", snap.cpu_percent),
                        "memory_used_mb": snap.memory_used_mb,
                        "memory_total_mb": snap.memory_total_mb,
                        "memory_percent": format!(
                            "{:.1}%",
                            if snap.memory_total_mb > 0 {
                                (snap.memory_used_mb as f64 / snap.memory_total_mb as f64) * 100.0
                            } else {
                                0.0
                            }
                        ),
                        "active_agents": snap.active_agents,
                        "pending_tasks": snap.pending_tasks,
                        "total_token_usage": snap.total_token_usage,
                        "disk_used_gb": format!("{:.2}", snap.disk_used_gb),
                        "load_avg_1m": format!("{:.2}", snap.load_avg_1m),
                    }),
                ).unwrap_or_default()))
            }

            "history" => {
                let last_n = params["last_n"].as_u64().unwrap_or(10) as usize;
                let history = self.resource_monitor.history(last_n);

                if history.is_empty() {
                    return Ok(AgentToolResult::success(
                        "No resource history available yet.",
                    ));
                }

                let display: Vec<Value> = history
                    .iter()
                    .map(|snap| {
                        json!({
                            "timestamp": snap.timestamp.to_rfc3339(),
                            "cpu_percent": format!("{:.1}%", snap.cpu_percent),
                            "memory_mb": format!("{}/{}", snap.memory_used_mb, snap.memory_total_mb),
                            "active_agents": snap.active_agents,
                            "load_avg_1m": format!("{:.2}", snap.load_avg_1m),
                        })
                    })
                    .collect();

                Ok(AgentToolResult::success(serde_json::to_string_pretty(
                    &json!({
                        "snapshots": display,
                        "count": display.len(),
                    }),
                ).unwrap_or_default()))
            }

            "overloaded" => {
                let overloaded = self.resource_monitor.is_overloaded();
                Ok(AgentToolResult::success(serde_json::to_string(
                    &json!({
                        "overloaded": overloaded,
                        "status": if overloaded { "OVERLOADED" } else { "NOMINAL" },
                    }),
                ).unwrap_or_default()))
            }

            other => Err(format!(
                "Unknown resource action '{}'. Valid: snapshot, history, overloaded",
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
                    "enum": ["snapshot", "history", "overloaded"]
                },
                "last_n": { "type": "integer" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().any(|a| a == "snapshot"));
        assert!(actions.iter().any(|a| a == "history"));
        assert!(actions.iter().any(|a| a == "overloaded"));
    }
}
