//! Evaluators — assess benchmark task outputs against assertions.
//!
//! The evaluation pipeline:
//! 1. Structural — check phase_reached, evaluation_passed, IDs, duration
//! 2. Content — check response text for contains/not-contains/regex
//! 3. (Future) LLM-Judge — subjective quality assessment
//!
//! Each assertion is evaluated independently. Score is calculated as
//! the percentage of passing assertions. A task passes if score >= 80%
//! AND all structural assertions pass.

// Evaluation logic is in crate::task::Assertion::evaluate()
// and crate::runner::evaluate_task() for orchestration.
// This module is reserved for future LLM-Judge evaluator.

/// Weight categories for assertion scoring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssertionCategory {
    /// Structural field checks (phase, evaluation, IDs).
    Structural,
    /// Response text content checks.
    Content,
    /// LLM-Judge subjective quality.
    LlmJudge,
    /// Custom Rust evaluator.
    Custom,
}

impl AssertionCategory {
    /// Default weight for this category.
    pub fn default_weight(&self) -> f64 {
        match self {
            AssertionCategory::Structural => 2.0,
            AssertionCategory::Content => 1.0,
            AssertionCategory::LlmJudge => 0.5,
            AssertionCategory::Custom => 1.5,
        }
    }
}
