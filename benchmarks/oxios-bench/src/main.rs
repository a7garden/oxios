//! Oxios Benchmark System v2 — CLI entry point.

use clap::Parser;
use oxios_bench::config::BenchConfig;
use oxios_bench::report::compare::{compare_runs, print_compare};
use oxios_bench::report::console::print_report;
use oxios_bench::report::json_report::{build_summary, load_report, save_report};
use oxios_bench::runner::process_runner::ProcessRunner;
use oxios_bench::runner::Runner;
use oxios_bench::suite::{filter_tasks, Suite};
use oxios_bench::{BenchmarkRun, Tier};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "oxios-bench")]
#[command(about = "Oxios Benchmark System v2 — structured evaluation for Agent OS")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Run benchmark tasks
    Run {
        /// Tier to run: unit, integration, e2e, all
        #[arg(long, default_value = "all")]
        tier: String,

        /// Run only a specific suite (e.g. ouroboros, agent, tool)
        #[arg(long)]
        suite: Option<String>,

        /// Run tasks matching a tag (e.g. @smoke, @core)
        #[arg(long)]
        tag: Option<String>,

        /// Run a single task by ID
        #[arg(long)]
        task: Option<String>,

        /// Path to oxios binary
        #[arg(long)]
        bin: Option<String>,

        /// Maximum concurrent tasks
        #[arg(long, default_value = "1")]
        parallel: usize,

        /// Per-task timeout in seconds
        #[arg(long, default_value = "120")]
        timeout: u64,

        /// Compare against baseline for regressions
        #[arg(long)]
        baseline: Option<String>,

        /// Output as JSON only
        #[arg(long)]
        json: bool,

        /// Show per-assertion details
        #[arg(long)]
        verbose: bool,

        /// Custom suites directory
        #[arg(long)]
        suites_dir: Option<String>,
    },

    /// List available tasks and suites
    List {
        /// Custom suites directory
        #[arg(long)]
        suites_dir: Option<String>,
    },

    /// Compare two benchmark runs
    Compare {
        /// Path to baseline report JSON
        baseline: String,

        /// Path to current report JSON
        current: String,

        /// Score delta threshold for regression detection
        #[arg(long, default_value = "5.0")]
        threshold: f64,
    },

    /// Show a saved benchmark report
    Show {
        /// Path to the report JSON
        path: String,

        /// Show per-assertion details
        #[arg(long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() {
    // Initialize logging (quiet by default)
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let cli = Cli::parse();

    if let Err(e) = run_cli(cli).await {
        eprintln!("Error: {}", e);
        std::process::exit(3);
    }
}

