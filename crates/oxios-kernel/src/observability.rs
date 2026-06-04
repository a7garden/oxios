//! Observability — oxi-sdk 0.26.2 tracing, cost tracking, and audit.
//!
//! Provides global instances of oxi-sdk's `Tracer`, `CostTracker`, and `AuditLog`
//! for use across the kernel. These complement the existing `metrics` module
//! (Prometheus counters/gauges) with distributed tracing, per-agent cost
//! accounting, and structured audit logging.
//!
//! # Architecture
//!
//! ```text
//! Global instances (OnceLock):
//!   tracer()     → Tracer      (distributed spans: AgentSpan, ToolSpan, etc.)
//!   cost_tracker() → CostTracker (per-agent token/cost tracking)
//!   audit_log()  → AuditLog    (structured security audit entries)
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use oxios_kernel::observability;
//!
//! // Start a span for an agent execution
//! let _span = observability::tracer().start("seed-execution", observability::SpanKind::Agent);
//!
//! // Log audit entry
//! observability::audit_log()
//!     .log(observability::AuditEntry::tool_execution(
//!         "agent-1".into(),
//!         "exec".into(),
//!         "ls -la".into(),
//!         true,
//!         42,
//!     ));
//! ```

use oxi_sdk::ModelRegistry;
// Re-exports grouped by concern. All names are part of the kernel's public
// surface (re-exported via `lib.rs`) — do not remove or rename without
// auditing downstream consumers.
//
// `audit_trail::*` types (AuditTrail, AuditAction, HashDigest, ...) are
// intentionally NOT re-exported here: they live in the dormant `audit_trail`
// module of oxi-sdk and will be activated in Phase F (RFC-014).
pub use oxi_sdk::{
    // ── Audit (in-memory) ──────────────────────────────────────────────
    // Simple structured audit log. Replaced by `audit_trail` (blake3 chain)
    // in Phase F.
    AuditEntry,
    AuditFilter,
    AuditLog,
    // ── Cost ───────────────────────────────────────────────────────────
    // Per-agent token usage and cost accounting.
    CostBreakdown,
    CostSnapshot,
    CostTracker,
    CostTrackerConfig,
    GlobalCostSnapshot,
    // ── Tracing ────────────────────────────────────────────────────────
    // Distributed spans for agent/tool/kernel operations.
    Span,
    SpanContext,
    SpanGuard,
    SpanId,
    SpanKind,
    SpanStatus,
    TokenUsage,
    TraceId,
    Tracer,
};
use std::sync::Arc;

/// Global Tracer instance.
static TRACER: std::sync::OnceLock<Tracer> = std::sync::OnceLock::new();

/// Global CostTracker instance.
static COST_TRACKER: std::sync::OnceLock<CostTracker> = std::sync::OnceLock::new();

/// Global AuditLog instance.
static AUDIT_LOG: std::sync::OnceLock<AuditLog> = std::sync::OnceLock::new();

/// Get the global Tracer.
///
/// The tracer is lazily initialized on first access.
/// Used for distributed tracing of agent executions, tool calls, and kernel operations.
pub fn tracer() -> &'static Tracer {
    TRACER.get_or_init(Tracer::new)
}

/// Get the global CostTracker.
///
/// The cost tracker uses a minimal ModelRegistry for token cost estimation.
/// Record per-agent token usage after each LLM call.
pub fn cost_tracker() -> &'static CostTracker {
    COST_TRACKER.get_or_init(|| {
        let registry = Arc::new(ModelRegistry::from_static());
        CostTracker::new(registry, CostTrackerConfig::default())
    })
}

/// Get the global AuditLog.
///
/// The audit log stores structured security events (tool calls, access decisions,
/// lifecycle events). Entries can be queried by agent, action type, or time range.
pub fn audit_log() -> &'static AuditLog {
    AUDIT_LOG.get_or_init(|| AuditLog::new(1024))
}

/// Initialize all observability instances.
///
/// Call during kernel startup to ensure all instances are warm.
/// Non-blocking — just triggers lazy initialization.
pub fn init() {
    let _ = tracer();
    let _ = cost_tracker();
    let _ = audit_log();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracer_smoke() {
        let t = tracer();
        let _guard = t.start("test-span", SpanKind::Agent);
        // Span is active while guard is in scope
        drop(_guard);
    }

    #[test]
    fn test_cost_tracker_smoke() {
        let ct = cost_tracker();
        let model = oxi_sdk::Model::new(
            "test/model",
            "Test",
            oxi_sdk::Api::OpenAiCompletions,
            "test",
            "https://test.com",
        );
        ct.record(
            "test-agent",
            &model,
            TokenUsage {
                input: 100,
                output: 50,
                cache_read: 0,
                cache_write: 0,
            },
        );
        let snap = ct.snapshot("test-agent");
        assert!(snap.is_some());
    }

    #[test]
    fn test_audit_log_smoke() {
        let al = audit_log();
        al.log(AuditEntry::lifecycle("test-agent".into(), "started".into()));
        let entries = al.query(AuditFilter {
            agent_id: Some("test-agent".to_string()),
            entry_type: None,
            after_ms: None,
        });
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_init_idempotent() {
        init();
        init(); // Should not panic
    }
}
