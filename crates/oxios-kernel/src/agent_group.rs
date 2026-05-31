//! Agent group types for oxios orchestration.
//!
//! oxios has its own `OxiosAgentGroup` struct for managing groups of agents
//! spawned by the orchestrator (Seed splitting, state persistence, events).
//!
//! For multi-agent execution within a pipeline/parallel/orchestrated workflow,
//! use the re-exports from oxi_sdk: `SdkAgentGroup`, `SdkGroupResult`.
//! See `lib.rs` for oxi-sdk re-exports.

use chrono::Utc;
use oxios_ouroboros::Seed;
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
    /// The child seed this agent executes.
    pub seed: Seed,
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
    /// The parent seed that spawned this group.
    pub parent_seed_id: Uuid,
    /// Agents in this group.
    pub agents: Vec<OxiosGroupAgent>,
}

impl OxiosAgentGroup {
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
                    cspace_hint: parent_seed.cspace_hint.clone(),
                    original_request: parent_seed.original_request.clone(),
                    output_schema: None,
                };
                OxiosGroupAgent {
                    id: child_seed.id,
                    seed: child_seed,
                    status: OxiosAgentGroupStatus::Pending,
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
    fn test_group_new_splits_seed() {
        let parent = Seed {
            id: Uuid::new_v4(),
            goal: "Test goal".into(),
            constraints: vec!["constraint1".into()],
            acceptance_criteria: vec!["Criterion".into()],
            ontology: vec![],
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
            original_request: String::new(),
            output_schema: None,
        };

        let descriptions = vec!["subtask 1".into(), "subtask 2".into()];
        let group = OxiosAgentGroup::new(&parent, descriptions);

        assert_eq!(group.agents.len(), 2);
        assert!(group.pending_agents().len() == 2);
        assert!(!group.all_completed());
        assert_eq!(group.parent_seed_id, parent.id);
    }

    #[test]
    fn test_completion_pct_empty_group() {
        let parent = Seed {
            id: Uuid::new_v4(),
            goal: "Test".into(),
            constraints: vec![],
            acceptance_criteria: vec![],
            ontology: vec![],
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
            original_request: String::new(),
            output_schema: None,
        };

        let group = OxiosAgentGroup::new(&parent, vec![]);
        assert!((group.completion_pct() - 0.0).abs() < f64::EPSILON);
    }
}
