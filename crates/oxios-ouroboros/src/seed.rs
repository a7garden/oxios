//! Seed definition and ambiguity scoring.
//!
//! A Seed is an immutable specification. Once created, it does not change.
//! To modify, create a new Seed via the evolve phase.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a seed.
pub type SeedId = uuid::Uuid;

/// An immutable specification for agent execution.
///
/// The Seed captures the goal, constraints, acceptance criteria, and
/// relevant ontology entities. It is the contract between the user's
/// intent and the agent's execution.
///
/// Seeds are versioned via the `generation` field. Gen 0 is the initial
/// seed from `generate_seed()`. Each successful evolution increments
/// generation. Lineage is tracked via `parent_seed_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seed {
    /// Unique identifier for this seed.
    pub id: SeedId,
    /// The goal this seed aims to achieve.
    pub goal: String,
    /// Constraints that must be respected during execution.
    pub constraints: Vec<String>,
    /// Measurable criteria for acceptance.
    pub acceptance_criteria: Vec<String>,
    /// Named entities relevant to the problem domain.
    pub ontology: Vec<Entity>,
    /// Timestamp of seed creation.
    pub created_at: DateTime<Utc>,
    /// Evolution generation counter (0 = initial seed).
    #[serde(default)]
    pub generation: u32,
    /// Parent seed ID if this seed was evolved from another.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_seed_id: Option<SeedId>,
    /// Hint for the capability system to determine the agent's CSpace.
    ///
    /// Accepts a known template name ("worker", "standard", "operator",
    /// "supervisor") or a JSON string describing custom capabilities.
    /// When `None`, the kernel falls back to the persona role or the
    /// default "worker" template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cspace_hint: Option<String>,

    /// The user's original message, preserved verbatim.
    ///
    /// For complex tasks this differs from `goal` (which is the LLM's
    /// crystallized version). For simple tasks, goal == original_request.
    /// The agent runtime injects this into the system prompt so the agent
    /// sees the user's exact wording, including language-specific nuance.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub original_request: String,

    /// Optional JSON Schema for structured output validation.
    ///
    /// When set, the orchestrator uses `oxi_sdk::StructuredOutput` to
    /// extract and validate the agent's response against this schema.
    /// This replaces the simple boolean `success` check with proper
    /// schema-based evaluation.
    ///
    /// Example: `{"type": "object", "required": ["files_changed"]}`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    /// Project ID detected by the orchestrator, passed through to AgentInfo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<uuid::Uuid>,
}

impl Seed {
    /// Creates a new seed with the given goal and auto-generated ID.
    ///
    /// Generation is set to 0 and parent_seed_id is None.
    ///
    /// # Example
    ///
    /// ```
    /// use oxios_ouroboros::Seed;
    ///
    /// let seed = Seed::new("Build a web server");
    /// assert!(!seed.goal.is_empty());
    /// assert_eq!(seed.generation, 0);
    /// assert!(seed.parent_seed_id.is_none());
    /// ```
    /// Creates a new seed with the given goal and auto-generated ID.
    ///
    /// Generation is set to 0 and parent_seed_id is None.
    ///
    /// # Example
    ///
    /// ```
    /// use oxios_ouroboros::Seed;
    ///
    /// let seed = Seed::new("Build a web server");
    /// assert!(!seed.goal.is_empty());
    /// assert_eq!(seed.generation, 0);
    /// assert!(seed.parent_seed_id.is_none());
    /// ```
    pub fn new(goal: impl Into<String>) -> Self {
        let goal = goal.into();
        Self {
            id: uuid::Uuid::new_v4(),
            goal: goal.clone(),
            original_request: goal,
            constraints: Vec::new(),
            acceptance_criteria: Vec::new(),
            ontology: Vec::new(),
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
            output_schema: None,
            project_id: None,
        }
    }

    /// Creates a lightweight seed directly from a user message.
    ///
    /// Used for simple, unambiguous tasks where the user's exact wording
    /// is sufficient as-is — no Seed-generation LLM call needed.
    /// The message becomes both `goal` and `original_request` verbatim.
    pub fn from_message(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            id: uuid::Uuid::new_v4(),
            goal: msg.clone(),
            original_request: msg,
            constraints: Vec::new(),
            acceptance_criteria: Vec::new(),
            ontology: Vec::new(),
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
            output_schema: None,
            project_id: None,
        }
    }

    /// Creates a new evolved seed from a parent seed.
    ///
    /// The new seed has generation = parent.generation + 1 and
    /// parent_seed_id = parent.id.
    pub fn evolved_from(parent: &Seed) -> Seed {
        Self {
            id: uuid::Uuid::new_v4(),
            goal: parent.goal.clone(),
            original_request: parent.original_request.clone(),
            constraints: parent.constraints.clone(),
            acceptance_criteria: parent.acceptance_criteria.clone(),
            ontology: parent.ontology.clone(),
            created_at: Utc::now(),
            generation: parent.generation + 1,
            parent_seed_id: Some(parent.id),
            cspace_hint: parent.cspace_hint.clone(),
            output_schema: parent.output_schema.clone(),
            project_id: parent.project_id,
        }
    }
}

/// A named entity in the problem domain ontology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Human-readable name of the entity.
    pub name: String,
    /// Classification of the entity (e.g., "service", "data", "user").
    pub entity_type: String,
    /// Description of the entity's role in the domain.
    pub description: String,
}

/// Score measuring how ambiguous a seed specification is.
///
/// Lower ambiguity means the specification is clearer and more
/// ready for execution. The threshold for readiness is 0.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbiguityScore {
    /// Clarity of the stated goal (0.0 = unclear, 1.0 = crystal clear).
    pub goal_clarity: f64,
    /// Clarity of the constraints (0.0 = unclear, 1.0 = crystal clear).
    pub constraint_clarity: f64,
    /// Clarity of the success criteria (0.0 = unclear, 1.0 = crystal clear).
    pub success_criteria: f64,
}

impl AmbiguityScore {
    /// Creates a new ambiguity score with the given clarity values.
    pub fn new(goal_clarity: f64, constraint_clarity: f64, success_criteria: f64) -> Self {
        Self {
            goal_clarity: goal_clarity.clamp(0.0, 1.0),
            constraint_clarity: constraint_clarity.clamp(0.0, 1.0),
            success_criteria: success_criteria.clamp(0.0, 1.0),
        }
    }

    /// Computes the overall ambiguity (0.0 = clear, 1.0 = fully ambiguous).
    ///
    /// Weighted: goal 40%, constraints 30%, success criteria 30%.
    ///
    /// # Example
    ///
    /// ```
    /// use oxios_ouroboros::AmbiguityScore;
    ///
    /// let score = AmbiguityScore::new(1.0, 0.8, 0.9);
    /// assert!(score.ambiguity() < 0.2); // low ambiguity = ready
    /// assert!(score.is_ready());
    /// ```
    pub fn ambiguity(&self) -> f64 {
        1.0 - (self.goal_clarity * 0.4
            + self.constraint_clarity * 0.3
            + self.success_criteria * 0.3)
    }

    /// Returns true if the ambiguity is low enough to proceed to execution.
    pub fn is_ready(&self) -> bool {
        self.ambiguity() <= 0.2
    }
}

impl Default for AmbiguityScore {
    fn default() -> Self {
        // Maximum ambiguity until evaluated.
        Self {
            goal_clarity: 0.0,
            constraint_clarity: 0.0,
            success_criteria: 0.0,
        }
    }
}
