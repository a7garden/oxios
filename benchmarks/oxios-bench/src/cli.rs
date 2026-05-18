//! CLI for running Oxios benchmarks
//!
//! Usage:
//!   oxios-bench run                    # Run all tasks
//!   oxios-bench run --category math    # Run math tasks only
//!   oxios-bench run --task math_simple # Run single task
//!   oxios-bench report --latest         # Show latest report

use chrono::Utc;
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::{BenchmarkConfig, BenchmarkId, BenchmarkRun, TaskCategory, TaskResult};
use crate::report::{compute_stats, load_report, print_summary, save_report};

#[derive(Debug, Serialize)]
struct ChatRequest {
    message: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatResponse {
    id: String,
    echo: String,
    reply: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    phase: Option<String>,
}

/// Run a single benchmark task
async fn run_task(
    client: &Client,
    base_url: &str,
    task: &crate::TaskDefinition,
    _collector: &mut crate::collector::EventCollector,
) -> TaskResult {
    let start = Instant::now();

    println!("  Running: {} ({})", task.name, task.command);

    // Send chat request
    let resp = client
        .post(&format!("{}/api/chat", base_url))
        .json(&ChatRequest {
            message: task.command.to_string(),
        })
        .send()
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to send request: {}", e)
        });

    let response_text = if resp.status().is_success() {
        let status = resp.status();
        let chat_resp: ChatResponse = resp.json().await.unwrap_or(ChatResponse {
            id: "".to_string(),
            echo: "".to_string(),
            reply: format!("ERROR: status {}", status),
            session_id: None,
            phase: None,
        });
        chat_resp.reply
    } else {
        format!("ERROR: HTTP {}", resp.status().as_u16())
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // Evaluate response
    let mut result = (task.evaluation_fn)(&response_text);
    result.task_id = task.id.to_string();
    result.duration_ms = duration_ms;

    result
}

/// Run all benchmark tasks
async fn run_benchmark(
    base_url: &str,
    tasks: Vec<crate::TaskDefinition>,
) -> BenchmarkRun {
    let client = Client::new();
    let mut collector = crate::collector::EventCollector::new(&BenchmarkId::new().0);
    let mut task_results = Vec::new();

    println!("\nStarting benchmark with {} tasks...\n", tasks.len());

    for task in tasks {
        let result = run_task(&client, base_url, &task, &mut collector).await;
        let status = if result.passed { "✅" } else { "❌" };
        println!(
            "  {} {} - Score: {:.0}% ({}) [{}ms]",
            status,
            result.task_id,
            result.score,
            result.evaluation_notes,
            result.duration_ms
        );
        task_results.push(result);
    }

    BenchmarkRun {
        id: BenchmarkId::new(),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        task_results,
        trace: None,
        config: BenchmarkConfig::default(),
    }
}

#[derive(Parser, Debug)]
#[command(name = "oxios-bench")]
#[command(about = "Benchmark system for Oxios task execution evaluation")]
enum Commands {
    /// Run benchmark tasks
    Run {
        /// Task category to run (math, web_search, knowledge, memory, coding, multi_turn)
        #[arg(long)]
        category: Option<String>,

        /// Specific task ID to run
        #[arg(long)]
        task: Option<String>,

        /// Base URL for Oxios API
        #[arg(long, default_value = "http://127.0.0.1:4200")]
        url: String,
    },
    /// Show benchmark report
    Report {
        /// Report ID (or 'latest')
        #[arg(long, default_value = "latest")]
        id: String,

        /// Path to load report from
        #[arg(long)]
        path: Option<String>,
    },
}

/// Run the CLI
pub async fn run() -> anyhow::Result<()> {
    let args = Commands::parse();

    match args {
        Commands::Run {
            category,
            task,
            url,
        } => {
            let tasks = if let Some(task_id) = task {
                vec![crate::tasks::task_by_id(&task_id).expect("Unknown task ID")]
            } else if let Some(cat) = category {
                let cat = match cat.as_str() {
                    "math" => TaskCategory::Math,
                    "web_search" => TaskCategory::WebSearch,
                    "knowledge" => TaskCategory::Knowledge,
                    "memory" => TaskCategory::Memory,
                    "coding" => TaskCategory::Coding,
                    "multi_turn" => TaskCategory::MultiTurn,
                    _ => panic!("Unknown category: {}", cat),
                };
                crate::tasks::tasks_by_category(cat)
            } else {
                crate::tasks::all_tasks()
            };

            let run = run_benchmark(&url, tasks).await;

            // Print summary
            let stats = compute_stats(&run);
            println!("\n{}", "=".repeat(60));
            println!("BENCHMARK COMPLETE");
            println!("{}", "=".repeat(60));
            println!(
                "Total: {} | Passed: {} | Failed: {}",
                stats.total_tasks, stats.passed, stats.failed
            );
            println!("Average Score: {:.1}%", stats.average_score);
            println!("Total Duration: {}ms", stats.total_duration_ms);

            // Save report
            let report_path = format!(".oxios-bench/reports/{}.json", run.id.0);
            std::fs::create_dir_all(".oxios-bench/reports").ok();
            save_report(&run, &report_path)?;

            Ok(())
        }
        Commands::Report { id: _, path } => {
            if let Some(path) = path {
                let run = load_report(&path)?;
                print_summary(&run);
            } else {
                println!("Loading from reports directory...");
                let entries = std::fs::read_dir(".oxios-bench/reports").ok();
                if let Some(entries) = entries {
                    for entry in entries.flatten() {
                        if let Ok(name) = entry.file_name().into_string() {
                            if name.ends_with(".json") {
                                println!("  - {}", name);
                            }
                        }
                    }
                } else {
                    println!("No reports found. Run 'oxios-bench run' first.");
                }
            }
            Ok(())
        }
    }
}