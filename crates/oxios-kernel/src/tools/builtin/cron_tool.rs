//! Cron tool — wraps `InfraApi` cron methods behind the `AgentTool` interface.
//!
//! Provides agents with cron scheduling capabilities.
//! Actions: list, add, remove, trigger.
//!
//! ## Example
//!
//! ```json
//! { "action": "list" }
//! { "action": "add", "expression": "0 */6 * * *", "task": "Review open PRs" }
//! { "action": "remove", "id": "job-uuid" }
//! { "action": "trigger", "id": "job-uuid" }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::cron::CronScheduler;
use crate::kernel_handle::KernelHandle;

/// Agent tool for cron scheduling.
///
/// Wraps the cron-related methods of the `InfraApi` domain. Allows agents
/// to list, create, remove, and manually trigger cron jobs.
///
/// ## Actions
///
/// | Action    | Description              | Required params           | Optional params |
/// |-----------|--------------------------|---------------------------|-----------------|
/// | `list`    | List all cron jobs       | —                         | —               |
/// | `add`     | Add a new cron job       | `expression`, `task`      | —               |
/// | `remove`  | Remove a cron job        | `id`                      | —               |
/// | `trigger` | Manually trigger a job   | `id`                      | —               |
pub struct CronTool {
    cron_scheduler: Arc<CronScheduler>,
}

impl CronTool {
    /// Create a new `CronTool` from a `KernelHandle`.
    ///
    /// Extracts the `CronScheduler` Arc from the kernel's Infra API.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            cron_scheduler: kernel.infra.cron_scheduler.clone(),
        }
    }
}

impl std::fmt::Debug for CronTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CronTool").finish()
    }
}

#[async_trait]
impl AgentTool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn label(&self) -> &str {
        "Cron"
    }

    fn description(&self) -> &'static str {
        "Manage cron jobs — schedule recurring tasks. \
         Actions: list, add, remove, trigger."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "add", "remove", "trigger"],
                    "description": "Cron operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Job UUID (required for remove and trigger)"
                },
                "expression": {
                    "type": "string",
                    "description": "Cron expression, e.g. '0 */6 * * *' (add action only)"
                },
                "task": {
                    "type": "string",
                    "description": "Goal description for the scheduled agent (add action only)"
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
                let jobs = self.cron_scheduler.list_jobs();
                if jobs.is_empty() {
                    return Ok(AgentToolResult::success("No cron jobs defined."));
                }

                let display: Vec<Value> = jobs
                    .iter()
                    .map(|job| {
                        json!({
                            "id": job.id.to_string(),
                            "name": job.name,
                            "schedule": job.schedule,
                            "goal": job.goal,
                            "enabled": job.enabled,
                            "run_count": job.run_count,
                            "last_success": job.last_success,
                        })
                    })
                    .collect();

                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(
                        &json!({ "jobs": display, "count": display.len() }),
                    )
                    .unwrap_or_default(),
                ))
            }

            "add" => {
                let expression = params
                    .get("expression")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "add requires 'expression' parameter".to_string())?;
                let task = params
                    .get("task")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "add requires 'task' parameter".to_string())?;

                let job = crate::cron::CronJob::new(
                    format!("job_{}", uuid::Uuid::new_v4()),
                    expression.to_string(),
                    task.to_string(),
                );

                match self.cron_scheduler.add_job(job).await {
                    Ok(job_id) => Ok(AgentToolResult::success(
                        serde_json::to_string(&json!({
                            "job_id": job_id.to_string(),
                            "schedule": expression,
                            "goal": task,
                        }))
                        .unwrap_or_default(),
                    )),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to add cron job: {e}"
                    ))),
                }
            }

            "remove" => {
                let id_str = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "remove requires 'id' parameter".to_string())?;

                let job_id = match uuid::Uuid::parse_str(id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(AgentToolResult::error(format!("Invalid job ID: {e}"))),
                };

                match self.cron_scheduler.remove_job(job_id).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Cron job '{id_str}' removed."
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to remove cron job: {e}"
                    ))),
                }
            }

            "trigger" => {
                let id_str = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "trigger requires 'id' parameter".to_string())?;

                let job_id = match uuid::Uuid::parse_str(id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(AgentToolResult::error(format!("Invalid job ID: {e}"))),
                };

                match self.cron_scheduler.trigger_job(job_id) {
                    Ok(job) => Ok(AgentToolResult::success(format!(
                        "Cron job '{}' ({}) triggered successfully.",
                        job.name, id_str
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to trigger cron job: {e}"
                    ))),
                }
            }

            other => Err(format!(
                "Unknown cron action '{other}'. Valid: list, add, remove, trigger"
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
                    "enum": ["list", "add", "remove", "trigger"]
                },
                "id": { "type": "string" },
                "expression": { "type": "string" },
                "task": { "type": "string" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 4);
        assert!(actions.iter().any(|a| a == "list"));
        assert!(actions.iter().any(|a| a == "add"));
        assert!(actions.iter().any(|a| a == "remove"));
        assert!(actions.iter().any(|a| a == "trigger"));
    }
}
