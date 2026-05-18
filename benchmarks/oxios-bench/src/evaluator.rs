//! Task evaluator for benchmark results
//!
//! Evaluates task completion based on expected outcomes and response content.

use crate::{TaskResult};

/// Default evaluation function that checks for keyword presence
pub fn keyword_evaluation(response: &str, expected: &[&str]) -> TaskResult {
    let response_lower = response.to_lowercase();
    let matches: usize = expected.iter().filter(|kw| response_lower.contains(&kw.to_lowercase())).count();
    let score = if expected.is_empty() {
        100.0
    } else {
        (matches as f64 / expected.len() as f64) * 100.0
    };

    let passed = score >= 80.0;

    TaskResult {
        task_id: "".to_string(),
        passed,
        score,
        response: response.to_string(),
        expected: expected.iter().map(|s| s.to_string()).collect(),
        evaluation_notes: format!("{}/{} keywords matched ({:.0}%)", matches, expected.len(), score),
        duration_ms: 0,
    }
}

/// Evaluation function for math tasks
pub fn math_evaluation(response: &str, expected_answer: &str) -> TaskResult {
    let response_lower = response.to_lowercase();

    // Check if the expected answer appears in the response
    let has_answer = response_lower.contains(&expected_answer.to_lowercase());

    // Check for calculation indicators
    let has_calculation = response.contains('=') || response.contains("result");

    let score = if has_answer && has_calculation {
        100.0
    } else if has_answer {
        80.0
    } else {
        0.0
    };

    TaskResult {
        task_id: "".to_string(),
        passed: score >= 80.0,
        score,
        response: response.to_string(),
        expected: vec![expected_answer.to_string()],
        evaluation_notes: if has_answer {
            format!("Correct answer '{}' found", expected_answer)
        } else {
            format!("Expected answer '{}' not found", expected_answer)
        },
        duration_ms: 0,
    }
}

/// Evaluation function for web search tasks
pub fn web_search_evaluation(response: &str, required_info: &[&str]) -> TaskResult {
    let response_lower = response.to_lowercase();
    let matches: usize = required_info.iter().filter(|kw| response_lower.contains(&kw.to_lowercase())).count();
    let score = if required_info.is_empty() {
        100.0
    } else {
        (matches as f64 / required_info.len() as f64) * 100.0
    };

    let passed = score >= 60.0; // Web search is more flexible

    TaskResult {
        task_id: "".to_string(),
        passed,
        score,
        response: response.to_string(),
        expected: required_info.iter().map(|s| s.to_string()).collect(),
        evaluation_notes: format!("{}/{} info found ({:.0}%)", matches, required_info.len(), score),
        duration_ms: 0,
    }
}

/// Evaluation function for knowledge questions
pub fn knowledge_evaluation(response: &str, expected_fact: &str) -> TaskResult {
    let response_lower = response.to_lowercase();

    // Check if expected fact appears
    let has_fact = response_lower.contains(&expected_fact.to_lowercase());

    // Check for confidence indicators
    let has_confidence = response_lower.contains("capital")
        || response_lower.contains("is")
        || response_lower.contains("population")
        || response_lower.contains("answer");

    let score = if has_fact && has_confidence {
        100.0
    } else if has_fact {
        80.0
    } else {
        0.0
    };

    TaskResult {
        task_id: "".to_string(),
        passed: score >= 80.0,
        score,
        response: response.to_string(),
        expected: vec![expected_fact.to_string()],
        evaluation_notes: if has_fact {
            format!("Expected fact '{}' found", expected_fact)
        } else {
            format!("Expected fact '{}' not found", expected_fact)
        },
        duration_ms: 0,
    }
}

/// Evaluation function for session memory tasks
pub fn memory_evaluation(response: &str, _expected_content: &str) -> TaskResult {
    let response_lower = response.to_lowercase();

    // Check for memory-related confirmation
    let has_confirmation = response_lower.contains("remember")
        || response_lower.contains("saved")
        || response_lower.contains("stored")
        || response_lower.contains("confirmed")
        || response_lower.contains("understood")
        || response_lower.contains("okay")
        || response_lower.contains("got it");

    let score = if has_confirmation { 100.0 } else { 50.0 };

    TaskResult {
        task_id: "".to_string(),
        passed: has_confirmation,
        score,
        response: response.to_string(),
        expected: vec!["memory confirmation".to_string()],
        evaluation_notes: if has_confirmation {
            "Memory confirmation detected".to_string()
        } else {
            "No memory confirmation in response".to_string()
        },
        duration_ms: 0,
    }
}

/// Evaluate a response using custom criteria
pub fn evaluate_with_fn<F>(response: &str, f: F) -> TaskResult
where
    F: Fn(&str) -> (bool, f64, String),
{
    let (passed, score, notes) = f(response);
    TaskResult {
        task_id: "".to_string(),
        passed,
        score,
        response: response.to_string(),
        expected: vec![],
        evaluation_notes: notes,
        duration_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_evaluation() {
        let result = keyword_evaluation("The capital of Japan is Tokyo", &["Tokyo", "capital"]);
        assert!(result.passed);
        assert!(result.score >= 80.0);
    }

    #[test]
    fn test_math_evaluation() {
        let result = math_evaluation("17 * 23 = 391", "391");
        assert!(result.passed);
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_web_search_evaluation() {
        let result = web_search_evaluation("The time in Tokyo is 15:30", &["Tokyo", "time"]);
        assert!(result.passed);
    }
}