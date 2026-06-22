//! Directive: the task specification for the agent.
//!
//! A Directive is built either from the raw user message (Trivial tasks)
//! or from a crystallize LLM call (Substantial tasks). It carries everything
//! the agent needs to execute and (optionally) what the reviewer checks against.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the agent should do. Built from either the raw message (Trivial)
/// or a crystallize LLM call (Substantial).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Directive {
    /// The goal the agent aims to achieve.
    pub goal: String,

    /// The user's original message, preserved verbatim for language fidelity
    /// and exact filenames/paths.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub original_request: String,

    /// Constraints the agent must respect during execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,

    /// Verifiable acceptance criteria. Injected into the agent's system prompt
    /// AND checked by the review pass. Empty = no review (Trivial).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,

    /// Optional JSON Schema for structured output validation.
    /// When set, the reviewer checks JSON conformance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
}

impl Directive {
    /// Create a lightweight directive from a user message verbatim.
    /// Used for Trivial tasks where the message is sufficient as-is.
    pub fn from_message(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            goal: msg.clone(),
            original_request: msg,
            constraints: Vec::new(),
            acceptance_criteria: Vec::new(),
            output_schema: None,
        }
    }

    /// Whether this directive has criteria worth reviewing.
    pub fn needs_review(&self) -> bool {
        !self.acceptance_criteria.is_empty() || self.output_schema.is_some()
    }
}

/// The execution environment. Resolved by the orchestrator independently
/// of the task directive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecEnv {
    /// Rendered workspace context (active Mounts, project instructions, relevant memories).
    pub workspace_context: Option<String>,

    /// Resolved filesystem paths from active Mounts. paths[0] of the primary
    /// Mount is the CWD; every path is added to the agent's allowed_paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mount_paths: Vec<PathBuf>,

    /// Project ID detected by the orchestrator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<uuid::Uuid>,

    /// Hint for the capability system (CSpace template name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cspace_hint: Option<String>,
}

/// The result of reviewing an execution against a directive's criteria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    /// Whether the execution passed all criteria.
    pub passed: bool,

    /// 0.0–1.0 confidence score.
    pub score: f64,

    /// Human-readable notes (✓/✗ prefix convention).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,

    /// Specific gaps where criteria were not met — fed into retry context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<String>,
}

impl Verdict {
    /// Whether the verdict indicates a pass.
    pub fn all_passed(&self) -> bool {
        self.passed
    }
}

/// A single conversation exchange: user message → agent response (or question).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exchange {
    /// User's message.
    pub user: String,
    /// Agent's response (or clarifying question).
    pub agent: String,
}

/// Message processing context for the intent handler.
#[derive(Debug, Clone)]
pub struct MsgCtx {
    /// Session ID for history lookup.
    pub session_id: String,
    /// Previous exchanges in this session (from the state store).
    /// Provides clarify context and topic-shift detection.
    pub history: Vec<Exchange>,
    /// Comma-separated project IDs.
    pub project_ids: Option<String>,
    /// Comma-separated Mount IDs.
    pub mount_ids: Option<String>,
    /// User identifier.
    pub user_id: String,
}

/// Timestamp of directive creation (used for session metadata, not stored on the directive itself).
pub type DirectiveTimestamp = DateTime<Utc>;
