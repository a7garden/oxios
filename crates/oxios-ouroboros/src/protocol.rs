//! Ouroboros protocol trait and phase definitions.
//!
//! The protocol enforces the five-phase lifecycle:
//! interview → seed → execute → evaluate → evolve.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::seed::Seed;
use crate::{EvaluationResult, InterviewResult, ouroboros_engine::InterviewQuestionOutput};

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

/// Record of a single tool call during execution.
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
    /// Provider-specific tool call ID for start/end correlation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool_call_id: String,
    /// Timestamp when the tool call started (UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
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
    /// Tool calls recorded during execution.
    #[serde(default)]
    pub tool_calls: Vec<ToolCallRecord>,
    /// Total input tokens consumed during execution.
    #[serde(default)]
    pub tokens_input: u64,
    /// Total output tokens generated during execution.
    #[serde(default)]
    pub tokens_output: u64,
    /// Model ID used for this execution.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_id: String,
}

/// The Ouroboros protocol trait.
///
/// Implementations drive the full lifecycle from user input to
/// verified result. Each phase must complete before the next begins.
#[async_trait]
pub trait OuroborosProtocol: Send + Sync {
    /// Conduct an interview to clarify user intent.
    async fn interview(&self, user_input: &str) -> Result<InterviewResult>;

    /// Produce a parallel structured form of the interview questions
    /// for the interactive Web UI. Returns `Ok(None)` when the LLM did
    /// not produce structured output (e.g. the request was chat) or
    /// the JSON was malformed — the frontend falls back to markdown
    /// rendering of the plain `questions`.
    async fn interview_structured(
        &self,
        user_input: &str,
    ) -> Result<Option<Vec<InterviewQuestionOutput>>>;

    /// Generate an immutable seed from interview results.
    async fn generate_seed(&self, interview: &InterviewResult) -> Result<Seed>;

    /// Execute a seed, running the tool-calling loop.
    async fn execute(&self, seed: &Seed) -> Result<ExecutionResult>;

    /// Evaluate the result of execution against the seed's criteria.
    async fn evaluate(&self, seed: &Seed, execution: &ExecutionResult) -> Result<EvaluationResult>;

    /// Evolve the seed based on evaluation, returning a new seed if needed.
    async fn evolve(&self, seed: &Seed, evaluation: &EvaluationResult) -> Result<Option<Seed>>;

    /// Inject a persona system prompt for voice customization.
    /// When set, this is prepended to every LLM call in all phases.
    #[inline]
    fn set_persona_prompt(&self, _prompt: Option<String>) {}
}
