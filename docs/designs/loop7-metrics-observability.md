# Metrics & Observability — Loop 7 Design

## Status
**Author:** Loop 7  
**Created:** 2026-05-07  
**Updated:** 2026-05-07  
**Owner:** Oxios Team

---

## 1. Overview

### Why Observability?

Oxios Agent OS runs autonomous LLM agents that are inherently unpredictable. Unlike deterministic services, agent behavior emerges from model inference, tool-calling loops, and real-time environment interactions — making traditional APM insufficient.

We need observability to answer questions like:
- **Is an agent stuck?** (tool call duration, loop iteration count)
- **What is consuming my LLM quota?** (per-phase LLM calls, token estimates)
- **Which seeds are failing, and why?** (evaluation scores, error rates per tool)
- **Is the system healthy?** (active sessions, queue depth, container health)

The three-pillar approach — Metrics, Logs, Traces — provides complementary views: aggregated quantitative signals, point-in-time context, and causal event chains.

### Design Goals

1. **No external dependencies** — all metrics/stats exposed via in-process endpoints
2. **Low overhead** — lock-free counters where possible, histogram-based aggregations, log sampling at high volume
3. **Structured by default** — all log output is machine-parseable JSON
4. **Correlation IDs** — every orchestration gets a trace_id, propagated across phases, tools, and event bus messages
5. **Production-grade** — Prometheus-compatible metrics endpoint, OpenTelemetry trace export support

---

## 2. Three Pillars

### 2.1 Metrics — Prometheus Counters/Gauges/Histograms

Metrics are numerical measurements aggregated over time. They answer "how much?" and "how fast?" questions.

**Metric types:**
- **Counter** — monotonic, incremented on events (e.g., `oxios_agents_forked_total`)
- **Gauge** — current value, can go up or down (e.g., `oxios_agents_running`)
- **Histogram** — distribution of values with configurable buckets (e.g., `oxios_llm_duration_seconds`)

Histograms enable pre-computed percentiles (p50, p95, p99) via Prometheus client-side aggregation without storing raw data.

### 2.2 Logs — Structured JSON Events

Logs answer "what happened, when, and in what context?" Each log entry is a structured JSON object with a consistent schema including correlation IDs (trace_id, span_id, session_id).

**Design principles:**
- Structured fields over free text — `tool: "container_exec"` not `"container exec called"`
- Include all relevant context in each entry — agent_id, seed_id, session_id, phase, duration
- Log levels: `ERROR` for failures, `WARN` for degraded state, `INFO` for lifecycle events, `DEBUG` for instrumentation
- Sampling at high volume: full detail on errors, 1% sample of successful tool calls in steady state

### 2.3 Traces — OpenTelemetry Spans

Traces answer "what was the causal chain of events?" Each `handle_message` call starts a root span. Sub-spans cover each Ouroboros phase. Tool executions become child spans with timing and metadata.

**Benefits over logs alone:**
- Latency breakdown per phase (e.g., "interview phase took 2.3s, evaluate phase took 800ms")
- Causal attribution: "this tool error occurred within which agent's execution?"
- Distributed trace propagation: even without a full distributed system, OTel traces integrate with external tooling (Jaeger, Grafana Tempo, Honeycomb)

**Implementation note:** Tracing is lightweight — it uses `tracing` spans with `tracing-opentelemetry` for export. Spans are only exported to an OTel collector if one is configured. Without a collector, tracing adds negligible overhead (it becomes a no-op).

---

## 3. Key Metrics

### 3.1 Agent Lifecycle

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_agents_forked_total` | Counter | `priority` | Total agents forked |
| `oxios_agents_running` | Gauge | — | Current running agents |
| `oxios_agents_completed_total` | Counter | `success` | Agents completed (true/false) |
| `oxios_agents_failed_total` | Counter | `reason` | Agents failed by reason |

### 3.2 Messages & Orchestration

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_messages_processed_total` | Counter | `phase` | User messages processed |
| `oxios_orchestration_duration_seconds` | Histogram | — | Full Ouroboros loop duration |
| `oxios_phase_duration_seconds` | Histogram | `phase` | Per-phase duration (interview/seed/execute/evaluate/evolve) |

