//! Oxios Benchmark System v2
//!
//! A structured evaluation system for measuring Oxios Agent OS capabilities.
//! Uses `oxios run --json` structured output for deterministic evaluation.
//!
//! # Architecture
//!
//! ```text
//! TOML Task Definitions
//!       │
//!       ▼
//!   Runner (Unit / Kernel / Process)
//!       │
//!       ▼
//!   Evaluator (Structural + Content + LLM-Judge)
//!       │
//!       ▼
//!   Reporter (JSON + Console + Compare)
//! ```

pub mod config;
pub mod eval;
pub mod fixture;
pub mod report;
pub mod runner;
pub mod suite;
pub mod task;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Benchmark tier — determines how tasks are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Direct API calls, no LLM involved. Sub-millisecond.
    Unit,
    /// In-process kernel call. Tests subsystem integration.
    Integration,
    /// Spawn `oxios run --json` as subprocess. Full end-to-end.
    E2e,
}

impl Tier {
    /// All tier variants in order.
    pub fn all() -> &'static [Tier] {
        &[Tier::Unit, Tier::Integration, Tier::E2e]
    }

    /// Parse from string (case-insensitive).
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "unit" => Some(Tier::Unit),
            "integration" | "int" => Some(Tier::Integration),
            "e2e" | "end-to-end" => Some(Tier::E2e),
            _ => None,
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier::Unit => write!(f, "unit"),
            Tier::Integration => write!(f, "integration"),
            Tier::E2e => write!(f, "e2e"),
        }
    }
}

/// Oxios Ouroboros phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Interview,
    Seed,
    Execute,
    Evaluate,
    Evolve,
}

impl Phase {
    /// Parse from the `phase_reached` field of `oxios run --json`.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "interview" => Some(Phase::Interview),
            "seed" => Some(Phase::Seed),
            "execute" => Some(Phase::Execute),
            "evaluate" => Some(Phase::Evaluate),
            "evolve" => Some(Phase::Evolve),
            _ => None,
        }
    }

    /// All phases in order.
    pub fn ordered() -> &'static [Phase] {
        &[
            Phase::Interview,
            Phase::Seed,
            Phase::Execute,
            Phase::Evaluate,
            Phase::Evolve,
        ]
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Interview => write!(f, "Interview"),
            Phase::Seed => write!(f, "Seed"),
            Phase::Execute => write!(f, "Execute"),
            Phase::Evaluate => write!(f, "Evaluate"),
            Phase::Evolve => write!(f, "Evolve"),
        }
    }
}

/// Output from a single `oxios run --json` execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutput {
    /// The response text.
    pub response: String,
    /// Session ID for multi-turn.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Project ID that handled the message.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Project tag (e.g. "[🔧 oxios]").
    #[serde(default)]
    pub project_tag: Option<String>,
    /// Seed ID if seed was created.
    #[serde(default)]
    pub seed_id: Option<String>,
    /// Agent ID if agent executed.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Furthest Ouroboros phase reached.
    pub phase_reached: String,
    /// Whether Ouroboros evaluation passed.
    pub evaluation_passed: bool,
    /// Process exit code (0=success, 1=eval failed).
    pub exit_code: i32,
    /// Wall-clock duration in ms.
    pub duration_ms: u64,
    /// Workspace root for filesystem assertions.
    #[serde(skip)]
    pub workspace: PathBuf,
}

impl RunOutput {
    /// Parse from `oxios run --json` stdout.
    pub fn from_json_str(json: &str, workspace: PathBuf) -> anyhow::Result<Self> {
        let val: serde_json::Value = serde_json::from_str(json)?;
        Ok(Self {
            response: val["response"].as_str().unwrap_or("").to_string(),
            session_id: val["session_id"].as_str().map(|s| s.to_string()),
            project_id: val["primary_project_id"].as_str().map(|s| s.to_string()),
            project_tag: val["project_tag"].as_str().map(|s| s.to_string()),
            seed_id: val["seed_id"].as_str().map(|s| s.to_string()),
            agent_id: val["agent_id"].as_str().map(|s| s.to_string()),
            phase_reached: val["phase_reached"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            evaluation_passed: val["evaluation_passed"].as_bool().unwrap_or(false),
            exit_code: val["exit_code"].as_i64().unwrap_or(-1) as i32,
            duration_ms: val["duration_ms"].as_u64().unwrap_or(0),
            workspace,
        })
    }

    /// Parse from raw `oxios run --json` process output (stdout + exit code).
    pub fn from_process_output(stdout: &str, exit_code: i32, workspace: PathBuf) -> Self {
        let val: serde_json::Value = serde_json::from_str(stdout).unwrap_or_default();
        Self {
            response: val["response"].as_str().unwrap_or("").to_string(),
            session_id: val["session_id"].as_str().map(|s| s.to_string()),
            project_id: val["primary_project_id"].as_str().map(|s| s.to_string()),
            project_tag: val["project_tag"].as_str().map(|s| s.to_string()),
            seed_id: val["seed_id"].as_str().map(|s| s.to_string()),
            agent_id: val["agent_id"].as_str().map(|s| s.to_string()),
            phase_reached: val["phase_reached"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            evaluation_passed: val["evaluation_passed"].as_bool().unwrap_or(false),
            exit_code,
            duration_ms: val["duration_ms"].as_u64().unwrap_or(0),
            workspace,
        }
    }

    /// Get the phase as a typed enum.
    pub fn phase(&self) -> Option<Phase> {
        Phase::from_str_opt(&self.phase_reached)
    }
}

/// Result of evaluating a single assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Human-readable description of what was checked.
    pub assertion: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// What was actually observed.
    pub actual: String,
    /// What was expected.
    pub expected: String,
}

/// Result of running a single benchmark task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID.
    pub task_id: String,
    /// Task name.
    pub task_name: String,
    /// Suite this task belongs to.
    pub suite: String,
    /// Tier of this task.
    pub tier: Tier,
    /// Overall pass/fail.
    pub passed: bool,
    /// Score 0-100.
    pub score: f64,
    /// Per-assertion results.
    pub assertion_results: Vec<AssertionResult>,
    /// Wall-clock duration in ms.
    pub duration_ms: u64,
    /// Error message if the task could not be executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A regression detected between two runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    /// Task ID that regressed.
    pub task_id: String,
    /// Score in the baseline run.
    pub previous_score: f64,
    /// Score in the current run.
    pub current_score: f64,
    /// Delta (negative = regression).
    pub delta: f64,
}

/// Aggregate summary of a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    /// Total tasks executed.
    pub total: usize,
    /// Tasks that passed.
    pub passed: usize,
    /// Tasks that failed.
    pub failed: usize,
    /// Tasks that were skipped.
    pub skipped: usize,
    /// Average score across all tasks.
    pub score_avg: f64,
    /// Total wall-clock duration in ms.
    pub duration_total_ms: u64,
    /// Regressions compared to baseline.
    pub regressions: Vec<Regression>,
}

/// A complete benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    /// Unique run ID.
    pub id: String,
    /// When this run started.
    pub timestamp: DateTime<Utc>,
    /// Oxios version (from `oxios --version`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oxios_version: Option<String>,
    /// Git ref (current HEAD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    /// Per-task results.
    pub results: Vec<TaskResult>,
    /// Aggregate summary.
    pub summary: RunSummary,
}
