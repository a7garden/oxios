//! Console report — human-readable terminal output with colors.

use crate::report::json_report::{suite_stats, SuiteStats};
use crate::{BenchmarkRun, Regression, TaskResult};
use console::style;
use std::collections::BTreeMap;

/// Print a full benchmark report to stdout.
pub fn print_report(run: &BenchmarkRun, verbose: bool) {
    let summary = &run.summary;

    // Header
    println!();
    println!(
        "{}",
        style("━".repeat(65)).dim()
    );
    println!(
        "  {} — {} tasks · {} · {}",
        style("OXIOS BENCHMARK").bold().cyan(),
        style(summary.total).bold(),
        format_duration(summary.duration_total_ms),
        run.oxios_version
            .as_deref()
            .unwrap_or("unknown")
    );
    if let Some(ref git_ref) = run.git_ref {
        println!("  git: {}", style(git_ref).dim());
    }
    println!(
        "{}",
        style("━".repeat(65)).dim()
    );
    println!();

    // Per-suite table
    let stats = suite_stats(&run.results);
    if !stats.is_empty() {
        print_suite_table(&stats);
    }

    // Per-task details (if verbose or if there are failures)
    if verbose || summary.failed > 0 {
        print_task_details(&run.results, verbose);
    }

    // Summary
    print_summary_line(summary);

    // Regressions
    if !summary.regressions.is_empty() {
        print_regressions(&summary.regressions);
    }

    println!(
        "{}",
        style("━".repeat(65)).dim()
    );
    println!();
}

/// Print the per-suite summary table.
fn print_suite_table(stats: &BTreeMap<String, SuiteStats>) {
    println!(
        "  {}       {}  {}  {}  {}  {}",
        style("SUITE").bold().dim(),
        style("TOTAL").bold().dim(),
        style("PASS").bold().dim(),
        style("FAIL").bold().dim(),
        style("SCORE").bold().dim(),
        style("TIME").bold().dim(),
    );
    println!(
        "  {}",
        style("─".repeat(60)).dim()
    );

    for (name, s) in stats {
        let score_str = format!("{:.0}%", s.avg_score());
        let score_styled = if s.avg_score() >= 80.0 {
            style(&score_str).green()
        } else if s.avg_score() >= 60.0 {
            style(&score_str).yellow()
        } else {
            style(&score_str).red()
        };

        println!(
            "  {:14} {:5} {:5} {:5} {:8} {}",
            style(name).cyan(),
            s.total,
            style(s.passed).green(),
            style(s.failed).red(),
            score_styled,
            format_duration(s.duration_ms),
        );
    }
    println!();
}

/// Print per-task pass/fail details.
fn print_task_details(results: &[TaskResult], verbose: bool) {
    for result in results {
        if result.passed && !verbose {
            continue; // Skip passing tasks in non-verbose mode
        }

        let icon = if result.passed { "✅" } else { "❌" };
        println!(
            "  {} {} — {:.0}% ({})",
            icon,
            style(&result.task_id).bold(),
            result.score,
            style(&result.suite).dim(),
        );

        // Show assertion details for failures
        if !result.passed || verbose {
            for ar in &result.assertion_results {
                let icon = if ar.passed { "  ✓" } else { "  ✗" };
                let styled = if ar.passed {
                    style(icon).green()
                } else {
                    style(icon).red()
                };
                println!(
                    "{} {} — expected: {}, actual: {}",
                    styled,
                    style(&ar.assertion).dim(),
                    style(&ar.expected).dim(),
                    if ar.passed {
                        style(&ar.actual).green()
                    } else {
                        style(&ar.actual).red()
                    },
                );
            }
        }

        if let Some(ref err) = result.error {
            println!("    error: {}", style(err).red());
        }
    }
    println!();
}

/// Print the aggregate summary line.
fn print_summary_line(summary: &crate::RunSummary) {
    let pass_pct = if summary.total > 0 {
        (summary.passed as f64 / summary.total as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "  {} {} / {} passed ({:.0}%) · score avg: {:.1}% · {}",
        style("TOTAL:").bold(),
        style(summary.passed).green(),
        style(summary.total).bold(),
        pass_pct,
        summary.score_avg,
        format_duration(summary.duration_total_ms),
    );

    if summary.failed > 0 {
        println!(
            "  {}",
            style(format!("  {} tasks failed", summary.failed)).red()
        );
    }
}

/// Print regression warnings.
fn print_regressions(regressions: &[Regression]) {
    println!();
    println!("  {} Regressions detected:", style("⚠").yellow().bold());
    for reg in regressions {
        let severity = if reg.delta <= -100.0 {
            style("CRITICAL").red().bold()
        } else if reg.delta <= -50.0 {
            style("MAJOR").red()
        } else {
            style("MINOR").yellow()
        };
        println!(
            "    {} {} — {:.0} → {:.0} (Δ {:.0}) — {}",
            style("↘").red(),
            style(&reg.task_id).bold(),
            reg.previous_score,
            reg.current_score,
            reg.delta,
            severity,
        );
    }
}

/// Format milliseconds as human-readable duration.
fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{:.1}m", ms as f64 / 60_000.0)
    }
}
