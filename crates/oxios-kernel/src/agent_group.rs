//! Agent group types for oxios orchestration.
//!
//! Defines the data structures for multi-agent groups persisted to the state
//! store. The group creation logic (`delegate_subtasks`) was removed during
//! the RFC-027 migration; these types remain for reading historical data
//! from the state store via the `/api/agent-groups` API.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an agent within a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OxiosAgentGroupStatus {
    /// Agent is pending execution.
    Pending,
    /// Agent is currently running.
    Running,
    /// Agent completed successfully.
    Completed,
    /// Agent failed.
    Failed,
}

/// A single agent's entry in a group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OxiosGroupAgent {
    /// Unique ID for this group agent.
    pub id: Uuid,
    /// The goal this agent was assigned.
    pub goal: String,
    /// Current status.
    pub status: OxiosAgentGroupStatus,
    /// Result output (when completed).
    pub result: Option<String>,
}

/// A group of agents executing in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OxiosAgentGroup {
    /// Unique group ID.
    pub id: Uuid,
    /// The parent ID that spawned this group.
    pub parent_seed_id: Uuid,
    /// Agents in this group.
    pub agents: Vec<OxiosGroupAgent>,
}

impl OxiosAgentGroup {
    /// Get all pending agents.
    pub fn pending_agents(&self) -> Vec<&OxiosGroupAgent> {
        self.agents
            .iter()
            .filter(|a| a.status == OxiosAgentGroupStatus::Pending)
            .collect()
    }

    /// Get all completed agents.
    pub fn completed_agents(&self) -> Vec<&OxiosGroupAgent> {
        self.agents
            .iter()
            .filter(|a| a.status == OxiosAgentGroupStatus::Completed)
            .collect()
    }

    /// Get all failed agents.
    pub fn failed_agents(&self) -> Vec<&OxiosGroupAgent> {
        self.agents
            .iter()
            .filter(|a| a.status == OxiosAgentGroupStatus::Failed)
            .collect()
    }

    /// Check if all agents in the group have completed.
    pub fn all_completed(&self) -> bool {
        self.agents
            .iter()
            .all(|a| a.status == OxiosAgentGroupStatus::Completed)
    }

    /// Check if any agent has failed.
    pub fn any_failed(&self) -> bool {
        self.agents
            .iter()
            .any(|a| a.status == OxiosAgentGroupStatus::Failed)
    }

    /// Get completion percentage.
    pub fn completion_pct(&self) -> f64 {
        if self.agents.is_empty() {
            return 0.0;
        }
        let completed = self
            .agents
            .iter()
            .filter(|a| a.status == OxiosAgentGroupStatus::Completed)
            .count();
        completed as f64 / self.agents.len() as f64
    }

    /// Combine results from all completed agents.
    pub fn combined_results(&self) -> String {
        self.completed_agents()
            .iter()
            .filter_map(|a| a.result.as_ref())
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_pct_empty_group() {
        let group = OxiosAgentGroup {
            id: Uuid::new_v4(),
            parent_seed_id: Uuid::new_v4(),
            agents: vec![],
        };
        assert!((group.completion_pct() - 0.0).abs() < f64::EPSILON);
    }
}
