//! Core types for the Oxios kernel.
//!
//! Defines agent identity, status, and metadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for an agent instance.
pub type AgentId = uuid::Uuid;

/// Current status of an agent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is being initialized.
    Starting,
    /// Agent is actively executing tasks.
    Running,
    /// Agent is alive but not currently working.
    Idle,
    /// Agent has been stopped.
    Stopped,
    /// Agent has encountered an error.
    Failed,
    /// Agent finished execution successfully.
    Completed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Starting => write!(f, "starting"),
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Failed => write!(f, "failed"),
            AgentStatus::Completed => write!(f, "completed"),
        }
    }
}

/// Metadata about an agent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique identifier for this agent.
    pub id: AgentId,
    /// Human-readable name of the agent.
    pub name: String,
    /// Current status of the agent.
    pub status: AgentStatus,
    /// Timestamp when the agent was created.
    pub created_at: DateTime<Utc>,
    /// The seed this agent was forked from, if any.
    pub seed_id: Option<uuid::Uuid>,
    /// Project ID detected by the orchestrator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<uuid::Uuid>,
    /// Timestamp when execution started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when execution completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message if the agent failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of tool call steps completed.
    #[serde(default)]
    pub steps_completed: usize,
    /// Number of total steps (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps_total: Option<usize>,
    /// Tool calls recorded during execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallRecord>,
    /// Total input tokens consumed.
    #[serde(default)]
    pub tokens_input: u64,
    /// Total output tokens generated.
    #[serde(default)]
    pub tokens_output: u64,
    /// Estimated cost in USD.
    #[serde(default)]
    pub cost_usd: f64,
    /// Model ID used for execution.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_id: String,
    /// Session ID that spawned this agent (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Record of a single tool call during agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool name (e.g. "read", "bash", "grep").
    pub tool: String,
    /// Input parameters or invocation summary.
    pub input: String,
    /// Output or result summary.
    pub output: String,
    /// Duration of the tool call in milliseconds.
    pub duration_ms: u64,
    /// Whether the tool call returned an error.
    #[serde(default)]
    pub is_error: bool,
    /// Provider-specific tool call ID.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool_call_id: String,
    /// Timestamp when the tool call started (UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_display_all_variants() {
        assert_eq!(AgentStatus::Starting.to_string(), "starting");
        assert_eq!(AgentStatus::Running.to_string(), "running");
        assert_eq!(AgentStatus::Idle.to_string(), "idle");
        assert_eq!(AgentStatus::Stopped.to_string(), "stopped");
        assert_eq!(AgentStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_agent_status_equality() {
        assert_eq!(AgentStatus::Running, AgentStatus::Running);
        assert_ne!(AgentStatus::Running, AgentStatus::Idle);
    }

    #[test]
    fn test_agent_status_serialization_roundtrip() {
        for status in [
            AgentStatus::Starting,
            AgentStatus::Running,
            AgentStatus::Idle,
            AgentStatus::Stopped,
            AgentStatus::Failed,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let restored: AgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, restored);
        }
    }

    #[test]
    fn test_agent_info_construction() {
        let id = AgentId::new_v4();
        let seed_id = uuid::Uuid::new_v4();
        let now = Utc::now();

        let info = AgentInfo {
            id,
            name: "test-agent".to_string(),
            status: AgentStatus::Running,
            created_at: now,
            seed_id: Some(seed_id),
            project_id: None,
            started_at: None,
            completed_at: None,
            error: None,
            steps_completed: 0,
            steps_total: None,
            tool_calls: vec![],
            tokens_input: 0,
            tokens_output: 0,
            cost_usd: 0.0,
            model_id: String::new(),
            session_id: None,
        };

        assert_eq!(info.id, id);
        assert_eq!(info.name, "test-agent");
        assert_eq!(info.status, AgentStatus::Running);
        assert_eq!(info.created_at, now);
        assert_eq!(info.seed_id, Some(seed_id));
    }

    #[test]
    fn test_agent_info_serialization_roundtrip() {
        let info = AgentInfo {
            id: AgentId::new_v4(),
            name: "serializer".to_string(),
            status: AgentStatus::Idle,
            created_at: Utc::now(),
            seed_id: None,
            project_id: None,
            started_at: None,
            completed_at: None,
            error: None,
            steps_completed: 3,
            steps_total: Some(5),
            tool_calls: vec![],
            tokens_input: 100,
            tokens_output: 50,
            cost_usd: 0.002,
            model_id: String::new(),
            session_id: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let restored: AgentInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, info.id);
        assert_eq!(restored.name, info.name);
        assert_eq!(restored.status, info.status);
        assert_eq!(restored.seed_id, None);
        assert_eq!(restored.steps_completed, 3);
    }

    #[test]
    fn test_agent_status_copy() {
        let status = AgentStatus::Running;
        let copied = status; // Copy semantics
        assert_eq!(status, copied); // status is still valid because Copy
    }
}
