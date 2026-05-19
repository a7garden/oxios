//! Knowledge tool — agent-facing tool for markdown note management.
//!
//! Provides a single `knowledge` tool with action-based dispatch, following
//! the same pattern as `memory_tools.rs`. Actions: read, write, delete,
//! move, tree, search, backlinks.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool as OxiAgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};

use crate::kernel_handle::KnowledgeApi;
use crate::memory::MemoryManager;
use crate::KernelHandle;

/// Tool for reading, writing, and managing markdown knowledge notes.
///
/// Uses action-based dispatch: `read`, `write`, `delete`, `move`, `tree`,
/// `search`, `backlinks`.
pub struct KnowledgeTool {
    knowledge_dir: std::path::PathBuf,
    memory: Arc<MemoryManager>,
    engine: Arc<dyn crate::engine::EngineProvider>,
    default_model: String,
}

impl KnowledgeTool {
    /// Create from a [`KernelHandle`], extracting the necessary components.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            knowledge_dir: kernel.knowledge.root(),
            memory: kernel.agents.memory_manager().clone(),
            engine: Arc::new(crate::engine::OxiEngineProvider::new(
                kernel.knowledge.model_id(),
            )),
            default_model: kernel.knowledge.model_id().to_string(),
        }
    }

    /// Create with explicit parameters (for testing).
    pub fn new(knowledge_dir: std::path::PathBuf, memory: Arc<MemoryManager>) -> Self {
        Self {
            knowledge_dir,
            memory,
            engine: Arc::new(crate::engine::OxiEngineProvider::new("anthropic/claude-sonnet-4")),
            default_model: "anthropic/claude-sonnet-4".to_string(),
        }
    }

    /// Build a temporary KnowledgeApi for this operation.
    fn make_api(&self) -> KnowledgeApi {
        KnowledgeApi::new(
            self.knowledge_dir.clone(),
            self.memory.clone(),
            self.engine.clone(),
            self.default_model.clone(),
        )
    }
}

impl std::fmt::Debug for KnowledgeTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeTool").finish()
    }
}

#[async_trait]
impl OxiAgentTool for KnowledgeTool {
    fn name(&self) -> &str {
        "knowledge"
    }

    fn label(&self) -> &str {
        "Knowledge"
    }

