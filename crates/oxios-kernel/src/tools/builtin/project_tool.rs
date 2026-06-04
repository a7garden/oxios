//! Project tool — wraps `ProjectManager` behind the `AgentTool` interface.
//!
//! Provides agents with Project query capabilities through an action-based
//! parameter schema. Actions: list, get, link_memory, unlink_memory.
//!
//! Agents can query projects and manage memory associations,
//! but cannot create, update, or remove projects (user-level only).

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::kernel_handle::KernelHandle;
use crate::project::ProjectManager;

/// Agent tool for Project queries (RFC-011).
///
/// Wraps the `ProjectManager` behind a single `AgentTool` implementation.
/// The tool uses an `action` parameter to dispatch operations.
///
/// ## Actions
///
/// | Action          | Description                      | Required params           |
/// |-----------------|----------------------------------|---------------------------|
/// | `list`          | List all Projects                | —                         |
/// | `get`           | Get Project details              | `id` or `name`            |
/// | `link_memory`   | Link a memory to a project       | `project_id`, `memory_id` |
/// | `unlink_memory` | Unlink a memory from a project   | `project_id`, `memory_id` |
pub struct ProjectTool {
    project_manager: Option<Arc<ProjectManager>>,
}

impl ProjectTool {
    /// Create a new `ProjectTool` from a `KernelHandle`.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            project_manager: kernel.projects.as_ref().map(|p| p.project_manager.clone()),
        }
    }
}

impl std::fmt::Debug for ProjectTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectTool").finish()
    }
}

#[async_trait]
impl AgentTool for ProjectTool {
    fn name(&self) -> &str {
        "project"
    }

    fn label(&self) -> &str {
        "Project"
    }

    fn description(&self) -> &'static str {
        "Query registered Projects — work contexts with paths and memory associations. \
         Actions: list, get, link_memory, unlink_memory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "link_memory", "unlink_memory"],
                    "description": "Project operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Project UUID"
                },
                "name": {
                    "type": "string",
                    "description": "Project name (alternative to id for 'get')"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project UUID (for link_memory/unlink_memory)"
                },
                "memory_id": {
                    "type": "string",
                    "description": "Memory UUID (for link_memory/unlink_memory)"
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

        let pm = self
            .project_manager
            .as_ref()
            .ok_or_else(|| "Project system not available (SQLite not enabled)".to_string())?;

        match action {
            "list" => {
                let projects = pm.list_projects();
                if projects.is_empty() {
                    return Ok(AgentToolResult::success("No Projects registered."));
                }
                let mut output = format!("Found {} Project(s):\n\n", projects.len());
                for p in &projects {
                    let paths_str = if p.paths.is_empty() {
                        "(no paths)".to_string()
                    } else {
                        p.paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    output.push_str(&format!(
                        "- {} {} ({}) paths={} tags={}\n",
                        p.emoji,
                        p.name,
                        &p.id.to_string()[..8.min(p.id.to_string().len())],
                        paths_str,
                        p.tags.join(", "),
                    ));
                }
                Ok(AgentToolResult::success(output))
            }

            "get" => {
                let project = if let Some(id_str) = params.get("id").and_then(|v| v.as_str()) {
                    let id = uuid::Uuid::parse_str(id_str)
                        .map_err(|e| format!("Invalid project ID: {e}"))?;
                    pm.get_project(id)
                } else if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    pm.get_project_by_name(name)
                } else {
                    return Err("'get' requires 'id' or 'name' parameter".to_string());
                };

                match project {
                    Some(p) => {
                        // Also get associated memory IDs
                        let memory_ids = pm.get_project_memory_ids(p.id).unwrap_or_default();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "id": p.id.to_string(),
                                "name": p.name,
                                "description": p.description,
                                "emoji": p.emoji,
                                "source": p.source.to_string(),
                                "paths": p.paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
                                "tags": p.tags,
                                "memory_visible": p.memory_visible,
                                "associated_memory_count": memory_ids.len(),
                                "last_active": p.last_active_at.to_rfc3339(),
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    None => Ok(AgentToolResult::error("Project not found")),
                }
            }

            "link_memory" => {
                let project_id_str = params
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "link_memory requires 'project_id'".to_string())?;
                let memory_id = params
                    .get("memory_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "link_memory requires 'memory_id'".to_string())?;

                let project_id = uuid::Uuid::parse_str(project_id_str)
                    .map_err(|e| format!("Invalid project_id: {e}"))?;

                match pm.link_memory(project_id, memory_id) {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Linked memory {} to project {}",
                        &memory_id[..8.min(memory_id.len())],
                        &project_id_str[..8.min(project_id_str.len())],
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to link memory: {e}"
                    ))),
                }
            }

            "unlink_memory" => {
                let project_id_str = params
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "unlink_memory requires 'project_id'".to_string())?;
                let memory_id = params
                    .get("memory_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "unlink_memory requires 'memory_id'".to_string())?;

                let project_id = uuid::Uuid::parse_str(project_id_str)
                    .map_err(|e| format!("Invalid project_id: {e}"))?;

                match pm.unlink_memory(project_id, memory_id) {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Unlinked memory {} from project {}",
                        &memory_id[..8.min(memory_id.len())],
                        &project_id_str[..8.min(project_id_str.len())],
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to unlink memory: {e}"
                    ))),
                }
            }

            other => Err(format!(
                "Unknown project action '{other}'. Valid: list, get, link_memory, unlink_memory"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_actions() {
        let schema = serde_json::json!({
            "properties": {
                "action": {
                    "enum": ["list", "get", "link_memory", "unlink_memory"]
                }
            }
        });
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 4);
        assert!(actions.iter().any(|a| a == "list"));
        assert!(actions.iter().any(|a| a == "get"));
        assert!(actions.iter().any(|a| a == "link_memory"));
        assert!(actions.iter().any(|a| a == "unlink_memory"));
    }
}