### 3.3 LLM Calls

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_llm_calls_total` | Counter | `phase` | LLM API calls |
| `oxios_llm_duration_seconds` | Histogram | `model` | LLM API call duration |
| `oxios_llm_errors_total` | Counter | `model`, `error_type` | LLM API errors |
| `oxios_llm_tokens_total` | Counter | `model`, `type` | Tokens used (input/output) |

### 3.4 Tool Execution

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_tool_calls_total` | Counter | `tool` | Tool calls by name |
| `oxios_tool_duration_seconds` | Histogram | `tool` | Tool execution duration |
| `oxios_tool_errors_total` | Counter | `tool`, `error_type` | Tool errors |

Tier 1 (oxi native): `read`, `write`, `edit`, `grep`, `find`, `ls`  
Tier 2 (oxios): `container_exec`, `host_exec`  
Tier 3 (programs): dynamic, based on loaded programs  
Tier 4 (MCP): dynamic, based on MCP server registration

### 3.5 Memory

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_memory_entries_total` | Gauge | `type` | Current memory entries by type |
| `oxios_memory_recall_total` | Counter | — | Recall operations |
| `oxios_memory_recall_latency_seconds` | Histogram | — | Recall latency |
| `oxios_memory_store_total` | Counter | `type` | Store operations |

### 3.6 Container & Workspace

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_container_exec_total` | Counter | `command_type` | Container exec calls |
| `oxios_container_exec_duration_seconds` | Histogram | `command_type` | Container exec duration |
| `oxios_container_health` | Gauge | `container_id` | Container health status (0/1) |
| `oxios_workspace_size_bytes` | Gauge | — | Workspace disk usage |

### 3.7 Sessions & Scheduler

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_active_sessions` | Gauge | — | Current active user sessions |
| `oxios_scheduler_queued_tasks` | Gauge | `priority` | Queued tasks by priority |
| `oxios_scheduler_running_tasks` | Gauge | — | Currently running tasks |
| `oxios_scheduler_rate_limit_remaining` | Gauge | — | Remaining rate limit capacity |
| `oxios_zombie_tasks_reaped_total` | Counter | — | Zombie tasks reaped |

### 3.8 Event Bus

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `oxios_event_bus_published_total` | Counter | `event_type` | Events published |
| `oxios_event_bus_received_total` | Counter | `event_type` | Events received by subscribers |
| `oxios_event_bus_channel_overflow_total` | Counter | — | Events dropped due to channel overflow |

### 3.9 Histogram Buckets

Default buckets (seconds):  
`[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]`

Specific buckets per category:
- **LLM calls:** `[0.5, 1, 2, 5, 10, 30, 60, 120]` (LLM latency can be high)
- **Tool calls:** `[0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10]`
- **Phase durations:** `[0.1, 0.25, 0.5, 1, 2, 5, 10, 30, 60, 120, 300]`

---

## 4. Structured Logging Format

Every log entry follows this JSON schema:

```json
{
  "ts": "2026-05-07T14:32:01.234Z",
  "level": "INFO",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "span_id": "00f067aa0ba902b7",
  "parent_span_id": "4bf92f3577b34da6",
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "session_id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
  "seed_id": "a4c3e2d1-b6f8-4e9a-8c5d-7f2b1e3a4c6d",
  "phase": "Execute",
  "msg": "Tool call completed",
  "tool": "container_exec",
  "duration_ms": 234,
  "exit_code": 0,
  "error": null
}
```

**Field definitions:**

| Field | Type | Description |
|-------|------|-------------|
| `ts` | ISO8601 string | Timestamp in UTC |
| `level` | string | `DEBUG`, `INFO`, `WARN`, `ERROR` |
| `trace_id` | hex string (32 chars) | Root trace for this orchestration |
| `span_id` | hex string (16 chars) | Current span within the trace |
| `parent_span_id` | hex string (16 chars) | Parent span (if any) |
| `agent_id` | UUID | Agent instance (null if not yet forked) |
| `session_id` | UUID | User session (null if no session) |
| `seed_id` | UUID | Current seed being executed |
| `phase` | string | Current Ouroboros phase |
| `msg` | string | Human-readable message |
| `tool` | string | Tool name (for tool-related log entries) |
| `duration_ms` | number | Duration in milliseconds (for operations with timing) |
| `exit_code` | number | Process exit code (null if not applicable) |
| `error` | string | Error message (null if no error) |
| `extra` | object | Additional context (optional) |

**Log level guidelines:**

- `ERROR`: Unrecoverable failures — agent crashed, LLM API error, container panic
- `WARN`: Degraded but recoverable — rate limit hit, retry succeeded, zombie task detected
- `INFO`: Lifecycle events — agent forked, phase started/completed, session started/ended
- `DEBUG`: Instrumented detail — entering/exiting functions, loop iteration, queue depth

**Sampling strategy:**
- 100% of errors and warnings (always useful for debugging)
- 100% of lifecycle events (low volume, always useful)
- 1% of successful tool calls in steady state (full detail on demand via trace_id)
- 10% of successful tool calls during anomaly detection (e.g., if error rate spikes, increase sampling)

---

## 5. Tracing Approach

### 5.1 Trace Hierarchy

```
trace_id: orchestration-root
│
├── span: interview
│   └── span: llm_call (interview)
│
├── span: seed_generation
│   └── span: llm_call (seed_gen)
│
├── span: execute (agent loop)
│   ├── span: llm_call (agent)
│   │   └── span: tool_call (read)
│   │   └── span: tool_call (container_exec)
│   │   └── span: tool_call (mcp_<server>_<tool>)
│   └── span: llm_call (agent)
│       └── span: tool_call (...)
│
├── span: evaluate
│   └── span: llm_call (evaluate)
│
└── span: evolve (optional, repeated)
    └── span: llm_call (evolve)
