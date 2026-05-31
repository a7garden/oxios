//! Task runners — execute benchmark tasks through different interfaces.

pub mod process_runner;

use crate::task::Task;
use crate::{RunOutput, TaskResult, Tier};
use async_trait::async_trait;

/// A runner executes a benchmark task and returns its output.
#[async_trait]
pub trait Runner: Send + Sync {
    /// Run a single task and return the output.
    async fn run(&self, task: &Task) -> anyhow::Result<RunOutput>;

    /// Friendly name for this runner.
    fn name(&self) -> &str;

    /// Which tier this runner handles.
    fn tier(&self) -> Tier;
}

/// Evaluate a task's output against its assertions and produce a TaskResult.
pub fn evaluate_task(task: &Task, output: &RunOutput, duration_ms: u64) -> TaskResult {
    let mut assertion_results = Vec::new();

    // Evaluate task-level assertions
    for assertion in &task.assertions {
        assertion_results.push(assertion.evaluate(output));
    }

    // If multi-turn, also evaluate turn-level assertions against the final output
    for turn in &task.turns {
        for assertion in &turn.assertions {
            assertion_results.push(assertion.evaluate(output));
        }
    }

    // Calculate weighted score
    let total_weight: f64 = assertion_results.len() as f64; // uniform weight for now
    let earned: f64 = assertion_results
        .iter()
        .map(|r| if r.passed { 1.0 } else { 0.0 })
        .sum();
    let score = if assertion_results.is_empty() {
        100.0 // No assertions = auto-pass
    } else {
        (earned / total_weight) * 100.0
    };

    // A task passes if score >= 80 AND all structural assertions pass
    let all_structural_pass = assertion_results.iter().all(|r| r.passed);
    let passed = score >= 80.0 && all_structural_pass;

    TaskResult {
        task_id: task.id.clone(),
        task_name: task.name.clone(),
        suite: task.suite.clone(),
        tier: task.tier,
        passed,
        score,
        assertion_results,
        duration_ms,
        error: None,
    }
}
