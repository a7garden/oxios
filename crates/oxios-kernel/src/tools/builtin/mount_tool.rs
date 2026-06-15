//! Mount tool — wraps `MountManager` behind the `AgentTool` interface (RFC-025).
//!
//! Provides agents with Mount query and enrichment capabilities. The agent
//! explores a Mount's filesystem and writes its findings via the `update`
//! action — this is the agent-driven enrichment path.
//!
//! Actions: `list`, `get`, `update` (enrichment).

use async_trait::async_trait;
use std::sync::Arc;

use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{Value, json};

use crate::kernel_handle::KernelHandle;
use crate::mount::{MountId, MountManager};

/// Agent tool for Mount queries + agent-driven enrichment (RFC-025).
///
/// Agents can query Mounts and refine their `auto_description`/`auto_meta`
/// via the `update` action, but cannot create or remove Mounts (user-level).
pub struct MountTool {
    mount_manager: Option<Arc<MountManager>>,
}

impl MountTool {
    /// Create from a `KernelHandle`.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            mount_manager: kernel.mounts.as_ref().map(|m| m.mount_manager.clone()),
        }
    }
}

impl std::fmt::Debug for MountTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountTool").finish()
    }
}

#[async_trait]
impl AgentTool for MountTool {
    fn name(&self) -> &str {
        "mount"
    }

    fn label(&self) -> &str {
        "Mount"
    }

    fn description(&self) -> &'static str {
        "Query and enrich Mounts (path aliases). The agent explores a Mount's \
         filesystem and writes its findings to auto_description/auto_meta via \
         'update'. Actions: list, get, update."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "update"],
                    "description": "Mount operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Mount UUID"
                },
                "name": {
                    "type": "string",
                    "description": "Mount name (alternative to id for 'get')"
                },
                "auto_description": {
                    "type": "string",
                    "description": "(update) Agent-written description, ≤500 chars. Cite real files you read."
                },
                "auto_meta": {
                    "type": "object",
                    "description": "(update) Auto-detected metadata to set",
                    "properties": {
                        "languages": { "type": "array", "items": { "type": "string" } },
                        "stack": { "type": "array", "items": { "type": "string" } },
                        "markers": { "type": "array", "items": { "type": "string" } },
                        "summary": { "type": "string" }
                    }
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        let mm = self
            .mount_manager
            .as_ref()
            .ok_or_else(|| "Mount system not available (SQLite not enabled)".to_string())?;

        match action {
            "list" => {
                let mounts = mm.list_mounts();
                if mounts.is_empty() {
                    return Ok(AgentToolResult::success("No Mounts registered."));
                }
                let mut out = format!("Found {} Mount(s):\n\n", mounts.len());
                for m in &mounts {
                    let paths = m
                        .paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let langs = m.auto_meta.languages.join(", ");
                    out.push_str(&format!(
                        "- {} ({}) [{}]\n  paths: {}\n  summary: {}\n",
                        m.name,
                        &m.id.to_string()[..8],
                        if langs.is_empty() { "unknown" } else { &langs },
                        paths,
                        m.summary_line(),
                    ));
                }
                Ok(AgentToolResult::success(out))
            }

            "get" => {
                let mount = if let Some(id_str) = params.get("id").and_then(|v| v.as_str()) {
                    let id =
                        MountId::parse_str(id_str).map_err(|e| format!("Invalid mount ID: {e}"))?;
                    mm.get_mount(id)
                } else if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    mm.get_mount_by_name(name)
                } else {
                    return Err("'get' requires 'id' or 'name' parameter".to_string());
                };

                match mount {
                    Some(m) => Ok(AgentToolResult::success(
                        serde_json::to_string_pretty(&json!({
                            "id": m.id.to_string(),
                            "name": m.name,
                            "source": m.source.to_string(),
                            "paths": m.paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
                            "auto_description": m.auto_description,
                            "auto_meta": {
                                "languages": m.auto_meta.languages,
                                "stack": m.auto_meta.stack,
                                "markers": m.auto_meta.markers,
                                "summary": m.auto_meta.summary,
                            },
                            "enrichment_pending": m.enrichment_pending,
                            "last_enriched_at": m.last_enriched_at.map(|t| t.to_rfc3339()),
                            "last_active_at": m.last_active_at.to_rfc3339(),
                        }))
                        .unwrap_or_default(),
                    )),
                    None => Ok(AgentToolResult::error("Mount not found")),
                }
            }

            "update" => {
                let id_str = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "update requires 'id'".to_string())?;
                let id =
                    MountId::parse_str(id_str).map_err(|e| format!("Invalid mount ID: {e}"))?;

                let auto_description = params
                    .get("auto_description")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let auto_meta = params.get("auto_meta").and_then(|v| v.as_object()).map(
                    |obj| crate::mount::MountMeta {
                        languages: obj
                            .get("languages")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        stack: obj
                            .get("stack")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        markers: obj
                            .get("markers")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        summary: obj
                            .get("summary")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or_default(),
                    },
                );

                if auto_description.is_none() && auto_meta.is_none() {
                    return Err(
                        "update requires 'auto_description' or 'auto_meta'".to_string()
                    );
                }

                match mm.update_enrichment(id, auto_description, auto_meta) {
                    Ok(m) => Ok(AgentToolResult::success(format!(
                        "Updated Mount '{}' ({}). enrichment_pending cleared.",
                        m.name, &id_str[..8.min(id_str.len())]
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to update mount: {e}"
                    ))),
                }
            }

            other => Err(format!(
                "Unknown mount action '{other}'. Valid: list, get, update"
            )),
        }
    }
}