```

Each span records:
- Span name (e.g., `execute`, `tool_call`)
- Start/end time
- Attributes (tool name, phase, seed_id, agent_id, etc.)
- Status (OK or Error with message)

### 5.2 Trace Context Propagation

- `handle_message` creates the root span with a new `trace_id`
- The `trace_id` is passed through all async calls (carried in `OrchestrationContext`)
- Sub-spans are created for each phase within the orchestration
- Tool execution spans are children of their calling LLM span
- Event bus events include `trace_id` and `span_id` in their metadata

### 5.3 Implementation

Using `tracing` + `tracing-opentelemetry`:

```rust
// Root span for orchestration
let span = tracing::info_span!(
    "orchestration",
    session_id = %session_id,
    user_id = %user_id
);
let guard = span.enter();

// Phase span (child of orchestration root)
let phase_span = tracing::info_span!("phase", phase = ?phase);
let _guard = phase_span.enter();

// LLM call span
let llm_span = tracing::info_span!(
    "llm_call",
    model = %config.model_id,
    phase = ?phase
);
```

---

## 6. API Endpoints

### 6.1 Prometheus Metrics

```
GET /api/metrics
```

Returns Prometheus text format metrics. No authentication (internal use only, or protected by reverse proxy).

**Response:**
```
# HELP oxios_agents_running Current number of running agents
# TYPE oxios_agents_running gauge
oxios_agents_running 3

