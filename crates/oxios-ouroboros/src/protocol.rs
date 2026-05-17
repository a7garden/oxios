//! Ouroboros protocol trait and phase definitions.
//!
//! The protocol enforces the five-phase lifecycle:
//! interview → seed → execute → evaluate → evolve.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::seed::Seed;
use crate::{EvaluationResult, InterviewResult};

/// The phases of the Ouroboros lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Clarify intent through questions and answers.
    Interview,
    /// Generate an immutable specification.
    Seed,
    /// Run the tool-calling loop per spec.
    Execute,
    /// Three-stage verification of results.
    Evaluate,
    /// Feed evaluation back as input for the next loop.
    Evolve,
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Interview => write!(f, "interview"),
            Phase::Seed => write!(f, "seed"),
            Phase::Execute => write!(f, "execute"),
            Phase::Evaluate => write!(f, "evaluate"),
            Phase::Evolve => write!(f, "evolve"),
        }
    }
}

/// Result of executing a seed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// The output produced by execution.
    pub output: String,
    /// Number of steps completed during execution.
    pub steps_completed: usize,
    /// Whether execution completed successfully.
    pub success: bool,
}

/// The Ouroboros protocol trait.
///
/// Implementations drive the full lifecycle from user input to
/// verified result. Each phase must complete before the next begins.
#[async_trait]
pub trait OuroborosProtocol: Send + Sync {
    /// Conduct an interview to clarify user intent.
    async fn interview(&self, user_input: &str) -> Result<InterviewResult>;

    /// Generate an immutable seed from interview results.
    async fn generate_seed(&self, interview: &InterviewResult) -> Result<Seed>;

    /// Execute a seed, running the tool-calling loop.
    async fn execute(&self, seed: &Seed) -> Result<ExecutionResult>;

    /// Evaluate the result of execution against the seed's criteria.
    async fn evaluate(&self, seed: &Seed, execution: &ExecutionResult) -> Result<EvaluationResult>;

    /// Evolve the seed based on evaluation, returning a new seed if needed.
    async fn evolve(&self, seed: &Seed, evaluation: &EvaluationResult) -> Result<Option<Seed>>;

    /// Generate a direct conversational response without entering
    /// the full Ouroboros pipeline. Used for greetings, small talk,
    /// and non-task messages.
    async fn chat(&self, user_message: &str) -> Result<String> {
        // Default: just echo back a generic response
        Ok(format!("Received: {}", user_message))
    }

    /// Inject a persona system prompt for voice customization.
    /// When set, this is prepended to every LLM call in all phases.
    #[inline]
    fn set_persona_prompt(&self, _prompt: Option<String>) {}
}
