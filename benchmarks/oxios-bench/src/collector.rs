//! Event collector for benchmark traces
//!
//! Subscribes to Oxios event bus and collects events during benchmark execution.

use chrono::Utc;

use crate::{
    AgentSummary, EvaluationSummary, ExecutionTrace, KernelEventEnvelope, MemorySummary,
    PhaseSummary, SeedSummary, SpaceSummary, TraceReport,
};

/// Event collector that subscribes to the Oxios kernel event bus.
///
/// Note: In this standalone benchmark binary, we collect events by polling
/// the Oxios HTTP API and parsing SSE events. The collector service itself
/// is designed to work with direct kernel event bus subscription when
/// running as part of the Oxios process.
pub struct EventCollector {
    collection_id: String,
    events: Vec<KernelEventEnvelope>,
    start_time: chrono::DateTime<Utc>,
}

impl EventCollector {
    pub fn new(collection_id: &str) -> Self {
        Self {
            collection_id: collection_id.to_string(),
            events: Vec::new(),
            start_time: Utc::now(),
        }
    }

    /// Record an event from the kernel
    pub fn record_event(&mut self, event_type: &str, payload: serde_json::Value) {
        self.events.push(KernelEventEnvelope {
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            payload,
        });
    }

    /// Build execution trace from collected events
    pub fn build_trace(mut self) -> ExecutionTrace {
        let mut trace = ExecutionTrace::new(&self.collection_id);
        trace.start_time = self.start_time;
        trace.end_time = Utc::now();
        trace.events = std::mem::take(&mut self.events);
        trace
    }

    /// Parse events from an SSE event string
    pub fn parse_sse_event(data: &str) -> Option<KernelEventEnvelope> {
        let event_type = data.lines().find(|l| l.starts_with("event:")).map(|l| {
            l.strip_prefix("event:").unwrap_or("unknown").trim().to_string()
        })?;

        let json_data = data.lines().find(|l| !l.starts_with("event:") && !l.starts_with("data:")).map(|l| l.trim()).unwrap_or("{}");

        let payload: serde_json::Value = serde_json::from_str(json_data).ok()?;

        Some(KernelEventEnvelope {
            timestamp: Utc::now(),
            event_type,
            payload,
        })
    }
}

/// Analyzer for execution traces
pub struct TraceAnalyzer;

impl TraceAnalyzer {
    /// Analyze an execution trace and produce a structured report
    pub fn analyze(trace: &ExecutionTrace) -> TraceReport {
        let mut agents = Vec::new();
        let mut seeds = Vec::new();
        let mut spaces = Vec::new();
        let mut memories = Vec::new();
        let mut phases = Vec::new();
        let mut evaluations = Vec::new();
        let mut event_counts = std::collections::HashMap::new();

        for event in &trace.events {
            *event_counts.entry(event.event_type.clone()).or_insert(0) += 1;

            match event.event_type.as_str() {
                "AgentCreated" => {
                    if let (Some(id), Some(name)) = (
                        event.payload.get("id").and_then(|v| v.as_str()),
                        event.payload.get("name").and_then(|v| v.as_str()),
                    ) {
                        agents.push(AgentSummary {
                            id: id.to_string(),
                            name: name.to_string(),
                            created_at: event.timestamp.to_rfc3339(),
                        });
                    }
                }
                "SeedCreated" => {
                    if let Some(seed_id) = event.payload.get("seed_id").and_then(|v| v.as_str()) {
                        seeds.push(SeedSummary {
                            id: seed_id.to_string(),
                            goal: event.payload.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            created_at: event.timestamp.to_rfc3339(),
                        });
                    }
                }
                "SpaceCreated" => {
                    if let (Some(space_id), Some(name)) = (
                        event.payload.get("space_id").and_then(|v| v.as_str()),
                        event.payload.get("name").and_then(|v| v.as_str()),
                    ) {
                        spaces.push(SpaceSummary {
                            id: space_id.to_string(),
                            name: name.to_string(),
                            created_at: event.timestamp.to_rfc3339(),
                        });
                    }
                }
                "MemoryStored" => {
                    if let Some(id) = event.payload.get("id").and_then(|v| v.as_str()) {
                        memories.push(MemorySummary {
                            id: id.to_string(),
                            memory_type: event.payload.get("memory_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                            source: event.payload.get("source").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                        });
                    }
                }
                "PhaseCompleted" => {
                    phases.push(PhaseSummary {
                        phase: event.payload.get("phase").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                        session_id: event.payload.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        result_summary: event.payload.get("result_summary").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    });
                }
                "EvaluationComplete" => {
                    if let Some(seed_id) = event.payload.get("seed_id").and_then(|v| v.as_str()) {
                        let passed = event.payload.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
                        evaluations.push(EvaluationSummary {
                            seed_id: seed_id.to_string(),
                            passed,
                        });
                    }
                }
                _ => {}
            }
        }

        TraceReport {
            collection_id: trace.benchmark_id.clone(),
            duration_ms: trace.duration_ms(),
            agents_created: agents,
            seeds_created: seeds,
            spaces_created: spaces,
            memories_stored: memories,
            phases_completed: phases,
            evaluations,
            event_count_by_type: event_counts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_event_collector() {
        let mut collector = EventCollector::new("test-123");
        collector.record_event("AgentCreated", json!({"id": "agent-1", "name": "test-agent"}));
        let trace = collector.build_trace();
        assert_eq!(trace.events.len(), 1);
    }

    #[test]
    fn test_trace_analyzer() {
        let trace = ExecutionTrace::new("test-123");
        let report = TraceAnalyzer::analyze(&trace);
        assert_eq!(report.collection_id, "test-123");
    }
}