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
}

fn default_is_task() -> bool {
    true
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
}

impl Default for InterviewResult {
    fn default() -> Self {
        Self::new()
    }
}
