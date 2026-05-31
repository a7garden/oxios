//! JSON report serialization.

use crate::{BenchmarkRun, Regression, RunSummary, TaskResult};
use std::collections::BTreeMap;

/// Generate a pretty-printed JSON report.
pub fn generate_json(run: &BenchmarkRun) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(run)?)
}

/// Save a benchmark run to a JSON file.
pub fn save_report(run: &BenchmarkRun, path: &str) -> anyhow::Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = generate_json(run)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a benchmark run from a JSON file.
pub fn load_report(path: &str) -> anyhow::Result<BenchmarkRun> {
    let json = std::fs::read_to_string(path)?;
    let run: BenchmarkRun = serde_json::from_str(&json)?;
    Ok(run)
}

/// Build a RunSummary from task results.
pub fn build_summary(results: &[TaskResult], regressions: Vec<Regression>) -> RunSummary {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();
    let score_avg = if total > 0 {
        results.iter().map(|r| r.score).sum::<f64>() / total as f64
    } else {
        0.0
    };
    let duration_total_ms: u64 = results.iter().map(|r| r.duration_ms).sum();

    RunSummary {
        total,
        passed,
        failed,
        skipped: 0,
        score_avg,
        duration_total_ms,
        regressions,
    }
}

/// Build per-suite statistics.
pub fn suite_stats(results: &[TaskResult]) -> BTreeMap<String, SuiteStats> {
    let mut map: BTreeMap<String, SuiteStats> = BTreeMap::new();

    for result in results {
        let stats = map.entry(result.suite.clone()).or_default();
        stats.total += 1;
        if result.passed {
            stats.passed += 1;
        } else {
            stats.failed += 1;
        }
        stats.score_sum += result.score;
        stats.duration_ms += result.duration_ms;
    }

    map
}

/// Statistics for a single suite.
#[derive(Debug, Clone, Default)]
pub struct SuiteStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub score_sum: f64,
    pub duration_ms: u64,
}

impl SuiteStats {
    pub fn avg_score(&self) -> f64 {
        if self.total > 0 {
            self.score_sum / self.total as f64
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tier;

    fn make_result(
        id: &str,
        suite: &str,
        passed: bool,
        score: f64,
        duration_ms: u64,
    ) -> TaskResult {
        TaskResult {
            task_id: id.to_string(),
            task_name: id.to_string(),
            suite: suite.to_string(),
            tier: Tier::E2e,
            passed,
            score,
            assertion_results: vec![],
            duration_ms,
            error: None,
        }
    }

    #[test]
    fn test_build_summary() {
        let results = vec![
            make_result("a", "ouroboros", true, 100.0, 100),
            make_result("b", "ouroboros", false, 50.0, 200),
        ];
        let summary = build_summary(&results, vec![]);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.score_avg, 75.0);
        assert_eq!(summary.duration_total_ms, 300);
    }

    #[test]
    fn test_suite_stats() {
        let results = vec![
            make_result("a", "ouroboros", true, 100.0, 100),
            make_result("b", "ouroboros", true, 80.0, 200),
            make_result("c", "agent", true, 100.0, 50),
        ];
        let stats = suite_stats(&results);
        assert_eq!(stats.get("ouroboros").unwrap().total, 2);
        assert_eq!(stats.get("ouroboros").unwrap().avg_score(), 90.0);
        assert_eq!(stats.get("agent").unwrap().total, 1);
    }
}
