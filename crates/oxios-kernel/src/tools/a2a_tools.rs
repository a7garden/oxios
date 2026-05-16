//! A2A tools — let agents communicate with other agents at runtime.
//!
//! Provides three tools:
//! - `a2a_delegate` — delegate a task to another agent by capability
//! - `a2a_send` — send a message to a specific agent
//! - `a2a_query` — discover agents by capability or skill

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext, ToolError};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::a2a::{A2AMessage, A2AProtocol, TaskPriority, TaskSpec};
use crate::types::AgentId;
// ─── A2aDelegateTool ───────────────────────────────────────────────────

/// Tool for delegating a task to another agent discovered by capability.
///
/// Usage flow:
/// 1. Agent calls `a2a_delegate` with a description and required capability.
/// 2. The tool queries the AgentCardRegistry for agents with that capability.
/// 3. If found, it delegates the task via A2A and waits for the result.
/// 4. Returns the execution result to the calling agent.
pub struct A2aDelegateTool {
    a2a: Arc<A2AProtocol>,
    my_agent_id: AgentId,
}

impl A2aDelegateTool {
    /// Create a new A2A delegate tool.
    pub fn new(a2a: Arc<A2AProtocol>, agent_id: AgentId) -> Self {
        Self {
            a2a,
            my_agent_id: agent_id,
        }
    }

    /// Create an `A2aDelegateTool` from a [`KernelHandle`].
    ///
    /// Extracts the A2A protocol from the kernel's a2a facade.
    pub fn from_kernel(kernel: &crate::kernel_handle::KernelHandle, agent_id: AgentId) -> Self {
        Self::new(kernel.a2a.protocol().clone(), agent_id)
    }
}

impl std::fmt::Debug for A2aDelegateTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aDelegateTool").finish()
    }
}

#[async_trait]
impl AgentTool for A2aDelegateTool {
    fn name(&self) -> &str {
        "a2a_delegate"
    }

    fn label(&self) -> &str {
        "A2A Delegate"
    }

