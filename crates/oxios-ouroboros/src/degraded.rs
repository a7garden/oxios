//! Degraded-mode fallbacks for Ouroboros protocol phases.
//!
//! When the LLM provider is unavailable or returns unparseable output,
//! these fallbacks produce context-aware defaults instead of generic
//! placeholders. They preserve user intent from the available context.

use crate::AmbiguityScore;
use crate::{EvaluationResult, InterviewResult, Seed};

/// Produce a degraded interview result when LLM parsing fails.
///
/// Unlike a generic default, this uses the user's original message
/// to produce meaningful ambiguity scores.
pub fn degraded_interview(user_input: &str) -> InterviewResult {
    let has_verb = contains_action_verb(user_input);
    let has_specifics = contains_specifics(user_input);

    let goal_clarity = if has_verb { 0.5 } else { 0.2 };
    let constraint_clarity = if has_specifics { 0.5 } else { 0.2 };
    let success_clarity = 0.2;

    let mut result = InterviewResult::new();
    result.original_message = user_input.to_string();
    result.is_task = has_verb;
    if has_verb {
        result.add_exchange(
            "Could you describe the goal in more detail? What specific outcome do you expect?",
            "",
        );
    } else {
        result.chat_response = "Hello! How can I help you today?".to_string();
    }
    result.update_ambiguity(AmbiguityScore::new(
        goal_clarity,
        constraint_clarity,
        success_clarity,
    ));

    result
}

/// Produce a degraded seed when LLM generation fails.
///
/// Uses the interview context to preserve as much user intent as possible.
///
/// TODO: Connect to `generate_seed()` fallback when full integration is done.
#[allow(dead_code)]
pub fn degraded_seed(interview: &InterviewResult) -> Seed {
    let goal = if !interview.original_message.is_empty() {
        interview.original_message.clone()
    } else {
        "Task from user input".to_string()
    };

    let mut seed = Seed::new(&goal);
    seed.original_request = goal;
    seed
}

/// Produce a degraded evaluation when LLM evaluation fails.
///
/// Uses keyword matching between acceptance criteria and output.
pub fn degraded_evaluation(seed: &Seed, output: &str, mechanical_pass: bool) -> EvaluationResult {
    let output_lower = output.to_lowercase();
    let mut matched = 0;

    for criterion in &seed.acceptance_criteria {
        // Simple keyword matching: check if key nouns from criterion appear in output
        let keywords: Vec<&str> = criterion
            .split_whitespace()
            .filter(|w| w.len() > 3 && !matches!(*w, "the" | "must" | "should" | "shall"))
            .collect();
        let keyword_match = keywords
            .iter()
            .any(|kw| output_lower.contains(&kw.to_lowercase()));
        if keyword_match {
            matched += 1;
        }
    }

    let ratio = if seed.acceptance_criteria.is_empty() {
        0.5
    } else {
        matched as f64 / seed.acceptance_criteria.len() as f64
    };

    let score = if mechanical_pass { 0.7 } else { ratio * 0.6 };

    let notes = if mechanical_pass {
        vec!["Evaluation parsing failed; using mechanical + keyword fallback.".to_string()]
    } else {
        vec![
            "Evaluation parsing failed; using keyword fallback.".to_string(),
            format!(
                "Keywords matched: {}/{} criteria",
                matched,
                seed.acceptance_criteria.len()
            ),
        ]
    };

    EvaluationResult {
        mechanical_pass,
        semantic_pass: Some(ratio > 0.5),
        consensus_pass: None,
        score,
        notes,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if the message contains action verbs indicating a task.
fn contains_action_verb(text: &str) -> bool {
    let verbs = [
        "create",
        "make",
        "build",
        "fix",
        "find",
        "deploy",
        "analyze",
        "review",
        "write",
        "read",
        "run",
        "execute",
        "delete",
        "update",
        "move",
        "copy",
        "implement",
        "refactor",
        "debug",
        "test",
        "install",
        "configure",
        "setup",
    ];
    let lower = text.to_lowercase();
    verbs.iter().any(|v| lower.contains(v))
}

/// Check if the message contains specific details (paths, filenames, languages).
fn contains_specifics(text: &str) -> bool {
    // Contains a path-like pattern
    if text.contains('/') || text.contains(".rs") || text.contains(".py") || text.contains(".ts") {
        return true;
    }
    // Contains quoted strings
    if text.contains('"') || text.contains('\'') {
        return true;
    }
    // Contains specific language/framework names
    let specifics = [
        "rust",
        "python",
        "typescript",
        "javascript",
        "go",
        "java",
        "tokio",
        "react",
        "axum",
        "cargo",
        "npm",
        "docker",
    ];
    let lower = text.to_lowercase();
    specifics.iter().any(|s| lower.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_degraded_interview_with_task() {
        let result = degraded_interview("Create a new Rust file called main.rs");
        assert!(result.is_task);
        assert!(!result.original_message.is_empty());
        assert!(!result.questions.is_empty());
    }

    #[test]
    fn test_degraded_interview_with_greeting() {
        let result = degraded_interview("Hello, how are you?");
        assert!(!result.is_task);
    }

    #[test]
    fn test_degraded_seed_preserves_input() {
        let mut interview = InterviewResult::new();
        interview.original_message = "Fix the login bug".to_string();
        let seed = degraded_seed(&interview);
        assert_eq!(seed.goal, "Fix the login bug");
    }

    #[test]
    fn test_degraded_evaluation_keyword_match() {
        let seed = Seed::new("Create config.toml");
        let output = "Created config.toml with default settings";
        let result = degraded_evaluation(&seed, output, false);
        assert!(result.score > 0.0);
    }

    #[test]
    fn test_contains_action_verb() {
        assert!(contains_action_verb("Please fix the bug"));
        assert!(contains_action_verb("Create a new file"));
        assert!(!contains_action_verb("Hello there"));
    }

    #[test]
    fn test_contains_specifics() {
        assert!(contains_specifics("Edit src/main.rs"));
        assert!(contains_specifics("Use Rust and Tokio"));
        assert!(!contains_specifics("Help me with something"));
    }
}
