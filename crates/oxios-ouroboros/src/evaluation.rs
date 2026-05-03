//! Three-stage evaluation of execution results.
//!
//! Evaluation proceeds through three stages:
//! 1. **Mechanical** — Does the output satisfy acceptance criteria literally?
//! 2. **Semantic** — Does the output actually solve the user's intent?
//! 3. **Consensus** — Would multiple evaluators agree?

use serde::{Deserialize, Serialize};

/// Result of evaluating an execution against its seed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Stage 1: mechanical acceptance criteria check.
    pub mechanical_pass: bool,
    /// Stage 2: semantic correctness check (None if not yet evaluated).
    pub semantic_pass: Option<bool>,
    /// Stage 3: consensus check (None if not yet evaluated).
    pub consensus_pass: Option<bool>,
    /// Overall score (0.0 to 1.0).
    pub score: f64,
    /// Notes from each evaluation stage.
    pub notes: Vec<String>,
}

impl EvaluationResult {
    /// Creates a new evaluation result with only the mechanical stage completed.
    pub fn mechanical_only(pass: bool, score: f64) -> Self {
        Self {
            mechanical_pass: pass,
            semantic_pass: None,
            consensus_pass: None,
            score,
            notes: Vec::new(),
        }
    }

    /// Returns true if all completed evaluation stages have passed.
    pub fn all_passed(&self) -> bool {
        self.mechanical_pass
            && self.semantic_pass.unwrap_or(true)
            && self.consensus_pass.unwrap_or(true)
    }
}
