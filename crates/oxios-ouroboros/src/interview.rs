//! Interview phase: question generation and answer collection.
//!
//! The interview phase clarifies user intent until ambiguity is low
//! enough to generate a seed (ambiguity ≤ 0.2).

use serde::{Deserialize, Serialize};

use crate::seed::AmbiguityScore;

/// Result of an interview session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewResult {
    /// Clarifying questions asked during the interview.
    pub questions: Vec<String>,
    /// Answers collected for each question.
    pub answers: Vec<String>,
    /// Current ambiguity score after the interview.
    pub ambiguity: AmbiguityScore,
    /// Whether the interview has gathered enough clarity for seed generation.
    pub ready_for_seed: bool,
}

impl InterviewResult {
    /// Creates a new empty interview result with maximum ambiguity.
    pub fn new() -> Self {
        Self {
            questions: Vec::new(),
            answers: Vec::new(),
            ambiguity: AmbiguityScore::default(),
            ready_for_seed: false,
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