    fn description(&self) -> &'static str {
        "Manage markdown knowledge notes. Actions: read, write, delete, move, tree, search, backlinks. Notes are stored as .md files and indexed for search."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "write", "delete", "move", "tree", "search", "backlinks"],
                    "description": "The action to perform"
                },
                "path": {
                    "type": "string",
                    "description": "Note path (e.g., 'brain/Rust.md' or 'Chat.md')"
                },
                "content": {
                    "type": "string",
                    "description": "Content for write action"
                },
                "old_path": {
                    "type": "string",
                    "description": "Old path for move action"
                },
                "new_path": {
                    "type": "string",
                    "description": "New path for move action"
                },
                "dir": {
                    "type": "string",
                    "description": "Directory for tree action (default: root)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query for search action"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results for search/tree (default: 20)"
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
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return Ok(AgentToolResult::error("action is required"));
        }

        let api = self.make_api();

        match action {
            "read" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return Ok(AgentToolResult::error("path is required for read"));
                }
                match api.note_read(path) {
                    Ok(Some(content)) => Ok(AgentToolResult::success(&content)),
                    Ok(None) => Ok(AgentToolResult::error(format!("Note '{}' not found", path))),
                    Err(e) => Ok(AgentToolResult::error(format!("Failed to read note: {e}"))),
                }
            }
            "write" => {
                let path = params["path"].as_str().unwrap_or("");
                let content = params["content"].as_str().unwrap_or("");
                if path.is_empty() {
                    return Ok(AgentToolResult::error("path is required for write"));
                }
                if content.is_empty() {
                    return Ok(AgentToolResult::error("content is required for write"));
                }
                match api.note_write(path, content) {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Note '{}' written successfully",
                        path
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Failed to write note: {e}"))),
                }
            }
            "delete" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return Ok(AgentToolResult::error("path is required for delete"));
                }
                match api.note_delete(path) {
                    Ok(()) => Ok(AgentToolResult::success(format!("Note '{}' deleted", path))),
                    Err(e) => Ok(AgentToolResult::error(format!("Failed to delete note: {e}"))),
                }
            }
            "move" => {
                let old_path = params["old_path"]
                    .as_str()
                    .or_else(|| {
                        // Also accept "path" as old_path if old_path not provided
                        if params["new_path"].as_str().is_some() {
                            params["path"].as_str()
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                let new_path = params["new_path"].as_str().unwrap_or("");
                if old_path.is_empty() || new_path.is_empty() {
                    return Ok(AgentToolResult::error(
                        "old_path and new_path are required for move",
                    ));
                }
                match api.note_move(old_path, new_path) {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Note moved from '{}' to '{}'",
                        old_path, new_path
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Failed to move note: {e}"))),
                }
            }
            "tree" => {
                let dir = params["dir"].as_str().unwrap_or("/");
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                match api.note_tree(dir) {
                    Ok(entries) => {
                        let count = entries.len();
                        let entries: Vec<_> = entries.into_iter().take(limit).collect();
                        if entries.is_empty() {
                            return Ok(AgentToolResult::success("Directory is empty"));
                        }
                        let mut output = format!(
                            "Found {} entries (showing {}):\n\n",
                            count,
                            entries.len()
                        );
                        for entry in &entries {
                            let kind = if entry.is_dir { "📁" } else { "📄" };
                            output.push_str(&format!(
                                "{} {} ({})\n",
                                kind, entry.display_name, entry.name
                            ));
                        }
                        Ok(AgentToolResult::success(&output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!("Failed to list notes: {e}"))),
                }
            }
            "search" => {
                let query = params["query"].as_str().unwrap_or("");
                if query.is_empty() {
                    return Ok(AgentToolResult::error("query is required for search"));
                }
                let limit = params["limit"].as_u64().unwrap_or(10) as usize;
                match api.search(query, limit) {
                    Ok(hits) => {
                        if hits.is_empty() {
                            return Ok(AgentToolResult::success("No matching notes found"));
                        }
                        let mut output = format!("Found {} matching notes:\n\n", hits.len());
                        for hit in &hits {
                            let score = hit
                                .semantic_score
                                .map(|s| format!("{:.2}", s))
                                .unwrap_or_else(|| "-".to_string());
                            output.push_str(&format!(
                                "- {} (path: {}, backlinks: {}, name_sim: {}%, score: {})\n",
                                hit.name,
                                hit.path,
                                hit.backlink_count,
                                hit.name_similarity,
                                score
                            ));
                        }
                        Ok(AgentToolResult::success(&output))
                    }
                    Err(e) => {
                        Ok(AgentToolResult::error(format!("Failed to search notes: {e}")))
                    }
                }
            }
            "backlinks" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return Ok(AgentToolResult::error("path is required for backlinks"));
                }
                let backlinks = api.backlinks_for(path);
                if backlinks.is_empty() {
                    return Ok(AgentToolResult::success(format!(
                        "No backlinks for '{}'",
                        path
                    )));
                }
                let mut output = format!("Backlinks for '{}' ({}):\n\n", path, backlinks.len());
                for bl in &backlinks {
                    output.push_str(&format!(
                        "- {} → {} (line {})\n",
                        bl.source_path, bl.target_path, bl.line_number
                    ));
                }
                Ok(AgentToolResult::success(&output))
            }
            _ => Ok(AgentToolResult::error(format!(
                "Unknown action '{}'. Must be one of: read, write, delete, move, tree, search, backlinks",
                action
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_tool_schema() {
        let tool = KnowledgeTool::new(
            std::path::PathBuf::from("/tmp/test-kb"),
            Arc::new(MemoryManager::new(
                Arc::new(
                    crate::state_store::StateStore::new(std::path::PathBuf::from("/tmp/test-state"))
                        .unwrap(),
                ),
            )),
        );
        assert_eq!(tool.name(), "knowledge");
        let schema = tool.parameters_schema();
        assert!(schema["required"].is_array());
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 7);
    }
}
