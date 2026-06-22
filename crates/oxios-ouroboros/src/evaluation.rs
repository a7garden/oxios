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
                    // Extract language-agnostic key tokens via the shared
                    // helper (chars().count(), not byte length, so CJK
                    // text isn't silently dropped) and check if most of
                    // them appear in the output.
                    let tokens = key_tokens(criterion);
                    if tokens.is_empty() {
                        // Fallback to full substring match
                        let contains = output_lower.contains(&c_lower);
                        (contains, format!("substring_match={contains}"))
                    } else {
                        // Check how many key tokens appear in the output
                        let matched = tokens
                            .iter()
                            .filter(|t| output_lower.contains(t.as_str()))
                            .count();
                        let ratio = matched as f64 / tokens.len() as f64;
                        let passed = ratio >= 0.5; // At least half the key tokens match
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
/// non-ASCII characters. The non-ASCII branch is what makes this
/// language-agnostic: `str::len()` counts UTF-8 bytes, so a single CJK
/// glyph (3 bytes) was always rejected by the old `w.len() > 3` filter —
/// here it is kept because one CJK character is a meaningful token.
///
/// Shared by [`MechanicalEvalResult::evaluate`] and the degraded
/// evaluator ([`crate::degraded::degraded_evaluation`]) so both paths
/// agree on tokenization and stop-words.
pub fn key_tokens(criterion: &str) -> Vec<String> {
    criterion
        .to_lowercase()
        .split_whitespace()
        .filter(|w| {
            let non_ascii = w.bytes().any(|b| b >= 0x80);
            let meaningful = if non_ascii {
                // Non-ASCII (e.g. CJK) — single glyphs are meaningful.
                w.chars().count() >= 1
            } else {
                // ASCII — require > 3 chars to drop noise (a, the, is, of).
                w.chars().count() > 3
            };
            meaningful && !STOPWORDS.contains(w)
        })
        .map(str::to_string)
        .collect()
}
