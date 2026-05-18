//! Report generation for benchmark results
//!
//! Produces structured JSON reports and human-readable summaries.

use crate::{BenchmarkRun, BenchmarkStats, TaskResult};

/// Generate a JSON report from a benchmark run
pub fn generate_json_report(run: &BenchmarkRun) -> String {
    serde_json::to_string_pretty(run).unwrap_or_else(|e| {
        format!("{{\"error\": \"failed to generate report: {}\"}}", e)
    })
}

/// Generate aggregate statistics from a benchmark run
pub fn compute_stats(run: &BenchmarkRun) -> BenchmarkStats {
    let mut stats = BenchmarkStats::default();
    stats.total_tasks = run.task_results.len();
    stats.total_duration_ms = run
        .task_results
        .iter()
        .map(|r| r.duration_ms)
        .sum::<u64>();

    for result in &run.task_results {
        if result.passed {
            stats.passed += 1;
        } else {
            stats.failed += 1;
        }
        stats.average_score += result.score;
    }

    if stats.total_tasks > 0 {
        stats.average_score /= stats.total_tasks as f64;
    }

    // Extract structure info from trace if available
    if let Some(ref trace) = run.trace {
        stats.agents_created = trace
            .events
            .iter()
            .filter(|e| e.event_type == "AgentCreated")
            .count();
        stats.seeds_created = trace
            .events
            .iter()
            .filter(|e| e.event_type == "SeedCreated")
            .count();
        stats.spaces_created = trace
            .events
            .iter()
            .filter(|e| e.event_type == "SpaceCreated")
            .count();
        stats.memories_stored = trace
            .events
            .iter()
            .filter(|e| e.event_type == "MemoryStored")
            .count();
        stats.phases_completed = trace
            .events
            .iter()
            .filter(|e| e.event_type == "PhaseCompleted")
            .count();
    }

    stats
}

/// Print a human-readable benchmark summary
pub fn print_summary(run: &BenchmarkRun) {
    let stats = compute_stats(run);

    println!("\n{}", "=".repeat(60));
    println!("OXIOS BENCHMARK REPORT");
    println!("{}", "=".repeat(60));
    println!("Benchmark ID: {}", run.id.0);
    println!("Started: {}", run.started_at.to_rfc3339());
    if let Some(completed) = run.completed_at {
        println!("Completed: {}", completed.to_rfc3339());
    }
    println!();

    println!("--- Task Results ({}) ---", stats.total_tasks);
    for (i, result) in run.task_results.iter().enumerate() {
        let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
        println!(
            "  {}. {} [{}] - Score: {:.0}% ({})",
            i + 1,
            result.task_id,
            status,
            result.score,
            result.evaluation_notes
        );
    }
    println!();

    println!("--- Aggregate ---");
    println!("  Total: {}", stats.total_tasks);
    println!("  Passed: {} ({:.0}%)", stats.passed, (stats.passed as f64 / stats.total_tasks as f64) * 100.0);
    println!("  Failed: {} ({:.0}%)", stats.failed, (stats.failed as f64 / stats.total_tasks as f64) * 100.0);
    println!("  Average Score: {:.1}%", stats.average_score);
    println!("  Total Duration: {}ms", stats.total_duration_ms);
    println!();

    println!("--- System Structure Observed ---");
    println!("  Agents Created: {}", stats.agents_created);
    println!("  Seeds Created: {}", stats.seeds_created);
    println!("  Spaces Created: {}", stats.spaces_created);
    println!("  Memories Stored: {}", stats.memories_stored);
    println!("  Phases Completed: {}", stats.phases_completed);
    println!("{}", "=".repeat(60));
}

/// Print a single task result
pub fn print_task_result(result: &TaskResult) {
    let status = if result.passed { "PASS" } else { "FAIL" };
    println!("[{}] {} - {:.0}%", status, result.task_id, result.score);
    println!("  Response: {}", result.response.chars().take(100).collect::<String>());
    println!("  Notes: {}", result.evaluation_notes);
}

/// Save report to file
pub fn save_report(run: &BenchmarkRun, path: &str) -> anyhow::Result<()> {
    let json = generate_json_report(run);
    std::fs::write(path, json)?;
    println!("Report saved to: {}", path);
    Ok(())
}

/// Load a previous benchmark run from file
pub fn load_report(path: &str) -> anyhow::Result<BenchmarkRun> {
    let json = std::fs::read_to_string(path)?;
    let run: BenchmarkRun = serde_json::from_str(&json)?;
    Ok(run)
}

/// Format duration in human-readable form
pub fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{:.1}m", ms as f64 / 60000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_stats() {
        let run = BenchmarkRun {
            id: BenchmarkId::new(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            task_results: vec![
                TaskResult {
                    task_id: "test1".to_string(),
                    passed: true,
                    score: 100.0,
                    response: "response".to_string(),
                    expected: vec![],
                    evaluation_notes: "ok".to_string(),
                    duration_ms: 100,
                },
                TaskResult {
                    task_id: "test2".to_string(),
                    passed: false,
                    score: 50.0,
                    response: "response".to_string(),
                    expected: vec![],
                    evaluation_notes: "ok".to_string(),
                    duration_ms: 200,
                },
            ],
            trace: None,
            config: BenchmarkConfig::default(),
        };

        let stats = compute_stats(&run);
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.passed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.average_score, 75.0);
    }
}