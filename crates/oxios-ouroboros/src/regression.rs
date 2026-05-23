//! Regression detection across Ouroboros evolution generations.
//!
//! Tracks which acceptance criteria previously passed but started failing,
//! and injects regression context into the evolve prompt so the LLM
//! is aware of what it previously broke.

use crate::Seed;

/// A regression: an acceptance criterion that previously passed but now fails.
#[derive(Debug, Clone)]
pub struct Regression {
    /// Index of the acceptance criterion.
    pub ac_index: usize,
    /// The criterion text.
    pub ac_text: String,
    /// Generation where it last passed.
    pub passed_in_generation: u32,
    /// Generation where it started failing.
    pub failed_since_generation: u32,
    /// Number of consecutive failures.
    pub consecutive_failures: u32,
}

/// A record of one generation's evaluation.
#[derive(Debug, Clone)]
pub struct GenerationRecord {
    /// The seed for this generation.
    pub seed: Seed,
    /// Per-AC pass/fail results (index → passed).
    pub ac_results: Vec<bool>,
    /// Overall evaluation score.
    pub score: f64,
}

/// Regression detector that examines generation history.
#[derive(Debug, Clone, Default)]
pub struct RegressionDetector {
    /// History of generation records (ordered by generation).
    generations: Vec<GenerationRecord>,
}

impl RegressionDetector {
    /// Create a new empty detector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a generation's results.
    pub fn record(&mut self, record: GenerationRecord) {
        self.generations.push(record);
        // Keep only last 10 generations
        if self.generations.len() > 10 {
            self.generations.remove(0);
        }
    }

    /// Detect regressions across recorded generations.
    pub fn detect(&self) -> Vec<Regression> {
        if self.generations.len() < 2 {
            return vec![];
        }

        let latest = self.generations.last().unwrap();
        let mut regressions = Vec::new();

        for (ac_idx, &currently_passing) in latest.ac_results.iter().enumerate() {
            if currently_passing {
                continue; // Currently passing — no regression
            }

            // Find when it last passed
            let mut last_passed_gen: Option<u32> = None;
            let mut consecutive_failures = 0u32;

            for record in self.generations.iter().rev() {
                if ac_idx < record.ac_results.len() && record.ac_results[ac_idx] {
                    last_passed_gen = Some(record.seed.generation);
                    break;
                }
                consecutive_failures += 1;
            }

            if let Some(passed_gen) = last_passed_gen {
                let ac_text = if ac_idx < latest.seed.acceptance_criteria.len() {
                    latest.seed.acceptance_criteria[ac_idx].clone()
                } else {
                    format!("Criterion {}", ac_idx + 1)
                };

                regressions.push(Regression {
                    ac_index: ac_idx,
                    ac_text,
                    passed_in_generation: passed_gen,
                    failed_since_generation: passed_gen + 1,
                    consecutive_failures: consecutive_failures.saturating_sub(1),
                });
            }
        }

        regressions
    }

    /// Format regressions for injection into the evolve prompt.
    pub fn format_for_prompt(regressions: &[Regression]) -> String {
        if regressions.is_empty() {
            return String::new();
        }

        let mut lines = vec![format!("## REGRESSIONS ({})", regressions.len())];
        for reg in regressions {
            lines.push(format!(
                "  - AC {}: passed in Gen {}, failing since Gen {} ({} consecutive failures): {}",
                reg.ac_index + 1,
                reg.passed_in_generation,
                reg.failed_since_generation,
                reg.consecutive_failures,
                reg.ac_text
            ));
        }
        lines.push(
            "  CRITICAL: These ACs previously passed. Preserve their behavior while fixing others."
                .to_string(),
        );

        lines.join("\n")
    }

    /// Number of recorded generations.
    pub fn len(&self) -> usize {
        self.generations.len()
    }

    /// Whether any generations have been recorded.
    pub fn is_empty(&self) -> bool {
        self.generations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entity;
    use chrono::Utc;

    fn make_seed(gen: u32, ac_count: usize) -> Seed {
        Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Test goal".to_string(),
            constraints: vec![],
            acceptance_criteria: (0..ac_count)
                .map(|i| format!("Criterion {}", i + 1))
                .collect(),
            ontology: vec![],
            created_at: Utc::now(),
            generation: gen,
            parent_seed_id: None,
            cspace_hint: None,
            original_request: String::new(),
        }
    }

    #[test]
    fn test_no_regressions_single_generation() {
        let mut detector = RegressionDetector::new();
        detector.record(GenerationRecord {
            seed: make_seed(0, 3),
            ac_results: vec![true, true, true],
            score: 1.0,
        });
        assert!(detector.detect().is_empty());
    }

    #[test]
    fn test_detects_regression() {
        let mut detector = RegressionDetector::new();
        // Gen 0: all pass
        detector.record(GenerationRecord {
            seed: make_seed(0, 3),
            ac_results: vec![true, true, true],
            score: 1.0,
        });
        // Gen 1: AC 2 starts failing
        detector.record(GenerationRecord {
            seed: make_seed(1, 3),
            ac_results: vec![true, false, true],
            score: 0.7,
        });

        let regressions = detector.detect();
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].ac_index, 1);
        assert_eq!(regressions[0].passed_in_generation, 0);
    }

    #[test]
    fn test_no_regression_if_never_passed() {
        let mut detector = RegressionDetector::new();
        detector.record(GenerationRecord {
            seed: make_seed(0, 2),
            ac_results: vec![true, false],
            score: 0.5,
        });
        detector.record(GenerationRecord {
            seed: make_seed(1, 2),
            ac_results: vec![true, false],
            score: 0.5,
        });

        // AC 2 never passed, so it's not a regression
        assert!(detector.detect().is_empty());
    }

    #[test]
    fn test_format_for_prompt() {
        let regressions = vec![Regression {
            ac_index: 1,
            ac_text: "Tests pass".to_string(),
            passed_in_generation: 0,
            failed_since_generation: 1,
            consecutive_failures: 2,
        }];
        let text = RegressionDetector::format_for_prompt(&regressions);
        assert!(text.contains("REGRESSIONS"));
        assert!(text.contains("Tests pass"));
        assert!(text.contains("CRITICAL"));
    }
}
