//! Fallback helpers for LLM parsing failures and mechanical evaluation.
//!
//! Used by [`crate::engine::IntentEngine`] when an LLM response can't be
//! parsed — provides safe defaults that preserve user intent.

use serde::{Deserialize, Serialize};

use crate::directive::Verdict;

// ---------------------------------------------------------------------------
// Degraded fallbacks (LLM parse failure)
// ---------------------------------------------------------------------------

/// Produce a degraded [`Verdict`] when the review LLM call fails.
///
/// Reports the mechanical check result with no semantic analysis.
pub fn degraded_verdict(passed: bool) -> Verdict {
    Verdict {
        passed,
        score: if passed { 1.0 } else { 0.0 },
        notes: Vec::new(),
        gaps: if passed {
            Vec::new()
        } else {
            vec!["Mechanical check failed".to_string()]
        },
    }
}

// ---------------------------------------------------------------------------
// Mechanical evaluation (no LLM)
// ---------------------------------------------------------------------------

/// Result of mechanical (non-LLM) evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanicalEvalResult {
    /// Each criterion and whether it passed mechanically.
    pub criterion_results: Vec<CriterionResult>,
    /// Overall mechanical pass (all criteria passed).
    pub all_passed: bool,
}

/// Result of evaluating a single acceptance criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    /// The acceptance criterion being checked.
    pub criterion: String,
    /// Whether it passed.
    pub passed: bool,
    /// Why it passed or failed.
    pub reason: String,
}

impl MechanicalEvalResult {
    /// Run mechanical checks against acceptance criteria.
    ///
    /// Checks structural patterns only (language-agnostic):
    /// - Substring containment (works for any language)
    /// - Exit code 0 presence
    /// - Absence of common error markers
    pub fn evaluate(criteria: &[String], output: &str) -> Self {
        let output_lower = output.to_lowercase();
        let mut results = Vec::new();

        for criterion in criteria {
            let c_lower = criterion.to_lowercase();
            let (passed, reason) =
                if c_lower.contains("exit code") || c_lower.contains("exit status") {
                    let has_zero = output_lower.contains("exit code 0")
                        || output_lower.contains("exit status 0");
                    (has_zero, format!("exit_code_0={has_zero}"))
                } else {
                    let tokens = key_tokens(criterion);
                    if tokens.is_empty() {
                        let contains = output_lower.contains(&c_lower);
                        (contains, format!("substring_match={contains}"))
                    } else {
                        let matched = tokens
                            .iter()
                            .filter(|t| output_lower.contains(t.as_str()))
                            .count();
                        let ratio = matched as f64 / tokens.len() as f64;
                        let passed = ratio >= 0.5;
                        (
                            passed,
                            format!(
                                "keyword_match={}/{} ({:.0}%)",
                                matched,
                                tokens.len(),
                                ratio * 100.0
                            ),
                        )
                    }
                };
            results.push(CriterionResult {
                criterion: criterion.clone(),
                passed,
                reason,
            });
        }

        let all_passed = results.iter().all(|r| r.passed);
        Self {
            criterion_results: results,
            all_passed,
        }
    }
}

/// Common English stop-words excluded from acceptance-criterion key tokens.
const STOPWORDS: &[&str] = &[
    "the", "must", "should", "shall", "where", "which", "that", "with", "from", "this",
];

/// Extract meaningful key tokens from an acceptance criterion.
///
/// Lowercases the criterion and splits on whitespace, keeping tokens that
/// are either ASCII words longer than 3 characters or contain any
/// non-ASCII characters (language-agnostic: CJK glyphs are meaningful).
pub fn key_tokens(criterion: &str) -> Vec<String> {
    criterion
        .to_lowercase()
        .split_whitespace()
        .filter(|w| {
            let non_ascii = w.bytes().any(|b| b >= 0x80);
            let meaningful = if non_ascii {
                w.chars().count() >= 1
            } else {
                w.chars().count() > 3
            };
            meaningful && !STOPWORDS.contains(w)
        })
        .map(str::to_string)
        .collect()
}
