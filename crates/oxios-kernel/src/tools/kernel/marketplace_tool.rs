//! Marketplace tool — wraps `MarketplaceApi` behind the `AgentTool` interface.
//!
//! Provides agents with ClawHub marketplace capabilities.
//! Actions: search, get, install, update, update_all, check_updates.
//!
//! ## Example
//!
//! ```json
//! { "action": "search", "query": "code review", "limit": 10 }
//! { "action": "install", "slug": "code-review-helper", "version": "1.2.0" }
//! { "action": "update", "slug": "code-review-helper" }
//! { "action": "check_updates" }
//! ```

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::kernel_handle::KernelHandle;
use crate::kernel_handle::MarketplaceApi;

/// Agent tool for ClawHub marketplace operations.
///
/// Wraps the `MarketplaceApi` domain of the `KernelHandle`. Allows agents
/// to search, install, and update skills from the ClawHub marketplace.
///
/// ## Actions
///
/// | Action         | Description                        | Required params | Optional params      |
/// |----------------|------------------------------------|-----------------|----------------------|
/// | `search`       | Search ClawHub for skills          | `query`         | `limit`              |
/// | `get`          | Get skill detail from ClawHub      | `slug`          | —                    |
/// | `install`      | Install a skill from ClawHub       | `slug`          | `version`            |
/// | `update`       | Update a specific installed skill   | `slug`          | —                    |
/// | `update_all`   | Update all installed ClawHub skills | —             | —                    |
/// | `check_updates`| Check for available updates        | —               | —                    |
pub struct MarketplaceTool {
    api: MarketplaceApi,
}

impl MarketplaceTool {
    /// Create a new `MarketplaceTool` from a `KernelHandle`.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            api: kernel.marketplace_api.clone(),
        }
    }
}

impl std::fmt::Debug for MarketplaceTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceTool").finish()
    }
}

#[async_trait]
impl AgentTool for MarketplaceTool {
    fn name(&self) -> &str {
        "marketplace"
    }

    fn label(&self) -> &str {
        "Marketplace"
    }

    fn description(&self) -> &'static str {
        "Search, install, and update skills from the ClawHub marketplace. \
         Actions: search, get, install, update, update_all, check_updates."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "get", "install", "update", "update_all", "check_updates"],
                    "description": "Marketplace operation to perform"
                },
                "query": {
                    "type": "string",
                    "description": "Search query string (search action)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (search action, default 20)"
                },
                "slug": {
                    "type": "string",
                    "description": "Skill slug — the unique identifier on ClawHub (get, install, update actions)"
                },
                "version": {
                    "type": "string",
                    "description": "Specific version to install (install action, optional; defaults to latest)"
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
            "search" => {
                let query = params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "search requires 'query' parameter".to_string())?;
                let limit = params["limit"].as_u64().map(|l| l as usize);

                match self.api.search(query, limit).await {
                    Ok(results) => {
                        let display: Vec<Value> = results
                            .into_iter()
                            .map(|r| {
                                json!({
                                    "slug": r.slug,
                                    "displayName": r.display_name,
                                    "summary": r.summary,
                                    "version": r.version,
                                    "score": r.score,
                                })
                            })
                            .collect();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "results": display,
                                "count": display.len(),
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Marketplace search failed: {}",
                        e
                    ))),
                }
            }

            "get" => {
                let slug = params
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "get requires 'slug' parameter".to_string())?;

                match self.api.get_skill(slug).await {
                    Ok(detail) => {
                        let display = json!({
                            "slug": detail.skill.as_ref().map(|s| &s.slug),
                            "displayName": detail.skill.as_ref().map(|s| &s.display_name),
                            "summary": detail.skill.as_ref().and_then(|s| s.summary.clone()),
                            "latestVersion": detail.latest_version.as_ref().map(|v| &v.version),
                            "changelog": detail.latest_version.as_ref().and_then(|v| v.changelog.clone()),
                            "os": detail.metadata.as_ref().and_then(|m| m.os.clone()),
                            "owner": detail.owner.as_ref().map(|o| {
                                json!({
                                    "handle": o.handle,
                                    "displayName": o.display_name,
                                })
                            }),
                        });
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&display).unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to get skill '{}': {}",
                        slug, e
                    ))),
                }
            }

            "install" => {
                let slug = params
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "install requires 'slug' parameter".to_string())?;
                let version = params
                    .get("version")
                    .and_then(|v| v.as_str());

                match self.api.install(slug, version).await {
                    Ok(result) => Ok(AgentToolResult::success(
                        serde_json::to_string_pretty(&json!({
                            "ok": result.ok,
                            "slug": result.slug,
                            "version": result.version,
                            "targetDir": result.target_dir.display().to_string(),
                            "changelog": result.changelog,
                        }))
                        .unwrap_or_default(),
                    )),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to install '{}': {}",
                        slug, e
                    ))),
                }
            }

            "update" => {
                let slug = params
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "update requires 'slug' parameter".to_string())?;

                match self.api.update(slug).await {
                    Ok(result) => Ok(AgentToolResult::success(
                        serde_json::to_string_pretty(&json!({
                            "ok": result.ok,
                            "slug": result.slug,
                            "previousVersion": result.previous_version,
                            "version": result.version,
                            "changed": result.changed,
                        }))
                        .unwrap_or_default(),
                    )),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to update '{}': {}",
                        slug, e
                    ))),
                }
            }

            "update_all" => {
                match self.api.update_all().await {
                    Ok(results) => {
                        let display: Vec<Value> = results
                            .into_iter()
                            .map(|r| {
                                json!({
                                    "ok": r.ok,
                                    "slug": r.slug,
                                    "previousVersion": r.previous_version,
                                    "version": r.version,
                                    "changed": r.changed,
                                    "error": r.error,
                                })
                            })
                            .collect();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "results": display,
                                "count": display.len(),
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to update all skills: {}",
                        e
                    ))),
                }
            }

            "check_updates" => {
                match self.api.check_updates().await {
                    Ok(updates) => {
                        if updates.is_empty() {
                            return Ok(AgentToolResult::success("All skills are up to date."));
                        }
                        let display: Vec<Value> = updates
                            .into_iter()
                            .map(|u| {
                                json!({
                                    "slug": u.slug,
                                    "currentVersion": u.current_version,
                                    "latestVersion": u.latest_version,
                                    "changelog": u.changelog,
                                })
                            })
                            .collect();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "updates": display,
                                "count": display.len(),
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to check updates: {}",
                        e
                    ))),
                }
            }

            other => Err(format!(
                "Unknown marketplace action '{}'. Valid: search, get, install, update, update_all, check_updates",
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
                    "enum": ["search", "get", "install", "update", "update_all", "check_updates"]
                },
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "slug": { "type": "string" },
                "version": { "type": "string" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 6);
    }
}