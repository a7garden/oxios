//! Marketplace tool — wraps `MarketplaceApi` behind the `AgentTool` interface.
//!
//! Provides agents with multi-registry marketplace capabilities.
//! Registries: ClawHub, Skills.sh (Vercel Labs ecosystem).
//!
//! Actions: search, get, install, update, update_all, check_updates,
//!           skills_sh_search, skills_sh_list, skills_sh_install, skills_sh_detail.
//!
//! ## Example
//!
//! ```json
//! { "action": "search", "query": "code review", "limit": 10 }
//! { "action": "install", "slug": "code-review-helper", "version": "1.2.0" }
//! { "action": "skills_sh_search", "query": "frontend design" }
//! { "action": "skills_sh_install", "skill_id": "vercel-labs/agent-skills/frontend-design" }
//! ```

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{Value, json};

use crate::kernel_handle::KernelHandle;
use crate::kernel_handle::MarketplaceApi;

/// Agent tool for multi-registry marketplace operations.
///
/// Wraps the `MarketplaceApi` domain of the `KernelHandle`. Allows agents
/// to search, install, and update skills from ClawHub and Skills.sh.
///
/// ## Actions
///
/// | Action              | Description                              | Required params  | Optional params      |
/// |---------------------|------------------------------------------|------------------|----------------------|
/// | `search`            | Search ClawHub for skills                | `query`          | `limit`              |
/// | `get`               | Get skill detail from ClawHub            | `slug`           | —                    |
/// | `install`           | Install a skill from ClawHub             | `slug`           | `version`            |
/// | `update`            | Update a specific installed skill         | `slug`           | —                    |
/// | `update_all`        | Update all installed ClawHub skills       | —                | —                    |
/// | `check_updates`     | Check for available updates              | —                | —                    |
/// | `skills_sh_search`  | Search Skills.sh for skills              | `query`          | `limit`              |
/// | `skills_sh_list`    | List Skills.sh leaderboard               | —                | `view`, `page`, `per_page` |
/// | `skills_sh_install` | Install a skill from Skills.sh           | `skill_id`       | —                    |
/// | `skills_sh_detail`  | Get skill detail from Skills.sh          | `skill_id`       | —                    |
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
        "Search, install, and update skills from the ClawHub marketplace and Skills.sh registry. \
         ClawHub actions: search, get, install, update, update_all, check_updates. \
         Skills.sh actions: skills_sh_search, skills_sh_list, skills_sh_install, skills_sh_detail."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "get", "install", "update", "update_all", "check_updates", "skills_sh_search", "skills_sh_list", "skills_sh_install", "skills_sh_detail"],
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
                "skill_id": {
                    "type": "string",
                    "description": "Skills.sh skill identifier (format: owner/repo/skill-slug)"
                },
                "version": {
                    "type": "string",
                    "description": "Specific version to install (install action, optional; defaults to latest)"
                },
                "view": {
                    "type": "string",
                    "description": "Skills.sh leaderboard view: all-time, trending, or hot",
                    "default": "all-time"
                },
                "page": {
                    "type": "integer",
                    "description": "Page number for Skills.sh listing (0-indexed)"
                },
                "per_page": {
                    "type": "integer",
                    "description": "Results per page for Skills.sh listing (1-500, default 50)"
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
                        "Marketplace search failed: {e}"
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
                        "Failed to get skill '{slug}': {e}"
                    ))),
                }
            }

            "install" => {
                let slug = params
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "install requires 'slug' parameter".to_string())?;
                let version = params.get("version").and_then(|v| v.as_str());

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
                        "Failed to install '{slug}': {e}"
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
                        "Failed to update '{slug}': {e}"
                    ))),
                }
            }

            "update_all" => match self.api.update_all().await {
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
                    "Failed to update all skills: {e}"
                ))),
            },

            "check_updates" => match self.api.check_updates().await {
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
                    "Failed to check updates: {e}"
                ))),
            },

            // ─── Skills.sh Actions ───────────────────────────────────────
            "skills_sh_search" => {
                let query = params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "skills_sh_search requires 'query' parameter".to_string())?;
                let limit = params["limit"].as_u64().map(|l| l as usize);

                match self.api.search_skills_sh(query, limit).await {
                    Ok(resp) => {
                        let display: Vec<Value> = resp
                            .data
                            .into_iter()
                            .map(|s| {
                                json!({
                                    "id": s.id,
                                    "name": s.name,
                                    "slug": s.slug,
                                    "source": s.source,
                                    "installs": s.installs,
                                    "sourceType": s.source_type,
                                    "installUrl": s.install_url,
                                })
                            })
                            .collect();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "results": display,
                                "count": display.len(),
                                "searchType": resp.search_type,
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Skills.sh search failed: {e}"
                    ))),
                }
            }

            "skills_sh_list" => {
                let view = params.get("view").and_then(|v| v.as_str());
                let page = params["page"].as_i64();
                let per_page = params["per_page"].as_i64();

                match self.api.list_skills_sh(view, page, per_page).await {
                    Ok(resp) => {
                        let display: Vec<Value> = resp
                            .data
                            .into_iter()
                            .map(|s| {
                                json!({
                                    "id": s.id,
                                    "name": s.name,
                                    "slug": s.slug,
                                    "source": s.source,
                                    "installs": s.installs,
                                })
                            })
                            .collect();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "results": display,
                                "pagination": {
                                    "page": resp.pagination.page,
                                    "total": resp.pagination.total,
                                    "hasMore": resp.pagination.has_more,
                                },
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Skills.sh list failed: {e}"
                    ))),
                }
            }

            "skills_sh_install" => {
                let skill_id = params
                    .get("skill_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "skills_sh_install requires 'skill_id' parameter".to_string())?;

                match self.api.install_skills_sh(skill_id).await {
                    Ok(result) => Ok(AgentToolResult::success(
                        serde_json::to_string_pretty(&json!({
                            "ok": result.ok,
                            "slug": result.slug,
                            "source": result.source,
                            "skillId": result.skill_id,
                            "targetDir": result.target_dir.display().to_string(),
                            "fileCount": result.file_count,
                        }))
                        .unwrap_or_default(),
                    )),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to install from Skills.sh: {e}"
                    ))),
                }
            }

            "skills_sh_detail" => {
                let skill_id = params
                    .get("skill_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "skills_sh_detail requires 'skill_id' parameter".to_string())?;

                match self.api.get_skills_sh_skill(skill_id).await {
                    Ok(detail) => {
                        let files: Vec<Value> = detail
                        .files
                        .map(|f| f.into_iter().map(|file| json!({ "path": file.path, "size": file.contents.len() })).collect())
                        .unwrap_or_default();
                        Ok(AgentToolResult::success(
                            serde_json::to_string_pretty(&json!({
                                "id": detail.id,
                                "source": detail.source,
                                "slug": detail.slug,
                                "installs": detail.installs,
                                "hash": detail.hash,
                                "fileCount": files.len(),
                                "files": files,
                            }))
                            .unwrap_or_default(),
                        ))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to get Skills.sh detail: {e}"
                    ))),
                }
            }

            other => Err(format!(
                "Unknown marketplace action '{other}'. Valid: search, get, install, update, update_all, check_updates, skills_sh_search, skills_sh_list, skills_sh_install, skills_sh_detail"
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
                    "enum": ["search", "get", "install", "update", "update_all", "check_updates",
                             "skills_sh_search", "skills_sh_list", "skills_sh_install", "skills_sh_detail"]
                },
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "slug": { "type": "string" },
                "skill_id": { "type": "string" },
                "version": { "type": "string" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 10);
    }
}
