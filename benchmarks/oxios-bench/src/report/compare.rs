//! Regression comparison between benchmark runs.

use crate::{BenchmarkRun, Regression, TaskResult};

/// Compare two benchmark runs and detect regressions.
pub fn compare_runs(
    baseline: &BenchmarkRun,
    current: &BenchmarkRun,
    threshold: f64,
) -> CompareResult {
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    let mut unchanged = Vec::new();
    let mut new_tasks = Vec::new();
    let mut removed_tasks = Vec::new();

    // Index baseline by task ID
    let baseline_map: std::collections::HashMap<&str, &TaskResult> = baseline
        .results
        .iter()
        .map(|r| (r.task_id.as_str(), r))
        .collect();

    let current_map: std::collections::HashMap<&str, &TaskResult> = current
        .results
        .iter()
        .map(|r| (r.task_id.as_str(), r))
        .collect();

    // Find regressions and improvements
    for result in &current.results {
        match baseline_map.get(result.task_id.as_str()) {
            Some(baseline_result) => {
                let delta = result.score - baseline_result.score;
                if delta < -threshold {
                    regressions.push(Regression {
                        task_id: result.task_id.clone(),
                        previous_score: baseline_result.score,
                        current_score: result.score,
                        delta,
                    });
                } else if delta > threshold {
                    improvements.push(Improvement {
                        task_id: result.task_id.clone(),
                        previous_score: baseline_result.score,
                        current_score: result.score,
                        delta,
                    });
                } else {
                    unchanged.push(result.task_id.clone());
                }
            }
            None => {
                new_tasks.push(result.task_id.clone());
            }
        }
    }

    // Find removed tasks
    for result in &baseline.results {
        if !current_map.contains_key(result.task_id.as_str()) {
            removed_tasks.push(result.task_id.clone());
        }
    }

    let has_regressions = !regressions.is_empty();

    CompareResult {
        baseline_id: baseline.id.clone(),
        current_id: current.id.clone(),
        regressions,
        improvements,
        unchanged,
        new_tasks,
        removed_tasks,
        has_regressions,
    }
}

/// Result of comparing two benchmark runs.
#[derive(Debug)]
pub struct CompareResult {
    pub baseline_id: String,
    pub current_id: String,
    pub regressions: Vec<Regression>,
    pub improvements: Vec<Improvement>,
    pub unchanged: Vec<String>,
    pub new_tasks: Vec<String>,
    pub removed_tasks: Vec<String>,
    pub has_regressions: bool,
}

/// An improvement detected between two runs.
#[derive(Debug)]
pub struct Improvement {
    pub task_id: String,
    pub previous_score: f64,
    pub current_score: f64,
    pub delta: f64,
}

/// Print a comparison result to stdout.
pub fn print_compare(result: &CompareResult) {
    use console::style;

    println!();
    println!(
        "  {} {} → {}",
        style("COMPARISON:").bold().cyan(),
        style(&result.baseline_id[..8]).dim(),
        style(&result.current_id[..8]).dim(),
    );
    println!();

    if !result.improvements.is_empty() {
        println!("  {} Improvements:", style("▲").green().bold());
        for imp in &result.improvements {
            println!(
                "    {} {} — {:.0} → {:.0} (Δ +{:.0})",
                style("✅").to_string(),
                style(&imp.task_id).bold(),
                imp.previous_score,
                imp.current_score,
                imp.delta,
            );
        }
        println!();
    }

    if !result.regressions.is_empty() {
        println!("  {} Regressions:", style("▼").red().bold());
        for reg in &result.regressions {
            let severity = if reg.delta <= -100.0 {
                style("CRITICAL").red().bold()
            } else if reg.delta <= -50.0 {
                style("MAJOR").red()
            } else {
                style("MINOR").yellow()
            };
            println!(
                "    {} {} — {:.0} → {:.0} (Δ {:.0}) — {}",
                style("❌").to_string(),
                style(&reg.task_id).bold(),
                reg.previous_score,
                reg.current_score,
                reg.delta,
                severity,
            );
        }
        println!();
    }

    if !result.new_tasks.is_empty() {
        println!(
            "  New tasks: {}",
            result
                .new_tasks
                .iter()
                .map(|t| style(t).green().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if !result.removed_tasks.is_empty() {
        println!(
            "  Removed tasks: {}",
            result
                .removed_tasks
                .iter()
                .map(|t| style(t).dim().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    println!(
        "  Unchanged: {} tasks",
        style(result.unchanged.len()).dim()
    );

    if result.has_regressions {
        println!();
        println!(
            "  {}",
            style("EXIT CODE 2: Regressions detected").red().bold()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RunSummary, Tier};

    fn make_run(id: &str, results: Vec<TaskResult>) -> BenchmarkRun {
        BenchmarkRun {
            id: id.to_string(),
            timestamp: chrono::Utc::now(),
            oxios_version: None,
            git_ref: None,
            summary: RunSummary {
                total: results.len(),
                passed: results.iter().filter(|r| r.passed).count(),
                failed: results.iter().filter(|r| !r.passed).count(),
                skipped: 0,
                score_avg: 0.0,
                duration_total_ms: 0,
                regressions: vec![],
            },
            results,
        }
    }

    fn make_result(id: &str, score: f64) -> TaskResult {
        TaskResult {
            task_id: id.to_string(),
            task_name: id.to_string(),
            suite: "test".to_string(),
            tier: Tier::E2e,
            passed: score >= 80.0,
            score,
            assertion_results: vec![],
            duration_ms: 100,
            error: None,
        }
    }

    #[test]
    fn test_detect_regression() {
        let baseline = make_run(
            "baseline",
            vec![make_result("task_a", 100.0), make_result("task_b", 80.0)],
        );
        let current = make_run(
            "current",
            vec![make_result("task_a", 50.0), make_result("task_b", 80.0)],
        );

        let result = compare_runs(&baseline, &current, 5.0);
        assert!(result.has_regressions);
        assert_eq!(result.regressions.len(), 1);
        assert_eq!(result.regressions[0].task_id, "task_a");
        assert_eq!(result.unchanged.len(), 1);
    }

    #[test]
    fn test_detect_improvement() {
        let baseline = make_run("baseline", vec![make_result("task_a", 50.0)]);
        let current = make_run("current", vec![make_result("task_a", 100.0)]);

        let result = compare_runs(&baseline, &current, 5.0);
        assert!(!result.has_regressions);
        assert_eq!(result.improvements.len(), 1);
    }

    #[test]
    fn test_new_and_removed_tasks() {
        let baseline = make_run("baseline", vec![make_result("task_a", 100.0)]);
        let current = make_run("current", vec![make_result("task_b", 100.0)]);

        let result = compare_runs(&baseline, &current, 5.0);
        assert_eq!(result.new_tasks, vec!["task_b"]);
        assert_eq!(result.removed_tasks, vec!["task_a"]);
    }
}