# HELP oxios_llm_duration_seconds LLM call duration in seconds
# TYPE oxios_llm_duration_seconds histogram
oxios_llm_duration_seconds_bucket{model="claude-sonnet-4",le="0.5"} 42
oxios_llm_duration_seconds_bucket{model="claude-sonnet-4",le="1"} 89
...
oxios_llm_duration_seconds_sum{model="claude-sonnet-4"} 234.5
oxios_llm_duration_seconds_count{model="claude-sonnet-4"} 150
```

**Implementation:** Uses `metrics-exporter-prometheus` with a `HttpMetricsEndpoint`.

### 6.2 Health Check

```
GET /api/health
```

Returns system health status. Used by load balancers and orchestration systems (Kubernetes liveness/readiness probes).

**Response:**
```json
{
  "status": "ok",
  "checks": {
    "event_bus": "ok",
    "container_manager": "ok",
    "scheduler": "ok",
    "state_store": "ok",
    "rate_limit": "ok"
  },
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Status values:**
- `"ok"` — all checks passed
- `"degraded"` — some checks failed but system is operational
- `"unhealthy"` — critical checks failed

### 6.3 Status Dashboard

```
GET /api/status
```

Returns a comprehensive JSON dashboard of system state. Useful for debugging and monitoring dashboards.

**Response:**
```json
{
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "system": {
    "memory_used_mb": 128,
    "memory_allocated_mb": 256,
    "cpu_usage_percent": 12.5
  },
  "agents": {
    "running": 3,
    "completed_total": 47,
    "failed_total": 5
  },
  "sessions": {
    "active": 8
  },
  "scheduler": {
    "queued": 2,
    "running": 3,
    "rate_limit_remaining": 45
  },
  "memory": {
    "conversation_entries": 150,
    "fact_entries": 42,
    "episode_entries": 12
  },
  "containers": {
    "active": 3,
    "healthy": 3
  }
}
```

### 6.4 Traces (Optional OTLP Export)

If OpenTelemetry export is configured:
```
OTLP endpoint: https://collector.example.com:4317
Protocol: grpc
```

The `tracing-opentelemetry` layer exports spans to the configured OTLP collector.

---

## 7. Implementation Plan

### 7.1 Crates

#### `crates/oxios-kernel/src/metrics.rs` — Core Metrics Definitions

Central place for all metric definitions. Uses the `metrics` crate (https://metrics.github.io/).

**Metric definitions:**

```rust
use metrics::{describe_counter, describe_gauge, describe_histogram, Counter, Gauge, Histogram};

// Agents
pub static AGENTS_FORKED: Counter = metrics::counter!("oxios_agents_forked_total");
pub static AGENTS_RUNNING: Gauge = metrics::gauge!("oxios_agents_running");
pub static AGENTS_COMPLETED: Counter = metrics::counter!("oxios_agents_completed_total");
pub static AGENTS_FAILED: Counter = metrics::counter!("oxios_agents_failed_total");

// Orchestration
pub static MESSAGES_PROCESSED: Counter = metrics::counter!("oxios_messages_processed_total");
pub static ORCHESTRATION_DURATION: Histogram = metrics::histogram!("oxios_orchestration_duration_seconds");
pub static PHASE_DURATION: Histogram = metrics::histogram!("oxios_phase_duration_seconds");

// LLM
pub static LLM_CALLS: Counter = metrics::counter!("oxios_llm_calls_total");
pub static LLM_DURATION: Histogram = metrics::histogram!("oxios_llm_duration_seconds");
pub static LLM_ERRORS: Counter = metrics::counter!("oxios_llm_errors_total");

// Tools
pub static TOOL_CALLS: Counter = metrics::counter!("oxios_tool_calls_total");
pub static TOOL_DURATION: Histogram = metrics::histogram!("oxios_tool_duration_seconds");
pub static TOOL_ERRORS: Counter = metrics::counter!("oxios_tool_errors_total");

// Memory
pub static MEMORY_ENTRIES: Gauge = metrics::gauge!("oxios_memory_entries_total");
pub static MEMORY_RECALL: Counter = metrics::counter!("oxios_memory_recall_total");
pub static MEMORY_STORE: Counter = metrics::counter!("oxios_memory_store_total");

// Container
pub static CONTAINER_EXEC: Counter = metrics::counter!("oxios_container_exec_total");
pub static CONTAINER_EXEC_DURATION: Histogram = metrics::histogram!("oxios_container_exec_duration_seconds");

// Sessions
pub static ACTIVE_SESSIONS: Gauge = metrics::gauge!("oxios_active_sessions");
pub static SCHEDULER_QUEUED: Gauge = metrics::gauge!("oxios_scheduler_queued_tasks");
pub static SCHEDULER_RUNNING: Gauge = metrics::gauge!("oxios_scheduler_running_tasks");

// Events
pub static EVENT_BUS_PUBLISHED: Counter = metrics::counter!("oxios_event_bus_published_total");
pub static EVENT_BUS_DROPPED: Counter = metrics::counter!("oxios_event_bus_channel_overflow_total");
```

**Helper functions for recording metrics:**

```rust
/// Record a phase duration.
pub fn record_phase_duration(phase: &str, duration_secs: f64) {
    PHASE_DURATION.record(duration_secs, &[("phase", phase)]);
}

/// Record a tool call with duration and error status.
pub fn record_tool_call(tool: &str, duration_secs: f64, error: Option<&str>) {
    TOOL_CALLS.increment(&[("tool", tool)]);
    TOOL_DURATION.record(duration_secs, &[("tool", tool)]);
    if let Some(err) = error {
        TOOL_ERRORS.increment(&[("tool", tool), ("error_type", err)]);
    }
}
```

#### `crates/oxios-kernel/src/tracing.rs` — Tracing Setup

Configures the OpenTelemetry tracing pipeline.

**Key functions:**
- `init_tracing()` — initializes the tracing subscriber with OTLP export (if configured)
- `shutdown_tracing()` — flushes and shuts down the OTLP exporter
- `create_span(name, attrs)` — creates a new span within the current trace
- `wrap_future(span, future)` — wraps an async future to propagate trace context

**Configuration via environment:**
```rust
// OXIOS_OTEL_ENDPOINT=https://collector:4317
// OXIOS_TRACING_SAMPLE_RATE=0.1  // export 10% of traces
```

#### `crates/oxios-kernel/src/metrics.rs` — Metrics Exporter

```rust
use metrics_exporter_prometheus::PrometheusBuilder;

// Initialize the Prometheus exporter and bind to an HTTP endpoint.
pub fn init_metrics_exporter(listen_addr: &str) {
    PrometheusBuilder::new()
        .with_http_listener(listen_addr.parse().unwrap())
        .install()
        .expect("failed to install Prometheus exporter");
}
```

### 7.2 Web Routes

#### `channels/oxios-web/src/routes/infra.rs` — Metrics Endpoints

Extends the existing web routes with `/api/metrics`, `/api/health`, and `/api/status`.

```rust
use axum::{
    extract::State,
    response::Response,
    routing::get,
    Router,
};
use oxios_kernel::metrics::{MetricsRegistry, GlobalMetrics};
use std::sync::Arc;

pub fn infra_routes() -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
}

async fn metrics_handler() -> Response {
    let body = GlobalMetrics::export_prometheus();
    Response::builder()
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(body)
        .unwrap()
}

async fn health_handler(State(registry): State<Arc<MetricsRegistry>>) -> Response {
    let checks = registry.health_checks().await;
    let all_ok = checks.values().all(|s| s == "ok");
    
    let status = if all_ok { "ok" } else { "degraded" };
    let body = serde_json::to_string(&serde_json::json!({
        "status": status,
        "checks": checks,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": registry.uptime_seconds(),
    })).unwrap();
    
    let status_code = if all_ok { 200 } else { 503 };
    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json")
        .body(body)
        .unwrap()
}

async fn status_handler(State(registry): State<Arc<MetricsRegistry>>) -> Response {
    let dashboard = registry.dashboard().await;
    Response::builder()
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&dashboard).unwrap())
        .unwrap()
}
```

### 7.3 Instrumentation Points

#### `event_bus.rs` — Event Bus Instrumentation

In `EventBus::publish()`:

```rust
pub fn publish(&self, event: KernelEvent) -> Result<()> {
    let event_type = format!("{:?}", event);
    
    // Record metric
    EVENT_BUS_PUBLISHED.increment(&[("event_type", &event_type)]);
    
    let result = self.sender.send(event.clone());
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            EVENT_BUS_DROPPED.increment(&[("event_type", &event_type)]);
            // Channel full — this is non-fatal, events are broadcast-only
            Ok(())
        }
    }
}
```

#### `supervisor.rs` — Agent Lifecycle Instrumentation

In `fork()`:
```rust
async fn fork(&self, spec: &Seed) -> Result<AgentId> {
    let start = std::time::Instant::now();
    let priority_label = format!("{:?}", spec.priority.unwrap_or(Priority::Normal));
    
    let id = /* existing fork logic */;
    
    AGENTS_FORKED.increment(&[("priority", &priority_label)]);
    AGENTS_RUNNING.increment();
    
    tracing::info!(agent_id = %id, priority = %priority_label, "Agent forked");
    Ok(id)
}
```

In `run_with_seed()`:
```rust
async fn run_with_seed(&self, id: AgentId, seed: &Seed) -> Result<ExecutionResult> {
    let start = std::time::Instant::now();
    
    let result = /* existing logic */;
    
    let duration = start.elapsed().as_secs_f64();
    
    if result.success {
        AGENTS_COMPLETED.increment(&[("success", "true")]);
    } else {
        AGENTS_FAILED.increment(&[("reason", "execution_failed")]);
    }
    AGENTS_RUNNING.decrement();
    
    // Record duration histogram
    // TODO: add agent_duration_seconds histogram
    tracing::info!(
        agent_id = %id,
        success = result.success,
        steps = result.steps_completed,
        duration_secs = duration,
        "Agent completed"
    );
    
    Ok(result)
}
```

#### `orchestrator.rs` — Orchestration Instrumentation

In `handle_message()`:
```rust
pub async fn handle_message(...) -> Result<OrchestrationResult> {
    let start = Instant::now();
    
    // Create root span for this orchestration
    let span = tracing::info_span!(
        "orchestration",
        session_id = %session_id,
        user_id = %user_id
    );
    let _guard = span.enter();
    
    // ... existing logic ...
    
    // Record orchestration duration
    let duration = start.elapsed().as_secs_f64();
    ORCHESTRATION_DURATION.record(duration);
    PHASE_DURATION.record(duration, &[("phase", "orchestration")]);
    
    Ok(result)
}
```

In phase methods:
```rust
async fn publish_phase_started(&self, session_id: &str, phase: Phase) {
    let span = tracing::info_span!("phase", session_id = %session_id, phase = ?phase);
    let _guard = span.enter();
    
    // Update gauge
    ACTIVE_PHASES.increment(&[("phase", phase.as_str())]);
    
    // ... existing publish logic ...
}
```

#### `agent_runtime.rs` — Execution Instrumentation

In `run_agent_loop()`:
```rust
// Around LLM calls
let llm_start = Instant::now();
let result = /* LLM call */;
let llm_duration = llm_start.elapsed().as_secs_f64();

LLM_CALLS.increment(&[("phase", current_phase.as_str())]);
LLM_DURATION.record(llm_duration, &[("model", &config.model_id)]);

// Around tool calls
let tool_start = Instant::now();
let result = tool.execute(...);
let tool_duration = tool_start.elapsed().as_secs_f64();

TOOL_CALLS.increment(&[("tool", tool_name)]);
TOOL_DURATION.record(tool_duration, &[("tool", tool_name)]);

if let Err(e) = result {
    TOOL_ERRORS.increment(&[("tool", tool_name), ("error_type", e.kind())]);
    tracing::warn!(tool = %tool_name, error = %e, "Tool call failed");
}
```

For container execution specifically:
```rust
CONTAINER_EXEC.increment(&[("command_type", command_type)]);
CONTAINER_EXEC_DURATION.record(duration, &[("command_type", command_type)]);
```

#### `memory.rs` — Memory Instrumentation

In `remember()`:
```rust
pub async fn remember(&self, entry: MemoryEntry) -> Result<String> {
    let id = entry.id.clone();
    let category = entry.memory_type.category();
    
    self.state_store.save_json(category, &id, &entry).await?;
    
    MEMORY_STORE.increment(&[("type", entry.memory_type.label())]);
    // Update gauge
    self.update_memory_entry_gauge().await;
    
    tracing::debug!(id = %id, ty = entry.memory_type.label(), "Memory stored");
    Ok(id)
}
```

In `recall()`:
```rust
pub async fn recall(&self, query: &str) -> Result<Vec<MemoryEntry>> {
    let start = Instant::now();
    
    let result = self._recall_impl(query).await;
    
    let duration = start.elapsed().as_secs_f64();
    MEMORY_RECALL.increment();
    MEMORY_RECALL_LATENCY.record(duration);
    
    result
}
```

### 7.4 Structured Logging

Use `tracing` with a JSON formatter (custom or via `tracing-sermon`):

**JSON formatter output:**
```json
{"timestamp":"2026-05-07T14:32:01.234Z","level":"INFO","target":"oxios_kernel::orchestrator","message":"Phase started","session_id":"7c9e6679-7425-40de-944b-e07fc1f90ae7","phase":"Execute","span_id":"00f067aa0ba902b7"}
```

**Configuration:**
```rust
use tracing_subscriber::{fmt, EnvFilter};
use tracing_sermon::JsonFormat;

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .with_target(false)
    .with_thread_ids(false)
    .with_file(false)
    .with_line_number(false)
    .json()
    .init();
```

**Log sampling via tracing:**
```rust
// Only log 1% of successful tool calls at DEBUG level
if should_sample_tool_call() {
    tracing::debug!(
        tool = %tool_name,
        duration_ms = duration.as_millis(),
        "Tool call completed"
    );
}

// Always log errors
if let Err(e) = result {
    tracing::error!(
        tool = %tool_name,
        error = %e,
        "Tool call failed"
    );
}
```

---

## 8. File Structure

```
crates/oxios-kernel/
├── src/
│   ├── metrics.rs          # NEW: Metric definitions, counters/gauges/histograms, helper functions
│   ├── tracing.rs          # NEW: OpenTelemetry tracing setup, span management
│   ├── event_bus.rs        # MODIFIED: Instrument event publish/subscribe
│   ├── supervisor.rs       # MODIFIED: Instrument fork/exec/complete
│   ├── orchestrator.rs     # MODIFIED: Instrument phase transitions, orchestration duration
│   ├── agent_runtime.rs    # MODIFIED: Instrument LLM calls, tool calls, container exec
│   ├── memory.rs           # MODIFIED: Instrument memory store/recall operations
│   ├── scheduler.rs        # MODIFIED: Instrument queue depth, rate limiting
│   ├── kernel.rs           # MODIFIED: Initialize metrics + tracing on startup
│   └── lib.rs
│
channels/oxios-web/
└── src/
    └── routes/
        └── infra.rs        # NEW or EXTENDED: /metrics, /health, /status endpoints

docs/
└── designs/
    └── loop7-metrics-observability.md  # This document
```

---

## 9. Performance Considerations

### 9.1 Lock-Free Design

The `metrics` crate is designed for lock-free operation in the hot path:
- Counters/gauge increments use atomic operations, not mutex locks
- Histogram recording is also atomic (batch-friendly)
- No locks in the instrumentation points — only at the exporter level

### 9.2 Histogram Bucket Design

Histogram buckets are carefully chosen to:
- Capture the distribution at relevant percentiles (p50 at ~0.1s, p99 at ~10s for tools)
- Avoid too many buckets (overhead) or too few (poor resolution)
- Differ by category (LLM calls need larger buckets due to higher variance)

### 9.3 Log Sampling

High-volume logging (tool calls) is sampled:
- **Steady state:** 1% sample rate for successful tool calls
- **Anomaly mode:** When error rate exceeds threshold, increase to 100% temporarily
- **Always:** 100% of errors, warnings, and lifecycle events

**Implementation:**
```rust
use std::sync::atomic::{AtomicU64, Ordering};

static TOOL_CALL_COUNTER: AtomicU64 = AtomicU64::new(0);

fn should_sample_tool_call() -> bool {
    // Sample 1 in 100 (1% rate)
    TOOL_CALL_COUNTER.fetch_add(1, Ordering::Relaxed) % 100 == 0
}
```

### 9.4 Tracing Overhead

When no OTLP collector is configured, `tracing-opentelemetry` is a no-op (spans are created but not exported). When a collector is configured, spans are exported asynchronously (non-blocking).

**CPU overhead:**
- With tracing disabled: < 1% overhead
- With tracing at 100% export: ~2-5% overhead (span creation and serialization)
- With tracing at 10% sample rate: ~0.5% overhead

### 9.5 Metrics Export Interval

Prometheus scrape interval is typically 15s. Histogram data is stored in-memory until scraped. For high-cardinality metrics (e.g., per-tool breakdown), consider:
- Use labels sparingly — group similar tools (e.g., `tool: "tier1"`, `tool: "tier2"`)
- Use `recency` to drop stale metrics

---

## 10. Dependencies

```toml
# crates/oxios-kernel/Cargo.toml

[dependencies]
# Metrics
metrics = "0.22"
metrics-exporter-prometheus = "0.13"

# Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.24"
opentelemetry = { version = "0.24", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.13", features = ["grpc"] }

# Utilities
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
```

---

## 11. Migration Path

**Phase 1 — Core Metrics (Week 1)**
- Add `metrics.rs` with all metric definitions
- Add metric increments in `supervisor.rs` and `orchestrator.rs`
- Add `/api/metrics` endpoint in web routes
- Verify metrics appear in Prometheus format

**Phase 2 — Structured Logs (Week 2)**
- Configure `tracing-subscriber` with JSON formatter
- Replace existing `tracing::info!` calls with structured versions (add fields)
- Add log sampling for high-volume tool calls

**Phase 3 — Tracing (Week 3)**
- Add `tracing.rs` with OTLP initialization
- Instrument `orchestrator.rs` with phase spans
- Add trace context to `KernelEvent` (optional, if useful)

**Phase 4 — Health & Status (Week 4)**
- Add `/api/health` with check functions
- Add `/api/status` with dashboard JSON
- Integrate into existing web routes

**Phase 5 — Production Hardening (Week 5)**
- Tune histogram buckets based on real data
- Adjust sampling rates based on load testing
- Add cardinality guards for dynamic labels
- Document troubleshooting procedures

---

## 12. Troubleshooting Guide

### "Metrics endpoint shows no data"
1. Check that `metrics-exporter-prometheus` is initialized on startup
2. Verify the HTTP endpoint is accessible (`curl localhost:9090/metrics`)
3. Check for initialization panics in logs

### "Traces not appearing in collector"
1. Verify OTLP endpoint is reachable (`curl -v https://collector:4317`)
2. Check that `tracing-opentelemetry` was initialized with the correct endpoint
3. Verify sampling rate is > 0 (env var `OXIOS_TRACING_SAMPLE_RATE`)
4. Check for exporter flush errors in logs

### "High cardinality labels causing memory issues"
1. Audit labels — avoid unbounded string labels (e.g., full file paths)
2. Use bucketing for labels with high cardinality (e.g., `session_id` → `session_bucket: 0-100`)
3. Set `metrics::describe_gauge` with `Cardinality::Low` expectation

### "Logs volume too high"
1. Increase sampling rate for tool calls
2. Filter DEBUG logs at the subscriber level (`RUST_LOG=info`)
3. Enable log aggregation with retention policies (e.g., only keep errors for 30 days)

---

## 13. Future Considerations

### Correlation with APM
- Integrate with Datadog, New Relic, or Grafana Cloud via OTLP
- Add span attributes that link to APM transaction IDs

### Custom Dashboards
- Provide Grafana dashboard JSON (importable)
- Pre-built panels for: Ouroboros loop duration, phase breakdown, tool call latency, error rate

### Alerting
- Alert rules for: error rate > 5%, orchestration duration p99 > 30s, active agents > capacity
- Integrate with PagerDuty/Slack via webhook

### Distributed Tracing (Future)
- If Oxios adds multi-host deployment, ensure trace context propagates across hosts
- Consider using W3C TraceContext standard for cross-service traces

---

*End of Loop 7 Design*