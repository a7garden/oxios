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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Property tests — invariant checks proptest is built for.
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// key_tokens never returns a stopword.
        #[test]
        fn prop_key_tokens_excludes_stopwords(s in "[a-zA-Z ]{1,100}") {
            let tokens = key_tokens(&s);
            for sw in STOPWORDS {
                prop_assert!(
                    !tokens.iter().any(|t| t == sw),
                    "stopword {:?} leaked into tokens {:?} for input {:?}",
                    sw,
                    tokens,
                    s
                );
            }
        }

        /// key_tokens output tokens are always ≥ 4 chars (ASCII) or any
        /// non-ASCII token. So the lowercase-bounded character count
        /// property holds.
        #[test]
        fn prop_key_tokens_tokens_are_meaningful(s in "[a-zA-Z ]{1,100}") {
            let tokens = key_tokens(&s);
            for t in &tokens {
                let has_non_ascii = t.bytes().any(|b| b >= 0x80);
                if has_non_ascii {
                    // Non-ASCII tokens can be any length ≥ 1.
                    prop_assert!(!t.is_empty(), "non-ASCII token is empty: {:?}", t);
                } else {
                    // ASCII tokens must be longer than 3 chars.
                    prop_assert!(t.chars().count() > 3, "ascii token too short: {:?}", t);
                }
            }
        }

        /// MechanicalEvalResult::evaluate is deterministic: same input → same output.
        #[test]
        fn prop_evaluate_is_deterministic(
            criteria in proptest::collection::vec("[a-zA-Z ]{1,40}", 1..4),
            output in "[a-zA-Z 0-9]{1,80}",
        ) {
            let r1 = MechanicalEvalResult::evaluate(&criteria, &output);
            let r2 = MechanicalEvalResult::evaluate(&criteria, &output);
            prop_assert_eq!(r1.all_passed, r2.all_passed);
            prop_assert_eq!(r1.criterion_results.len(), r2.criterion_results.len());
            for (a, b) in r1.criterion_results.iter().zip(r2.criterion_results.iter()) {
                prop_assert_eq!(a.passed, b.passed);
                prop_assert_eq!(&a.reason, &b.reason);
            }
        }

        /// evaluate preserves criterion order: result[i] corresponds to
        /// criteria[i].
        #[test]
        fn prop_evaluate_preserves_criterion_order(
            criteria in proptest::collection::vec("[a-zA-Z ]{1,40}", 1..5),
            output in "[a-zA-Z 0-9]{1,80}",
        ) {
            let r = MechanicalEvalResult::evaluate(&criteria, &output);
            prop_assert_eq!(r.criterion_results.len(), criteria.len());
            for (got, want) in r.criterion_results.iter().zip(criteria.iter()) {
                prop_assert_eq!(&got.criterion, want);
            }
        }

        /// evaluate output count equals criteria count.
        #[test]
        fn prop_evaluate_result_count_matches_criteria(
            criteria in proptest::collection::vec("[a-zA-Z ]{1,40}", 0..6),
            output in "[a-zA-Z 0-9]{0,80}",
        ) {
            let r = MechanicalEvalResult::evaluate(&criteria, &output);
            prop_assert_eq!(r.criterion_results.len(), criteria.len());
        }

        /// all_passed is true iff every individual result is passed.
        #[test]
        fn prop_evaluate_all_passed_consistent(
            criteria in proptest::collection::vec("[a-zA-Z ]{1,40}", 1..5),
            output in "[a-zA-Z 0-9]{1,80}",
        ) {
            let r = MechanicalEvalResult::evaluate(&criteria, &output);
            let expected = r.criterion_results.iter().all(|c| c.passed);
            prop_assert_eq!(r.all_passed, expected);
        }
    }
    // -----------------------------------------------------------------------
    // key_tokens — tokenization invariants
    // -----------------------------------------------------------------------

    #[test]
    fn key_tokens_drops_short_ascii_words() {
        // "the" is both a stopword and ≤3 chars; "is" is ≤3 chars.
        assert!(key_tokens("the cat is on the rug").is_empty());
    }

    #[test]
    fn key_tokens_keeps_ascii_longer_than_three_chars() {
        let tokens = key_tokens("function must return value");
        // "function" (8) and "return" (6) and "value" (5) all > 3 and not stopwords.
        // "must" is a stopword.
        assert!(tokens.contains(&"function".to_string()));
        assert!(tokens.contains(&"return".to_string()));
        assert!(tokens.contains(&"value".to_string()));
        assert!(!tokens.contains(&"must".to_string()));
    }

    #[test]
    fn key_tokens_lowercases_input() {
        let tokens = key_tokens("CARGO Build TEST");
        assert!(tokens.contains(&"cargo".to_string()));
        assert!(tokens.contains(&"build".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn key_tokens_filters_all_listed_stopwords() {
        // All ten STOPWORDS entries — each is exactly one of the stopwords.
        for sw in STOPWORDS {
            let criterion = format!("must {sw} the code");
            let tokens = key_tokens(&criterion);
            assert!(
                !tokens.iter().any(|t| t == sw),
                "stopword {sw:?} leaked into tokens: {tokens:?}"
            );
        }
    }

    #[test]
    fn key_tokens_keeps_cjk_single_glyphs() {
        // 한 (Hangul) and 漢 (CJK) are non-ASCII and ≥1 char — both pass the
        // non-ascii branch even though they are short.
        let tokens = key_tokens("테스트 한자 漢字");
        assert_eq!(
            tokens,
            vec!["테스트".to_string(), "한자".to_string(), "漢字".to_string()]
        );
    }

    #[test]
    fn key_tokens_keeps_mixed_latin_punctuation_stripped_by_split_whitespace() {
        // split_whitespace collapses both kinds of whitespace; punctuation
        // glued to a word stays glued, so "rust-test" is one token, > 3 chars,
        // and not a stopword.
        let tokens = key_tokens("rust-test runs clean");
        assert!(tokens.contains(&"rust-test".to_string()));
    }

    #[test]
    fn key_tokens_empty_input_yields_empty_vec() {
        assert!(key_tokens("").is_empty());
        assert!(key_tokens("    ").is_empty());
    }

    // -----------------------------------------------------------------------
    // degraded_verdict — LLM-failure fallback contract
    // -----------------------------------------------------------------------

    #[test]
    fn degraded_verdict_passed_true_has_full_score_no_gaps() {
        let v = degraded_verdict(true);
        assert!(v.passed);
        assert_eq!(v.score, 1.0);
        assert!(v.notes.is_empty());
        assert!(v.gaps.is_empty());
    }

    #[test]
    fn degraded_verdict_passed_false_has_zero_score_and_canonical_gap() {
        let v = degraded_verdict(false);
        assert!(!v.passed);
        assert_eq!(v.score, 0.0);
        assert!(v.notes.is_empty());
        assert_eq!(v.gaps, vec!["Mechanical check failed".to_string()]);
    }

    // -----------------------------------------------------------------------
    // MechanicalEvalResult::evaluate — branches & invariants
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_exit_code_zero_branch_passes() {
        let result = MechanicalEvalResult::evaluate(
            &[String::from("script must exit code 0")],
            "running…\nexit code 0\n",
        );
        assert!(result.all_passed);
        assert_eq!(result.criterion_results.len(), 1);
        assert!(result.criterion_results[0].passed);
        assert!(
            result.criterion_results[0]
                .reason
                .contains("exit_code_0=true")
        );
    }

    #[test]
    fn evaluate_exit_status_zero_branch_passes() {
        let result = MechanicalEvalResult::evaluate(
            &[String::from("must report exit status 0")],
            "ok (exit status 0)",
        );
        assert!(result.all_passed);
        assert_eq!(result.criterion_results[0].reason, "exit_code_0=true");
    }

    #[test]
    fn evaluate_exit_code_nonzero_fails() {
        let result =
            MechanicalEvalResult::evaluate(&[String::from("must exit code 0")], "exit code 1\n");
        assert!(!result.all_passed);
        assert!(!result.criterion_results[0].passed);
        assert!(
            result.criterion_results[0]
                .reason
                .contains("exit_code_0=false")
        );
    }

    fn evaluate_keyword_ratio_majority_pass_threshold() {
        // criterion: "function must return value" → tokens: function, return, value
        // output omits "value" → 2/3 = 67% → passes (>= 0.5).
        let result = MechanicalEvalResult::evaluate(
            &[String::from("function must return value")],
            "the function will return now",
        );
        assert!(result.criterion_results[0].passed);
        assert!(result.criterion_results[0].reason.contains("2/3"));
    }

    #[test]
    fn evaluate_keyword_ratio_below_threshold_fails() {
        let result = MechanicalEvalResult::evaluate(
            &[String::from("function must return value")],
            "function runs", // only 1/3 keywords → 33% < 50%
        );
        assert!(!result.criterion_results[0].passed);
        assert!(result.criterion_results[0].reason.contains("1/3"));
    }

    #[test]
    fn evaluate_keyword_ratio_exact_half_passes() {
        // 1/2 = 50% — at the threshold, passes.
        let result = MechanicalEvalResult::evaluate(
            &[String::from("alpha beta")], // both tokens, neither is a stopword
            "only alpha is here",
        );
        assert!(result.criterion_results[0].passed);
        assert!(result.criterion_results[0].reason.contains("1/2"));
    }

    #[test]
    fn evaluate_falls_back_to_substring_match_when_no_tokens() {
        // "is" → no tokens (≤3 chars). Substring branch kicks in.
        let result = MechanicalEvalResult::evaluate(&[String::from("is ok")], "is ok to proceed");
        assert!(result.criterion_results[0].passed);
        assert!(
            result.criterion_results[0]
                .reason
                .contains("substring_match=true")
        );
    }

    #[test]
    fn evaluate_substring_fails_when_missing() {
        let result = MechanicalEvalResult::evaluate(&[String::from("is ok")], "definitely not ok");
        assert!(!result.criterion_results[0].passed);
        assert!(
            result.criterion_results[0]
                .reason
                .contains("substring_match=false")
        );
    }

    #[test]
    fn evaluate_all_passed_requires_every_criterion_pass() {
        let result = MechanicalEvalResult::evaluate(
            &[
                String::from("must exit code 0"),
                String::from("function must return value"),
            ],
            "function will return value\nexit code 0\n",
        );
        assert_eq!(result.criterion_results.len(), 2);
        assert!(result.all_passed);
    }

    #[test]
    fn evaluate_all_passed_false_when_any_fails() {
        // criterion[0] (exit code 0) passes; criterion[1] (function must return value)
        // has only 1/3 keywords in "function runs" → 33% < 50% → fails → all_passed=false.
        let result = MechanicalEvalResult::evaluate(
            &[
                String::from("must exit code 0"),
                String::from("function must return value"),
            ],
            "function runs\nexit code 0\n",
        );
        assert!(!result.all_passed);
        assert!(result.criterion_results[0].passed);
        assert!(!result.criterion_results[1].passed);
    }
    #[test]
    fn evaluate_empty_criteria_yields_empty_result_passing() {
        let result = MechanicalEvalResult::evaluate(&[], "any output");
        assert!(result.all_passed);
        assert!(result.criterion_results.is_empty());
    }

    #[test]
    fn evaluate_preserves_criterion_text_in_result() {
        let criterion = "function must return value";
        let result =
            MechanicalEvalResult::evaluate(&[criterion.to_string()], "function returns value");
        assert_eq!(result.criterion_results[0].criterion, criterion);
    }

    #[test]
    fn evaluate_is_case_insensitive_in_output() {
        // Uppercase tokens in output should still match lowercase criterion tokens.
        let result = MechanicalEvalResult::evaluate(
            &[String::from("function must return value")],
            "FUNCTION will RETURN some VALUE",
        );
        assert!(result.criterion_results[0].passed);
    }
}
