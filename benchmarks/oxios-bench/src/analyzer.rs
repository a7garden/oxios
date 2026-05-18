//! Trace analyzer for benchmark results
//!
//! Analyzes collected execution traces and produces structured reports.

use std::collections::HashMap;

use crate::{AgentSummary, EvaluationSummary, ExecutionTrace, MemorySummary, PhaseSummary, SeedSummary, SpaceSummary, TraceReport};

/// Extended trace analyzer with detailed event analysis
pub struct DetailedAnalyzer;

impl DetailedAnalyzer {
    /// Count events by type
    pub fn count_events(trace: &ExecutionTrace) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for event in &trace.events {
            *counts.entry(event.event_type.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Extract all agent IDs from trace
    pub fn extract_agents(trace: &ExecutionTrace) -> Vec<AgentSummary> {
        trace
            .events
            .iter()
            .filter(|e| e.event_type == "AgentCreated")
            .filter_map(|e| {
                Some(AgentSummary {
                    id: e.payload.get("id")?.as_str()?.to_string(),
                    name: e.payload.get("name")?.as_str()?.to_string(),
                    created_at: e.timestamp.to_rfc3339(),
                })
            })
            .collect()
    }

    /// Extract all seed IDs from trace
    pub fn extract_seeds(trace: &ExecutionTrace) -> Vec<SeedSummary> {
        trace
            .events
            .iter()
            .filter(|e| e.event_type == "SeedCreated")
            .filter_map(|e| {
                Some(SeedSummary {
                    id: e.payload.get("seed_id")?.as_str()?.to_string(),
                    goal: e.payload.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    created_at: e.timestamp.to_rfc3339(),
                })
            })
            .collect()
    }

    /// Extract all spaces from trace
    pub fn extract_spaces(trace: &ExecutionTrace) -> Vec<SpaceSummary> {
        trace
            .events
            .iter()
            .filter(|e| e.event_type == "SpaceCreated")
            .filter_map(|e| {
                Some(SpaceSummary {
                    id: e.payload.get("space_id")?.as_str()?.to_string(),
                    name: e.payload.get("name")?.as_str()?.to_string(),
                    created_at: e.timestamp.to_rfc3339(),
                })
            })
            .collect()
    }

    /// Extract all memories from trace
    pub fn extract_memories(trace: &ExecutionTrace) -> Vec<MemorySummary> {
        trace
            .events
            .iter()
            .filter(|e| e.event_type == "MemoryStored")
            .filter_map(|e| {
                Some(MemorySummary {
                    id: e.payload.get("id")?.as_str()?.to_string(),
                    memory_type: e.payload.get("memory_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    source: e.payload.get("source").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                })
            })
            .collect()
    }

    /// Extract all phases from trace
    pub fn extract_phases(trace: &ExecutionTrace) -> Vec<PhaseSummary> {
        trace
            .events
            .iter()
            .filter(|e| e.event_type == "PhaseCompleted")
            .filter_map(|e| {
                Some(PhaseSummary {
                    phase: e.payload.get("phase").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    session_id: e.payload.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    result_summary: e.payload.get("result_summary").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect()
    }

    /// Extract all evaluations from trace
    pub fn extract_evaluations(trace: &ExecutionTrace) -> Vec<EvaluationSummary> {
        trace
            .events
            .iter()
            .filter(|e| e.event_type == "EvaluationComplete")
            .filter_map(|e| {
                Some(EvaluationSummary {
                    seed_id: e.payload.get("seed_id")?.as_str()?.to_string(),
                    passed: e.payload.get("passed").and_then(|v| v.as_bool()).unwrap_or(false),
                })
            })
            .collect()
    }

    /// Build a complete trace report
    pub fn build_report(trace: &ExecutionTrace) -> TraceReport {
        crate::collector::TraceAnalyzer::analyze(trace)
    }
}

/// Event timeline for debugging
pub struct EventTimeline<'a> {
    trace: &'a ExecutionTrace,
}

impl<'a> EventTimeline<'a> {
    pub fn new(trace: &'a ExecutionTrace) -> Self {
        Self { trace }
    }

    pub fn print(&self) {
        println!("\n=== Event Timeline ({}) ===", self.trace.benchmark_id);
        for (i, event) in self.trace.events.iter().enumerate() {
            println!("{}. [{}] {}", i + 1, event.timestamp.format("%H:%M:%S%.3f"), event.event_type);
            if let Ok(pretty) = serde_json::to_string_pretty(&event.payload) {
                for line in pretty.lines().take(3) {
                    println!("   {}", line);
                }
            }
        }
        println!();
    }
}