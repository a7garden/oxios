//! Oxios Benchmark System
//!
//! A standardized evaluation system for measuring Oxios task execution capabilities.
//! Uses AI agent to issue natural language commands and evaluates task completion.

pub mod analyzer;
pub mod cli;
pub mod collector;
pub mod evaluator;
pub mod report;
pub mod tasks;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkId(pub String);

impl BenchmarkId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

/// A single benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub id: BenchmarkId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub task_results: Vec<TaskResult>,
    pub trace: Option<ExecutionTrace>,
    pub config: BenchmarkConfig,
}

/// Configuration for a benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub categories: Vec<TaskCategory>,
    pub timeout_secs: u64,
    pub collect_events: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            categories: vec![
                TaskCategory::Math,
                TaskCategory::WebSearch,
                TaskCategory::Knowledge,
                TaskCategory::Memory,
            ],
            timeout_secs: 60,
            collect_events: true,
        }
    }
}

/// Task categories for grouping
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskCategory {
    Math,
    WebSearch,
    Knowledge,
    Memory,
    Coding,
    Analysis,
    MultiTurn,
}

impl TaskCategory {
    pub fn label(&self) -> &'static str {
        match self {
            TaskCategory::Math => "math",
            TaskCategory::WebSearch => "web_search",
            TaskCategory::Knowledge => "knowledge",
            TaskCategory::Memory => "memory",
            TaskCategory::Coding => "coding",
            TaskCategory::Analysis => "analysis",
            TaskCategory::MultiTurn => "multi_turn",
        }
    }
}

/// A task definition in the TaskBank
#[derive(Debug, Clone)]
pub struct TaskDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub category: TaskCategory,
    pub command: &'static str,
    pub expected_outcomes: Vec<&'static str>,
    pub evaluation_fn: fn(response: &str) -> TaskResult,
}

/// Result of a single task evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub passed: bool,
    pub score: f64,
    pub response: String,
    pub expected: Vec<String>,
    pub evaluation_notes: String,
    pub duration_ms: u64,
}

/// Collected events during a benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub benchmark_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub events: Vec<KernelEventEnvelope>,
}

impl ExecutionTrace {
    pub fn new(benchmark_id: &str) -> Self {
        Self {
            benchmark_id: benchmark_id.to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            events: Vec::new(),
        }
    }

    pub fn duration_ms(&self) -> u64 {
        (self.end_time - self.start_time).num_milliseconds() as u64
    }
}

/// Kernel event wrapper with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelEventEnvelope {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Aggregated statistics for a benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStats {
    pub total_tasks: usize,
    pub passed: usize,
    pub failed: usize,
    pub average_score: f64,
    pub total_duration_ms: u64,
    pub agents_created: usize,
    pub seeds_created: usize,
    pub spaces_created: usize,
    pub memories_stored: usize,
    pub phases_completed: usize,
}

impl Default for BenchmarkStats {
    fn default() -> Self {
        Self {
            total_tasks: 0,
            passed: 0,
            failed: 0,
            average_score: 0.0,
            total_duration_ms: 0,
            agents_created: 0,
            seeds_created: 0,
            spaces_created: 0,
            memories_stored: 0,
            phases_completed: 0,
        }
    }
}

/// Trace analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceReport {
    pub collection_id: String,
    pub duration_ms: u64,
    pub agents_created: Vec<AgentSummary>,
    pub seeds_created: Vec<SeedSummary>,
    pub spaces_created: Vec<SpaceSummary>,
    pub memories_stored: Vec<MemorySummary>,
    pub phases_completed: Vec<PhaseSummary>,
    pub evaluations: Vec<EvaluationSummary>,
    pub event_count_by_type: HashMap<String, usize>,
}

/// Summary of an agent created during benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

/// Summary of a seed created during benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedSummary {
    pub id: String,
    pub goal: String,
    pub created_at: String,
}

/// Summary of a space created during benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

/// Summary of a memory entry stored during benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySummary {
    pub id: String,
    pub memory_type: String,
    pub source: String,
}

/// Summary of a phase completed during benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSummary {
    pub phase: String,
    pub session_id: String,
    pub result_summary: String,
}

/// Summary of an evaluation during benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub seed_id: String,
    pub passed: bool,
}