    fn description(&self) -> &str {
        "Delegate a task to another agent. Specify a capability (e.g. 'code-review', 'testing') \
         and a description of the work. The system will find a suitable agent, execute the task, \
         and return the result. This is a blocking call — it waits for the delegated agent to complete."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "Human-readable description of the task to delegate"
                },
                "capability": {
                    "type": "string",
                    "description": "Required capability of the target agent (e.g. 'code-review', 'testing', 'debugging')"
                },
                "payload": {
                    "type": "object",
                    "description": "Structured data for the task (optional)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["low", "normal", "high", "critical"],
                    "description": "Task priority (default: normal)"
                }
            },
            "required": ["description", "capability"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _shutdown: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, ToolError> {
        let description = params["description"].as_str().unwrap_or("").to_string();
        if description.is_empty() {
            return Ok(AgentToolResult::error(
                "Missing required parameter: description",
            ));
        }

        let capability = params["capability"].as_str().unwrap_or("").to_string();
        if capability.is_empty() {
            return Ok(AgentToolResult::error(
                "Missing required parameter: capability",
            ));
        }

        let payload = params.get("payload").cloned().unwrap_or(json!({}));
        let priority = parse_priority(params["priority"].as_str());

        let my_id = self.my_agent_id;

        // 1. Find agents with the required capability.
        let candidates = match self.a2a.query_capabilities(&capability).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(AgentToolResult::error(format!(
                    "Failed to query capabilities: {e}"
                )))
            }
        };

        if candidates.is_empty() {
            // No agent available — guide the LLM to handle it itself.
            return Ok(AgentToolResult::success(format!(
                "No agents currently available with capability '{}'. You should handle this task yourself.",
                capability
            )));
        }

        // 2. Pick the first available agent (could be smarter — load-balancing later).
        let target = &candidates[0];
        let target_id = target.agent_id;

        tracing::info!(
            from = %my_id,
            to = %target_id,
            target_name = %target.name,
            capability = %capability,
            "A2A delegating task"
        );

        // 3. Create task spec and delegate via A2A.
        let task = TaskSpec::new(&description, payload.clone()).with_priority(priority);
        let task_id = task.task_id;

        // 4. Execute via dispatch handler (blocking — waits for result).
        match self.a2a.execute_delegation(my_id, target_id, task).await {
            Some(Ok(result)) => Ok(AgentToolResult::success(
                serde_json::to_string(&json!({
                    "task_id": task_id.to_string(),
                    "delegated_to": target.name,
                    "delegated_to_id": target_id.to_string(),
                    "status": "completed",
                    "result": result,
                }))
                .unwrap_or_default(),
            )),
            Some(Err(e)) => Ok(AgentToolResult::error(format!(
                "A2A delegation failed: {}",
                e
            ))),
            None => {
                // No handler registered — fall back to fire-and-forget.
                tracing::warn!("No A2A dispatch handler registered, using fire-and-forget");
                match self
                    .a2a
                    .delegate_task(my_id, target_id, TaskSpec::new(&description, payload))
                    .await
                {
                    Ok(_) => Ok(AgentToolResult::success(format!(
                        "Task delegated to '{}' (no handler — fire-and-forget). Task ID: {}",
                        target.name, task_id
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Delegation failed: {e}"))),
                }
            }
        }
    }
}

// ─── A2aSendTool ───────────────────────────────────────────────────────

/// Tool for sending a direct message to a specific agent.
///
/// Unlike `a2a_delegate`, this sends a fire-and-forget message.
/// Use for status updates, notifications, or simple queries.
pub struct A2aSendTool {
    a2a: Arc<A2AProtocol>,
    my_agent_id: AgentId,
}

impl A2aSendTool {
    /// Create a new A2A send tool.
    pub fn new(a2a: Arc<A2AProtocol>, agent_id: AgentId) -> Self {
        Self {
            a2a,
            my_agent_id: agent_id,
        }
    }

    /// Create an `A2aSendTool` from a [`KernelHandle`].
    ///
    /// Extracts the A2A protocol from the kernel's a2a facade.
    pub fn from_kernel(kernel: &crate::kernel_handle::KernelHandle, agent_id: AgentId) -> Self {
        Self::new(kernel.a2a.protocol().clone(), agent_id)
    }
}

impl std::fmt::Debug for A2aSendTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aSendTool").finish()
    }
}

#[async_trait]
impl AgentTool for A2aSendTool {
    fn name(&self) -> &str {
        "a2a_send"
    }

    fn label(&self) -> &str {
        "A2A Send"
    }

