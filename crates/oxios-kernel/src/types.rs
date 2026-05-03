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
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Starting => write!(f, "starting"),
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Failed => write!(f, "failed"),
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
}
