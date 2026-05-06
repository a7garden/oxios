//! Ouroboros protocol types and parse_json unit tests — no LLM required.

use oxios_ouroboros::evaluation::EvaluationResult;
use oxios_ouroboros::interview::InterviewResult;
use oxios_ouroboros::protocol::Phase;
use oxios_ouroboros::seed::AmbiguityScore;

// ---------------------------------------------------------------------------
// Phase
// ---------------------------------------------------------------------------

#[test]
fn test_phase_display() {
    assert_eq!(Phase::Interview.to_string(), "interview");
    assert_eq!(Phase::Seed.to_string(), "seed");
    assert_eq!(Phase::Execute.to_string(), "execute");
    assert_eq!(Phase::Evaluate.to_string(), "evaluate");
    assert_eq!(Phase::Evolve.to_string(), "evolve");
}

// ---------------------------------------------------------------------------
// EvaluationResult
// ---------------------------------------------------------------------------

#[test]
fn test_evaluation_mechanical_only_pass() {
    let result = EvaluationResult::mechanical_only(true, 0.9);
    assert!(result.mechanical_pass);
    assert!(result.semantic_pass.is_none());
    assert!(result.consensus_pass.is_none());
    assert!(result.all_passed()); // None counts as true
}

#[test]
fn test_evaluation_mechanical_only_fail() {
    let result = EvaluationResult::mechanical_only(false, 0.3);
    assert!(!result.mechanical_pass);
    assert!(!result.all_passed());
}

#[test]
fn test_evaluation_all_stages_pass() {
    let result = EvaluationResult {
        mechanical_pass: true,
        semantic_pass: Some(true),
        consensus_pass: Some(true),
        score: 0.95,
        notes: vec![],
    };
    assert!(result.all_passed());
}

#[test]
fn test_evaluation_semantic_fails() {
    let result = EvaluationResult {
        mechanical_pass: true,
        semantic_pass: Some(false),
        consensus_pass: None,
        score: 0.5,
        notes: vec![],
    };
    assert!(!result.all_passed());
}

#[test]
fn test_evaluation_consensus_none_counts_as_pass() {
    let result = EvaluationResult {
        mechanical_pass: true,
        semantic_pass: Some(true),
        consensus_pass: None,
        score: 0.8,
        notes: vec![],
    };
    assert!(result.all_passed());
}

// ---------------------------------------------------------------------------
// InterviewResult
// ---------------------------------------------------------------------------

#[test]
fn test_interview_result_default() {
    let result = InterviewResult::new();
    assert!(result.questions.is_empty());
    assert!(result.answers.is_empty());
    assert!(!result.ready_for_seed);
}

#[test]
fn test_interview_add_exchange() {
    let mut result = InterviewResult::new();
    result.add_exchange("What's the goal?", "Build an API");
    assert_eq!(result.questions.len(), 1);
    assert_eq!(result.answers.len(), 1);
    assert_eq!(result.questions[0], "What's the goal?");
    assert_eq!(result.answers[0], "Build an API");
}

#[test]
fn test_interview_update_ambiguity_becomes_ready() {
    let mut result = InterviewResult::new();
    assert!(!result.ready_for_seed);

    let good_score = AmbiguityScore::new(1.0, 1.0, 1.0);
    result.update_ambiguity(good_score);
    assert!(result.ready_for_seed);
}

#[test]
fn test_interview_update_ambiguity_stays_not_ready() {
    let mut result = InterviewResult::new();
    let bad_score = AmbiguityScore::new(0.0, 0.0, 0.0);
    result.update_ambiguity(bad_score);
    assert!(!result.ready_for_seed);
}