    fn description(&self) -> &str {
        "Send a message to a specific agent by ID. Fire-and-forget — does not wait for a response. \
         Use for status updates, notifications, or sharing information."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_agent_id": {
                    "type": "string",
                    "description": "UUID of the target agent"
                },
                "message_type": {
                    "type": "string",
                    "enum": ["status_update", "result_sharing", "handshake"],
                    "description": "Type of message to send (default: status_update)"
                },
                "content": {
                    "type": "string",
                    "description": "The message content"
                },
                "task_id": {
                    "type": "string",
                    "description": "Task UUID this message relates to (for status_update and result_sharing)"
                },
                "payload": {
                    "type": "object",
                    "description": "Structured data to share (optional, for result_sharing)"
                },
                "progress": {
                    "type": "integer",
                    "description": "Progress percentage for status updates (0-100)"
                }
            },
            "required": ["target_agent_id", "message_type", "content"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _shutdown: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, ToolError> {
        let target_str = params["target_agent_id"].as_str().unwrap_or("");
        let target_id: AgentId = match Uuid::parse_str(target_str) {
            Ok(id) => id,
            Err(e) => {
                return Ok(AgentToolResult::error(format!(
                    "Invalid target_agent_id: {e}"
                )))
            }
        };

        let message_type = params["message_type"].as_str().unwrap_or("status_update");
        let content = params["content"].as_str().unwrap_or("").to_string();
        let payload = params.get("payload").cloned().unwrap_or(json!({}));
        let progress = params["progress"].as_u64().unwrap_or(0) as u8;
        let task_id: Uuid = params["task_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        let my_id = self.my_agent_id;

        let message = match message_type {
            "status_update" => A2AMessage::StatusUpdate {
                task_id,
                progress,
                message: content,
            },
            "result_sharing" => A2AMessage::ResultSharing {
                task_id,
                result: payload,
                summary: content,
            },
            "handshake" => {
                let card = self.a2a.registry().get_agent(my_id).await;
                let (name, capabilities) = card
                    .map(|c| (c.name, c.capabilities))
                    .unwrap_or(("unknown".into(), vec![]));
                A2AMessage::Handshake {
                    agent_id: my_id,
                    name,
                    capabilities,
                }
            }
            _ => {
                return Ok(AgentToolResult::error(format!(
                    "Unknown message_type: {message_type}"
                )))
            }
        };

        match self.a2a.send_message(my_id, target_id, message).await {
            Ok(request_id) => Ok(AgentToolResult::success(
                serde_json::to_string(&json!({
                    "request_id": request_id.to_string(),
                    "sent_to": target_str,
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Failed to send: {e}"))),
        }
    }
}

// ─── A2aQueryTool ──────────────────────────────────────────────────────

/// Tool for discovering other agents by capability.
///
/// Returns a list of agent cards matching the requested capability or skill.
pub struct A2aQueryTool {
    a2a: Arc<A2AProtocol>,
}

impl A2aQueryTool {
    /// Create a new A2A query tool.
    pub fn new(a2a: Arc<A2AProtocol>) -> Self {
        Self { a2a }
    }

    /// Create an `A2aQueryTool` from a [`KernelHandle`].
    ///
    /// Extracts the A2A protocol from the kernel's a2a facade.
    pub fn from_kernel(kernel: &crate::kernel_handle::KernelHandle) -> Self {
        Self::new(kernel.a2a.protocol().clone())
    }
}

impl std::fmt::Debug for A2aQueryTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aQueryTool").finish()
    }
}

#[async_trait]
impl AgentTool for A2aQueryTool {
    fn name(&self) -> &str {
        "a2a_query"
    }

    fn label(&self) -> &str {
        "A2A Query"
    }

    fn description(&self) -> &str {
        "Discover other agents by capability or skill. Returns a list of available agents \
         with their names, capabilities, and status."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "capability": {
                    "type": "string",
                    "description": "Search for agents with this capability"
                },
                "skill": {
                    "type": "string",
                    "description": "Search for agents with this skill"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)"
                }
            }
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _shutdown: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, ToolError> {
        let capability = params["capability"].as_str();
        let skill = params["skill"].as_str();
        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        let agents = if let Some(cap) = capability {
            match self.a2a.query_capabilities(cap).await {
                Ok(a) => a,
                Err(e) => return Ok(AgentToolResult::error(format!("Query failed: {e}"))),
            }
        } else if let Some(sk) = skill {
            match self.a2a.registry().find_agents_by_skill(sk).await {
                Ok(a) => a,
                Err(e) => return Ok(AgentToolResult::error(format!("Query failed: {e}"))),
            }
        } else {
            // No filter — return all agents.
            self.a2a.registry().list_agents().await
        };

        let cards: Vec<Value> = agents
            .into_iter()
            .take(limit)
            .map(|card| {
                json!({
                    "agent_id": card.agent_id.to_string(),
                    "name": card.name,
                    "description": card.description,
                    "capabilities": card.capabilities,
                    "skills": card.skills,
                    "status": format!("{:?}", card.status),
                })
            })
            .collect();

        Ok(AgentToolResult::success(
            serde_json::to_string(&json!({
                "agents": cards,
                "count": cards.len(),
            }))
            .unwrap_or_default(),
        ))
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────

fn parse_priority(s: Option<&str>) -> TaskPriority {
    match s {
        Some("low") => TaskPriority::Low,
        Some("high") => TaskPriority::High,
        Some("critical") => TaskPriority::Critical,
        _ => TaskPriority::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::types::AgentStatus;

    fn test_a2a() -> Arc<A2AProtocol> {
        Arc::new(A2AProtocol::new(EventBus::new(256)))
    }

    fn register_test_agent(a2a: &A2AProtocol, name: &str, caps: &[&str]) -> AgentId {
        let id = Uuid::new_v4();
        let mut card = crate::a2a::AgentCard::new(id, name, format!("Test agent: {name}"));
        for cap in caps {
            card = card.with_capability(*cap);
        }
        // Caller must be in async context.
        // Use `futures::executor::block_on` or call from #[tokio::test].
        id
    }

    async fn register_agent_async(a2a: &A2AProtocol, name: &str, caps: &[&str]) -> AgentId {
        let id = Uuid::new_v4();
        let mut card = crate::a2a::AgentCard::new(id, name, format!("Test agent: {name}"));
        for cap in caps {
            card = card.with_capability(*cap);
        }
        a2a.registry().register_agent(card).await.unwrap();
        id
    }

    #[tokio::test]
    async fn test_a2a_query_finds_capability() {
        let a2a = test_a2a();
        register_agent_async(&a2a, "reviewer", &["code-review"]).await;

        let tool = A2aQueryTool::new(a2a.clone());
        let params = json!({"capability": "code-review"});
        let result = tool.execute("tc", params, None).await.unwrap();
        assert!(result.output.contains("reviewer"));
        assert!(result.output.contains("1"));
    }

    #[tokio::test]
    async fn test_a2a_query_no_match() {
        let a2a = test_a2a();

        let tool = A2aQueryTool::new(a2a.clone());
        let params = json!({"capability": "nonexistent"});
        let result = tool.execute("tc", params, None).await.unwrap();
        assert!(result.output.contains("0"));
    }

    #[tokio::test]
    async fn test_a2a_query_respects_limit() {
        let a2a = test_a2a();
        register_agent_async(&a2a, "a1", &["test"]).await;
        register_agent_async(&a2a, "a2", &["test"]).await;
        register_agent_async(&a2a, "a3", &["test"]).await;

        let tool = A2aQueryTool::new(a2a.clone());
        let params = json!({"capability": "test", "limit": 2});
        let result = tool.execute("tc", params, None).await.unwrap();
        assert!(result.output.contains("2"));
    }

    #[tokio::test]
    async fn test_a2a_delegate_no_agents_returns_guidance() {
        let a2a = test_a2a();
        let agent_id = Uuid::new_v4();

        let tool = A2aDelegateTool::new(a2a.clone(), agent_id);
        let params = json!({"description": "review code", "capability": "code-review"});
        let result = tool.execute("tc", params, None).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("handle this task yourself"));
    }

    #[tokio::test]
    async fn test_a2a_send_invalid_uuid() {
        let a2a = test_a2a();
        let agent_id = Uuid::new_v4();

        let tool = A2aSendTool::new(a2a.clone(), agent_id);
        let params = json!({"target_agent_id": "not-a-uuid", "message_type": "status_update", "content": "hello"});
        let result = tool.execute("tc", params, None).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Invalid target_agent_id"));
    }

    #[tokio::test]
    async fn test_a2a_send_handshake() {
        let a2a = test_a2a();
        let my_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();

        // Register self so handshake can look up name.
        let card = crate::a2a::AgentCard::new(my_id, "me", "Test agent").with_capability("test");
        a2a.registry().register_agent(card).await.unwrap();

        let tool = A2aSendTool::new(a2a.clone(), my_id);
        let params = json!({"target_agent_id": target_id.to_string(), "message_type": "handshake", "content": "hello"});
        let result = tool.execute("tc", params, None).await.unwrap();
        assert!(result.success);

        // Verify message in queue.
        let msgs = a2a.receive_messages(target_id).await;
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_parse_priority() {
        assert!(matches!(parse_priority(Some("low")), TaskPriority::Low));
        assert!(matches!(parse_priority(Some("high")), TaskPriority::High));
        assert!(matches!(
            parse_priority(Some("critical")),
            TaskPriority::Critical
        ));
        assert!(matches!(parse_priority(None), TaskPriority::Normal));
        assert!(matches!(
            parse_priority(Some("unknown")),
            TaskPriority::Normal
        ));
    }
}