async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Run {
            tier,
            suite,
            tag,
            task,
            bin,
            parallel,
            timeout,
            baseline,
            json,
            verbose,
            suites_dir,
        } => {
            let mut config = BenchConfig::default();
            config.parallel = parallel;
            config.timeout_secs = timeout;

            if let Some(bin_path) = bin {
                config.oxios_bin = std::path::PathBuf::from(bin_path);
            }
            if let Some(dir) = suites_dir {
                config.suites_dir = std::path::PathBuf::from(dir);
            }

            // Parse tier filter
            let tier_filter = if tier == "all" {
                None
            } else {
                Some(
                    Tier::from_str_opt(&tier)
                        .ok_or_else(|| anyhow::anyhow!("Unknown tier: {}. Use: unit, integration, e2e, all", tier))?
                )
            };

            // Load suites
            let suites = Suite::load_all(&config.suites_dir)?;

            // Filter tasks
            let tasks = filter_tasks(
                &suites,
                tier_filter,
                suite.as_deref(),
                tag.as_deref(),
                task.as_deref(),
            );

            if tasks.is_empty() {
                println!("No tasks matched the given filters.");
                return Ok(());
            }

            println!(
                "Running {} task(s) with {} runner...\n",
                tasks.len(),
                config.oxios_bin.display()
            );

            // Execute tasks
            let runner = ProcessRunner::new(config.oxios_bin.clone());
            let mut results = Vec::new();
            let run_start = Instant::now();

            for task in &tasks {
                let task_start = Instant::now();
                print!("  {} ... ", task.id);
                std::io::Write::flush(&mut std::io::stdout()).ok();

                match runner.run(task).await {
                    Ok(output) => {
                        let duration_ms = task_start.elapsed().as_millis() as u64;
                        let result = oxios_bench::runner::evaluate_task(task, &output, duration_ms);
                        let icon = if result.passed { "✅" } else { "❌" };
                        println!(
                            "{} ({:.0}%, {}ms)",
                            icon, result.score, result.duration_ms
                        );
                        results.push(result);
                    }
                    Err(e) => {
                        println!("💥 error: {}", e);
                        results.push(oxios_bench::TaskResult {
                            task_id: task.id.clone(),
                            task_name: task.name.clone(),
                            suite: task.suite.clone(),
                            tier: task.tier,
                            passed: false,
                            score: 0.0,
                            assertion_results: vec![],
                            duration_ms: task_start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }

            let _total_ms = run_start.elapsed().as_millis() as u64;

            // Build regressions if baseline provided
            let regressions = if let Some(baseline_path) = &baseline {
                match load_report(baseline_path) {
                    Ok(baseline_run) => {
                        let current_run = make_run(&results, &regressions_from_results(&baseline_run, &results));
                        let cmp = compare_runs(&baseline_run, &current_run, 5.0);
                        if !json {
                            print_compare(&cmp);
                        }
                        cmp.regressions
                    }
                    Err(e) => {
                        eprintln!("Warning: Could not load baseline: {}", e);
                        vec![]
                    }
                }
            } else {
                vec![]
            };

            // Build final run
            let run = make_run(&results, &regressions);

            // Output
            if json {
                let json_str = oxios_bench::report::json_report::generate_json(&run)?;
                println!("{}", json_str);
            } else {
                print_report(&run, verbose);
            }

            // Save report
            let report_path = format!(".oxios-bench/reports/{}.json", run.id);
            match save_report(&run, &report_path) {
                Ok(()) => println!("  Report saved: {}", report_path),
                Err(e) => eprintln!("  Warning: Could not save report: {}", e),
            }

            // Exit code
            let failed = run.summary.failed;
            let has_regressions = !run.summary.regressions.is_empty();

            if has_regressions {
                std::process::exit(2);
            } else if failed > 0 {
                std::process::exit(1);
            }
        }

        Commands::List { suites_dir } => {
            let config = BenchConfig {
                suites_dir: suites_dir
                    .map(std::path::PathBuf::from)
                    .unwrap_or(BenchConfig::default().suites_dir),
                ..BenchConfig::default()
            };

            let suites = Suite::load_all(&config.suites_dir)?;
            let mut total = 0;

            for suite in &suites {
                println!(
                    "  {} ({} tasks):",
                    console::style(&suite.name).bold().cyan(),
                    suite.tasks.len()
                );
                for task in &suite.tasks {
                    println!(
                        "    {} [{}] {} — {}",
                        console::style(&task.id).bold(),
                        task.tier,
                        task.name,
                        if task.tags.is_empty() {
                            String::new()
                        } else {
                            format!("@{}", task.tags.join(" @"))
                        },
                    );
                }
                total += suite.tasks.len();
                println!();
            }

            println!("  Total: {} tasks in {} suites", total, suites.len());
        }

        Commands::Compare {
            baseline,
            current,
            threshold,
        } => {
            let baseline_run = load_report(&baseline)?;
            let current_run = load_report(&current)?;
            let result = compare_runs(&baseline_run, &current_run, threshold);
            print_compare(&result);
            if result.has_regressions {
                std::process::exit(2);
            }
        }

        Commands::Show { path, verbose } => {
            let run = load_report(&path)?;
            print_report(&run, verbose);
        }
    }

    Ok(())
}

fn make_run(results: &[oxios_bench::TaskResult], regressions: &[oxios_bench::Regression]) -> BenchmarkRun {
    let summary = build_summary(results, regressions.to_vec());
    let id = uuid::Uuid::new_v4().to_string();

    // Try to get git ref
    let git_ref = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    BenchmarkRun {
        id,
        timestamp: chrono::Utc::now(),
        oxios_version: None,
        git_ref,
        results: results.to_vec(),
        summary,
    }
}

fn regressions_from_results(
    _baseline: &BenchmarkRun,
    _current: &[oxios_bench::TaskResult],
) -> Vec<oxios_bench::Regression> {
    // Simplified: actual regression detection happens in compare_runs
    vec![]
}
