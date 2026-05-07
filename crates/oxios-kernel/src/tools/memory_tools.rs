//! Memory tools for cross-session agent memory.
//!
//! Provides three tools:
//! - `memory_write` — write a memory entry
//! - `memory_read` — read/list memory entries
//! - `memory_search` — search memory entries by content or tags

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use oxi_agent::{AgentTool, AgentToolResult};
use serde_json::{json, Value};

use crate::memory::{MemoryEntry, MemoryManager, MemoryType};

/// Tool for writing memory entries that persist across sessions.
pub struct MemoryWriteTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryWriteTool {
    /// Create a new MemoryWriteTool.
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

impl std::fmt::Debug for MemoryWriteTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryWriteTool").finish()
    }
}

#[async_trait]
impl AgentTool for MemoryWriteTool {
    fn name(&self) -> &str {
        "memory_write"
    }

    fn label(&self) -> &str {
        "Memory Write"
    }

    fn description(&self) -> &str {
        "Write a memory entry that persists across sessions. Use this to save important facts, episodes, or knowledge for future reference."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The memory content to store"
                },
                "memory_type": {
                    "type": "string",
                    "enum": ["fact", "episode", "knowledge"],
                    "description": "The type of memory entry"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for categorization"
                },
                "importance": {
                    "type": "number",
                    "description": "Importance score 0.0-1.0 (default 0.5)"
                }
            },
            "required": ["content", "memory_type"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, oxi_agent::tools::ToolError> {
        let content = params["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if content.is_empty() {
            return Ok(AgentToolResult::error("content is required"));
        }

        let memory_type_str = params["memory_type"]
            .as_str()
            .unwrap_or("fact");
        let memory_type = match memory_type_str {
            "fact" => MemoryType::Fact,
            "episode" => MemoryType::Episode,
            "knowledge" => MemoryType::Knowledge,
            _ => return Ok(AgentToolResult::error(
                &format!("Invalid memory_type '{}'. Must be one of: fact, episode, knowledge", memory_type_str),
            )),
        };

        let tags: Vec<String> = params["tags"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let importance = params["importance"]
            .as_f64()
            .unwrap_or(0.5) as f32;

        let now = Utc::now();
        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            memory_type,
            content,
            source: "agent".to_string(),
            session_id: None,
            tags,
            importance: importance.clamp(0.0, 1.0),
            created_at: now,
            accessed_at: now,
            access_count: 0,
        };
        let entry_id = entry.id.clone();

        match self.memory_manager.remember(entry).await {
            Ok(_) => Ok(AgentToolResult::success(&format!(
                "Memory entry saved (id: {}, type: {})",
                entry_id, memory_type_str,
            ))),
            Err(e) => Ok(AgentToolResult::error(&format!("Failed to write memory: {e}"))),
        }
    }
}

/// Tool for reading memory entries.
pub struct MemoryReadTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryReadTool {
    /// Create a new MemoryReadTool.
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

impl std::fmt::Debug for MemoryReadTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryReadTool").finish()
    }
}

#[async_trait]
impl AgentTool for MemoryReadTool {
    fn name(&self) -> &str {
        "memory_read"
    }

    fn label(&self) -> &str {
        "Memory Read"
    }

