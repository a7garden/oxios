//! Intent assessment: the single routing decision for every user message.
//!
//! [`Assessment`] is the output of the `assess` LLM call. It determines
//! whether a message is a conversation (no agent), needs clarification
//! (ask before acting), or is a task (proceed to execution).

use serde::{Deserialize, Serialize};

/// A structured question for the interactive Web UI interview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    /// Unique question identifier.
    pub id: String,
    /// Question text in the user's language.
    pub text: String,
    /// How the user can answer.
    pub kind: QuestionKind,
    /// Available options (empty for free_text).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<QuestionOption>,
}

/// How a question can be answered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    /// Open-ended text input.
    FreeText,
    /// Pick one option.
    SingleChoice,
    /// Pick multiple options.
    MultiChoice,
    /// Yes/no.
    YesNo,
}

/// A selectable option for choice questions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    /// Internal value (stored as the answer).
    pub value: String,
    /// Display label shown to the user.
    pub label: String,
}

/// The scope of a task: how much process it warrants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Self-contained request. Message verbatim becomes the directive.
    /// No crystallization, no review.
    /// Example: "read package.json", "what's the weather"
    Trivial,

    /// Multi-step or high-stakes. Crystallize into a structured directive.
    /// Execute, then review. Optionally retry.
    /// Example: "refactor the auth system", "migrate all tests to Vitest"
    Substantial,
}

/// The result of assessing one user message. The only routing decision.
#[derive(Debug, Clone)]
pub enum Assessment {
    /// Greeting, small talk, capability question — respond without spawning an agent.
    Conversation(String),

    /// Genuine task but ambiguous — ask before executing.
    Clarify {
        /// Structured questions to ask the user.
        questions: Vec<Question>,
    },

    /// Clear task — proceed to execution. Scope controls depth.
    Task(Scope),
}

impl Assessment {
    /// Whether this assessment requires an agent to be spawned.
    pub fn needs_agent(&self) -> bool {
        matches!(self, Assessment::Task(_))
    }

    /// Whether this assessment returns a conversational reply.
    pub fn conversation_reply(&self) -> Option<&str> {
        match self {
            Assessment::Conversation(reply) => Some(reply),
            _ => None,
        }
    }
}
