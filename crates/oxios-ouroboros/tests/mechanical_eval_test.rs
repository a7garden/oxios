//! Tests for MechanicalEvalResult — the language-agnostic mechanical
//! evaluation stage that checks acceptance criteria against execution
//! output before invoking the LLM semantic evaluator.

use oxios_ouroboros::evaluation::MechanicalEvalResult;

#[test]
fn mechanical_eval_all_criteria_pass() {
    let criteria = vec!["hello world".to_string(), "success".to_string()];
    let output = "The program printed hello world and reported success.";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed);
    assert_eq!(result.criterion_results.len(), 2);
    assert!(result.criterion_results[0].passed);
    assert!(result.criterion_results[1].passed);
}

#[test]
fn mechanical_eval_some_criteria_fail() {
    let criteria = vec!["hello world".to_string(), "missing keyword".to_string()];
    let output = "The program printed hello world.";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(!result.all_passed);
    assert!(result.criterion_results[0].passed);
    assert!(!result.criterion_results[1].passed);
}

#[test]
fn mechanical_eval_exit_code_detection() {
    let criteria = vec!["exit code 0".to_string()];
    let output = "Process finished with exit code 0";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed);
    assert!(result.criterion_results[0].passed);
}

#[test]
fn mechanical_eval_exit_status_detection() {
    let criteria = vec!["Exit Status is 0".to_string()];
    let output = "Result: exit status 0";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed);
}

#[test]
fn mechanical_eval_exit_code_failure() {
    let criteria = vec!["exit code 0".to_string()];
    let output = "Process failed with exit code 1";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(!result.all_passed);
    assert!(!result.criterion_results[0].passed);
}

#[test]
fn mechanical_eval_empty_criteria() {
    let criteria: Vec<String> = vec![];
    let output = "anything";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed); // vacuously true
    assert!(result.criterion_results.is_empty());
}