    fn description(&self) -> &str {
        "Read memory entries. Provide 'id' and 'memory_type' to read a specific entry, or just 'memory_type' to list entries of that type."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Optional specific memory entry ID to read."
                },
                "memory_type": {
                    "type": "string",
                    "enum": ["fact", "episode", "knowledge"],
                    "description": "Type of memory to list (required if no id provided)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max entries to return when listing (default 10)"
                }
            }
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, oxi_agent::tools::ToolError> {
        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        if let Some(id) = params["id"].as_str() {
            // Need memory_type to look up by ID
            let memory_type_str = params["memory_type"].as_str().unwrap_or("fact");
            let memory_type = parse_memory_type(memory_type_str);

            match self.memory_manager.get(id, memory_type).await {
                Ok(Some(entry)) => {
                    let output = format!(
                        "ID: {}\nType: {}\nSource: {}\nTags: {}\nImportance: {:.2}\nCreated: {}\n\n{}",
                        entry.id,
                        entry.memory_type.label(),
                        entry.source,
                        entry.tags.join(", "),
                        entry.importance,
                        entry.created_at,
                        entry.content,
                    );
                    Ok(AgentToolResult::success(&output))
                }
                Ok(None) => Ok(AgentToolResult::error(&format!("Memory entry '{}' not found", id))),
                Err(e) => Ok(AgentToolResult::error(&format!("Failed to read memory: {e}"))),
            }
        } else {
            // List entries by type
            let memory_type_str = params["memory_type"].as_str().unwrap_or("fact");
            let memory_type = parse_memory_type(memory_type_str);

            match self.memory_manager.list(memory_type, limit).await {
                Ok(entries) => {
                    if entries.is_empty() {
                        return Ok(AgentToolResult::success(&format!(
                            "No {} memory entries found.",
                            memory_type_str,
                        )));
                    }
                    let mut output = format!(
                        "Found {} {} entries:\n\n",
                        entries.len(),
                        memory_type_str,
                    );
                    for entry in &entries {
                        let preview = if entry.content.len() > 100 {
                            &entry.content[..100]
                        } else {
                            &entry.content
                        };
                        output.push_str(&format!(
                            "- [{}] {} (id: {}…, tags: {})\n",
                            entry.memory_type.label(),
                            preview,
                            &entry.id[..8.min(entry.id.len())],
                            entry.tags.join(", "),
                        ));
                    }
                    Ok(AgentToolResult::success(&output))
                }
                Err(e) => Ok(AgentToolResult::error(&format!("Failed to list memory: {e}"))),
            }
        }
    }
}

/// Tool for searching memory entries by content or tags.
pub struct MemorySearchTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemorySearchTool {
    /// Create a new MemorySearchTool.
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

impl std::fmt::Debug for MemorySearchTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemorySearchTool").finish()
    }
}

#[async_trait]
impl AgentTool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn label(&self) -> &str {
        "Memory Search"
    }

    fn description(&self) -> &str {
        "Search memory entries by keyword query. Optionally filter by memory type."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Text to search for in memory content"
                },
                "memory_type": {
                    "type": "string",
                    "enum": ["fact", "episode", "knowledge", "conversation", "session"],
                    "description": "Optional memory type to filter by"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (default 10)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, oxi_agent::tools::ToolError> {
        let query = params["query"].as_str().unwrap_or("");
        if query.is_empty() {
            return Ok(AgentToolResult::error("query is required"));
        }

        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        let memory_type = params["memory_type"]
            .as_str()
            .map(parse_memory_type);

        match self.memory_manager.search(query, memory_type, limit).await {
            Ok(entries) => {
                if entries.is_empty() {
                    return Ok(AgentToolResult::success("No matching memory entries found."));
                }
                let mut output = format!("Found {} matching entries:\n\n", entries.len());
                for entry in &entries {
                    let preview = if entry.content.len() > 100 {
                        &entry.content[..100]
                    } else {
                        &entry.content
                    };
                    output.push_str(&format!(
                        "- [{}] {} (id: {}…, importance: {:.2}, tags: {})\n",
                        entry.memory_type.label(),
                        preview,
                        &entry.id[..8.min(entry.id.len())],
                        entry.importance,
                        entry.tags.join(", "),
                    ));
                }
                Ok(AgentToolResult::success(&output))
            }
            Err(e) => Ok(AgentToolResult::error(&format!("Failed to search memory: {e}"))),
        }
    }
}

/// Parse a memory type string, defaulting to Fact.
fn parse_memory_type(s: &str) -> MemoryType {
    match s {
        "conversation" => MemoryType::Conversation,
        "session" => MemoryType::Session,
        "fact" => MemoryType::Fact,
        "episode" => MemoryType::Episode,
        "knowledge" => MemoryType::Knowledge,
        _ => MemoryType::Fact,
    }
}
