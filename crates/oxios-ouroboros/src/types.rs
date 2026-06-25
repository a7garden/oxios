//! Shared types used across the intent handling pipeline and kernel.

use serde::{Deserialize, Serialize};

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

/// Result of executing a directive.
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
    /// Failure classification (RFC-029). `Some` when the run failed with a
    /// classifiable provider/infra error; `None` on success, cancellation,
    /// abort, or unclassified failure. P2's RecoveryCoordinator reads this
    /// to choose the recovery strategy.
    ///
    /// `#[serde(default, skip_serializing_if)]` keeps existing JSON payloads
    /// (without the field) round-tripping cleanly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<crate::resilience::FailureClass>,
    /// Agent conversation state captured at the point of failure (RFC-029
    /// P2b). `Some` when the agent had accumulated state before a provider
    /// failure — allows the RecoveryCoordinator to restore into a new
    /// agent (with a different model) and continue rather than restarting.
    ///
    /// Serialized from `Agent::export_state()`. `None` on success, or when
    /// the failure occurred before any state was accumulated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore_state: Option<serde_json::Value>,
}

/// Single option for a structured interview question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewOptionOutput {
    /// Stable identifier for the answer payload (e.g. "points", "ko").
    pub value: String,
    /// Human-readable label rendered as a chip/button.
    pub label: String,
    /// Optional longer description shown as a tooltip.
    #[serde(default)]
    pub description: String,
}

/// One structured question produced by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewQuestionOutput {
    /// Short identifier used as the answer key (e.g. "q1", "q2").
    pub id: String,
    /// The question text (also present in the parallel `questions` array).
    pub text: String,
    /// Question kind — drives the frontend widget selection.
    #[serde(default = "default_question_kind")]
    pub kind: String,
    /// Choice options (empty for free_text / yes_no).
    #[serde(default)]
    pub options: Vec<InterviewOptionOutput>,
}

fn default_question_kind() -> String {
    "free_text".to_string()
}
