//! Agent group: manages parallel execution of multiple agents.
//!
//! An AgentGroup takes a parent Seed, splits it into child seeds,
//! and tracks their execution status and results.

use chrono::Utc;
use oxios_ouroboros::Seed;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an agent within a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentGroupStatus {
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
pub struct GroupAgent {
    /// Unique ID for this group agent.
    pub id: Uuid,
    /// The child seed this agent executes.
    pub seed: Seed,
    /// Current status.
    pub status: AgentGroupStatus,
    /// Result output (when completed).
    pub result: Option<String>,
}

/// A group of agents executing in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGroup {
    /// Unique group ID.
    pub id: Uuid,
    /// The parent seed that spawned this group.
    pub parent_seed_id: Uuid,
    /// Agents in this group.
    pub agents: Vec<GroupAgent>,
}

impl AgentGroup {
    /// Create a new agent group by splitting a parent seed into subtasks.
    pub fn new(parent_seed: &Seed, subtask_descriptions: Vec<String>) -> Self {
        let agents = subtask_descriptions
            .into_iter()
            .map(|desc| {
                let child_seed = Seed {
                    id: Uuid::new_v4(),
                    goal: desc,
                    constraints: parent_seed.constraints.clone(),
                    acceptance_criteria: vec!["Task completes successfully".into()],
                    ontology: parent_seed.ontology.clone(),
                    created_at: Utc::now(),
                    generation: parent_seed.generation + 1,
                    parent_seed_id: Some(parent_seed.id),
                };
                GroupAgent {
                    id: Uuid::new_v4(),
                    seed: child_seed,
                    status: AgentGroupStatus::Pending,
                    result: None,
                }
            })
            .collect();

        Self {
            id: Uuid::new_v4(),
            parent_seed_id: parent_seed.id,
            agents,
        }
    }

    /// Check if all agents are done (completed or failed).
    pub fn all_done(&self) -> bool {
        self.agents
            .iter()
            .all(|a| a.status == AgentGroupStatus::Completed || a.status == AgentGroupStatus::Failed)
    }

    /// Get combined results from all completed agents.
    pub fn combined_results(&self) -> String {
        self.agents
            .iter()
            .filter(|a| a.status == AgentGroupStatus::Completed)
            .filter_map(|a| a.result.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }

    /// Count agents by status.
    pub fn count_by_status(&self, status: AgentGroupStatus) -> usize {
        self.agents.iter().filter(|a| a.status == status).count()
    }
}
