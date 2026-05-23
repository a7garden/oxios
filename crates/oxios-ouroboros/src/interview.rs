//! Interview phase: question generation and answer collection.
//!
//! The interview phase clarifies user intent until ambiguity is low
//! enough to generate a seed (ambiguity ≤ 0.2).

use serde::{Deserialize, Serialize};

use crate::seed::AmbiguityScore;

/// Result of an interview session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewResult {
    /// The original user message that triggered this interview.
    #[serde(default)]
    pub original_message: String,
    /// Clarifying questions asked during the interview.
    pub questions: Vec<String>,
    /// Answers collected for each question.
    pub answers: Vec<String>,
    /// Current ambiguity score after the interview.
    pub ambiguity: AmbiguityScore,
    /// Whether the interview has gathered enough clarity for seed generation.
    pub ready_for_seed: bool,
    /// Whether this message is a task requiring execution.
    /// If false, the message is conversational and should get a direct response.
    #[serde(default = "default_is_task")]
    pub is_task: bool,
    /// Direct conversational response when is_task = false.
    #[serde(default)]
    pub chat_response: String,
    /// Full conversation history for multi-turn context.
    /// Each exchange is a user message + agent response (questions or chat).
    #[serde(default)]
    pub conversation_history: Vec<Exchange>,
    /// Task complexity: "simple" for clear single-action requests,
    /// "complex" for ambiguous or multi-step tasks.
    /// Defaults to "complex" (safe default).
    #[serde(default = "default_complexity")]
    pub complexity: String,
}

/// A single exchange in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Exchange {
    /// The user's message.
    pub user: String,
    /// The agent's response (questions asked or chat response).
    pub agent: String,
}

fn default_is_task() -> bool {
    true
}

fn default_complexity() -> String {
    "complex".to_string()
}

impl InterviewResult {
    /// Creates a new empty interview result with maximum ambiguity.
    pub fn new() -> Self {
        Self {
            original_message: String::new(),
            questions: Vec::new(),
            answers: Vec::new(),
            ambiguity: AmbiguityScore::default(),
            ready_for_seed: false,
            is_task: true,
            chat_response: String::new(),
            conversation_history: Vec::new(),
            complexity: default_complexity(),
        }
    }

    /// Adds a question-answer pair to the interview.
    pub fn add_exchange(&mut self, question: impl Into<String>, answer: impl Into<String>) {
        self.questions.push(question.into());
        self.answers.push(answer.into());
    }

    /// Updates the ambiguity score and readiness.
    pub fn update_ambiguity(&mut self, score: AmbiguityScore) {
        self.ready_for_seed = score.is_ready();
        self.ambiguity = score;
    }

    /// Adds a user-agent exchange to the conversation history.
    pub fn add_to_history(&mut self, user: impl Into<String>, agent: impl Into<String>) {
        self.conversation_history.push(Exchange {
            user: user.into(),
            agent: agent.into(),
        });
    }
}

impl Default for InterviewResult {
    fn default() -> Self {
        Self::new()
    }
}
