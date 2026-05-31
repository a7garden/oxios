# Observability Audit

**Date:** 2026-05-31  
**Scope:** Tracing, metrics, OpenTelemetry, operational visibility  
**Severity:** 🟢 Normal — everything works, but observability is mostly dark by default

---

## 1. Executive Summary

Oxios has a **solid structural foundation** for observability but most of it is inactive by default. The good news: the plumbing is already in place — metrics registry, tracing subscriber, oxi-sdk Tracer/CostTracker/AuditLog. The gap: nothing is wired together end-to-end, and the OTel feature compiles but exports nothing.

| Layer | Status | Assessment |
|-------|--------|------------|
| Structured logging (`tracing`) | ✅ Working | 440 calls across 9 crates, 3 formats (pretty/json/compact) |
| Metrics registry | ✅ Implemented | `MetricsRegistry` with counters/gauges/histograms, Prometheus text export |
| `/api/metrics` endpoint | ✅ Exists | Returns Prometheus text format — but metrics are zeroed (never registered) |
| `/api/status` endpoint | ✅ Working | Component health, uptime, agent counts |
| `/health` & `/health/ready` | ✅ Working | Basic liveness + readiness with state store + git checks |
| OTel feature gate | ⚠️ Compiles, no-op | `telemetry_otel.rs` returns empty layers — no real OTLP pipeline |
| `src/otel.rs` | ⚠️ Stub | Warns if enabled but exports nothing |
| oxi-sdk Tracer spans | ⚠️ Minimal | Only 1 call site (`agent_runtime.rs:411`) uses `tracer().start()` |
| Correlation IDs | ❌ Missing | No request ID flows through gateway → orchestrator → tools |
| `register_builtin_metrics()` | ❌ Never called | Exported but no caller — all `MetricsHandles` remain at zero |
| Alerting | ❌ Not in scope | Daily health check runs internally, not exposed |

---

## 2. Tracing Infrastructure

### 2.1 Structured Logging (`tracing` crate)

**440 `tracing::*!` macro calls** across the codebase:

| Crate | Calls | Key Modules |
|-------|-------|-------------|
| `oxios-kernel` | 244 | orchestrator (21), scheduler, memory/*, agent_lifecycle, supervisor, tools |
| `oxios-web` (surface) | 76 | routes/*, plugin, middleware, channel |
| `oxios` (binary) | 58 | main, kernel, cmd_run |
| `oxios-ouroboros` | 16 | ouroboros_engine |
| `oxios-gateway` | 15 | gateway |
| `oxios-mcp` | 13 | client, lib |
| `oxios-telegram` | 8 | lib, plugin |
| `oxios-bench` | 5 | suite, runner |
| `oxios-cli` | 3 | channel, interactive |
| `oxios-markdown` | 2 | knowledge, sync |

**Log level distribution (approximate):**
- `info!` — ~280 calls (dominant)
- `warn!` — ~80 calls
- `debug!` — ~55 calls
- `error!` — ~25 calls

### 2.2 Log Subscriber Setup (`src/main.rs:1237–1268`)

The binary initializes `tracing_subscriber::fmt()` with:

- **EnvFilter** — `RUST_LOG` env var → config `[logging].level` → `info` fallback
- **3 formats** — `pretty` (default), `json`, `compact` — via `[logging].format`
- **File output** — Rolling daily appender to `[daemon].log_dir/oxios.log`
- **Non-blocking writer** — `tracing_appender::non_blocking` to avoid blocking tokio

**Assessment:** Well-structured. JSON mode is production-ready for ELK/Loki/CloudWatch ingestion.

### 2.3 oxi-sdk Tracer (`crates/oxios-kernel/src/observability.rs`)

Provides global `OnceLock` instances:

| Instance | Purpose | Usage |
|----------|---------|-------|
| `tracer()` | Distributed spans (`AgentSpan`, `ToolSpan`) | **1 call site** — `agent_runtime.rs:411` |
| `cost_tracker()` | Per-agent token/cost accounting | **1 call site** — `agent_runtime.rs:603` |
| `audit_log()` | Structured security audit entries | **0 call sites** (only tests) |

**Gap:** The `tracer()` and `audit_log()` are barely used. The infrastructure exists but needs adoption across orchestrator, scheduler, tools, and gateway.

---

## 3. Metrics Infrastructure

### 3.1 Metrics Registry (`crates/oxios-kernel/src/metrics.rs`)

A full Prometheus-compatible metrics registry:

- **`MetricsRegistry`** — thread-safe registry with counters, gauges, histograms
- **`registry()`** — global `OnceLock` instance
- **`export()`** — Prometheus text format output
- **`register_builtin_metrics()`** — defines 22 metrics across 7 categories

**Defined metrics:**

| Category | Metrics |
|----------|---------|
| Agents | `oxios_agents_forked_total`, `oxios_agents_running` (gauge), `oxios_agents_completed_total`, `oxios_agents_failed_total` |
| Messages | `oxios_messages_processed_total`, `oxios_orchestration_duration_seconds` (histogram) |
| LLM | `oxios_llm_calls_total`, `oxios_llm_duration_seconds` (histogram), `oxios_llm_errors_total`, `oxios_llm_circuit_breaker_state` (gauge) |
| Tools | `oxios_tool_calls_total`, `oxios_tool_duration_seconds` (histogram), `oxios_tool_errors_total` |
| Memory | `oxios_memory_entries_total` (gauge), `oxios_memory_recall_total` |
| Exec | `oxios_exec_total`, `oxios_exec_duration_seconds` (histogram) |
| Sessions | `oxios_active_sessions` (gauge) |

### 3.2 `MetricsHandles` Struct

A convenience struct with `inc_*()` / `observe_*()` methods:

| Handle | Type | Call Sites |
|--------|------|-----------|
| `agents_forked` | Counter | `agent_lifecycle.rs:76` |
| `agents_completed` | Counter | (defined, not called) |
| `agents_failed` | Counter | (defined, not called) |
| `messages` | Counter | `orchestrator.rs:264` |
| `orch_duration` | Histogram | `orchestrator.rs:677` |
| `llm_circuit_breaker_state` | Gauge | `agent_runtime.rs:664,667` |

### 3.3 Critical Bug: `register_builtin_metrics()` Never Called

The function exists in `metrics.rs` and is re-exported via `lib.rs`, but **no code calls it**. This means:

- `registry().export()` returns an empty string
- `/api/metrics` returns nothing useful
- `get_metrics()` handles are initialized on first use via `OnceLock`, but `register_builtin_metrics()` also creates its own registrations — these are separate from `get_metrics()`, creating a dual-registration confusion

**Fix:** Call `register_builtin_metrics()` during kernel startup (e.g., in `Kernel::builder().build()` or `observability::init()`).

### 3.4 `/api/metrics` Endpoint

Two routes point to the same handler:

```
GET /metrics         → handle_metrics()   (public, no auth)
GET /api/metrics     → handle_metrics()   (protected, auth middleware)
```

Both return `registry().export()` — Prometheus text format. Currently returns empty/zeroed metrics.

### 3.5 `/api/status` Endpoint

Returns JSON with real data:

```json
{
  "service": "oxios",
  "status": "healthy",
  "version": "0.6.0",
  "channels": ["web"],
  "uptime": "1h 23m 45s",
  "components": {
    "state_store": { "healthy": true },
    "event_bus": { "healthy": true },
    "memory": { "healthy": true, "index_size": 12, "total_entries": 34 },
    "agents": { "active_count": 2, "total_forked": 47, "total_completed": 45, "total_failed": 0 }
  }
}
```

---

## 4. OpenTelemetry Integration

### 4.1 Feature Gate Architecture

```
crates/oxios-kernel/Cargo.toml:
  otel = ["tracing-opentelemetry", "opentelemetry", "opentelemetry_sdk",
          "opentelemetry-otlp", "opentelemetry-stdout"]
```

Dependencies (optional, OTel 0.27 / tracing-opentelemetry 0.28):

| Crate | Version | Purpose |
|-------|---------|---------|
| `tracing-opentelemetry` | 0.28 | Bridge between `tracing` and OTel |
| `opentelemetry` | 0.27 | Core API |
| `opentelemetry_sdk` | 0.27 | SDK with `rt-tokio` feature |
| `opentelemetry-otlp` | 0.27 | OTLP exporter (gRPC) |
| `opentelemetry-stdout` | 0.27 | Stdout exporter (debugging) |

### 4.2 Implementation Status

**`telemetry_otel.rs`** (feature=otel):
- Defines `TelemetryConfig` struct
- `init_telemetry_layers()` → returns `Ok(vec![])` — **empty, no actual OTLP pipeline built**
- Comment says: "foundation for future OTel pipeline setup"

**`telemetry_stub.rs`** (default, no otel):
- Same `TelemetryConfig` struct
- `init_telemetry_layers()` → `Ok(vec![])` — no-op as expected

**`src/otel.rs`** (binary):
- `init_otel(config)` → warns if enabled, returns no-op guard
- Guard's `Drop` is no-op

### 4.3 What's Missing for Real OTel

1. **No OTLP tracer provider construction** — need to create `opentelemetry_sdk::trace::TracerProvider` with OTLP exporter
2. **No subscriber layer wiring** — `init_telemetry_layers()` returns empty vec instead of `OpenTelemetryLayer`
3. **No config propagation** — `TelemetryConfig` has `endpoint` field but it's never read
4. **No shutdown** — `OtelGuard::drop()` should flush and shutdown the tracer provider
5. **Feature not propagated** — root `Cargo.toml` has no `otel` feature; `cargo build --features otel` fails on the binary

### 4.4 Compilation Status

```bash
$ cargo check -p oxios-kernel --features otel
# ✅ Compiles with 14 warnings (missing docs)
$ cargo check --features otel
# ❌ Error: package 'oxios' does not contain feature 'otel'
```

---

## 5. Audit Trail

`crates/oxios-kernel/src/audit_trail.rs` — Merkle-chain style tamper-evident log:

- Each entry includes `prev_hash` + computed `hash` (blake3)
- Chain integrity verification with `verify_chain()`
- Separate from the oxi-sdk `AuditLog` — this is kernel-level tamper evidence

**Status:** Working as designed. Not in scope for this audit.

---

## 6. Resource Monitoring

`crates/oxios-kernel/src/resource_monitor.rs`:

- `ResourceSnapshot` — CPU%, memory MB, active agents, pending tasks, tokens, disk, load avg
- Background sampling via `start_sampling()` 
- History with configurable retention

**Gap:** Not connected to `/api/metrics` or `/api/status`. Data exists but isn't exposed.

---

## 7. Key Findings

### ✅ What Works

1. **Structured logging** — 440 calls, 3 formats, env filter, file appender. Production-ready.
2. **Metrics registry** — Full Prometheus-compatible implementation with counters/gauges/histograms.
3. **Health endpoints** — `/health`, `/health/ready`, `/api/status` with real component checks.
4. **Cost tracking** — `CostTracker` records per-agent token usage (1 active call site).
5. **Circuit breaker state** — Gauge updated on open/close transitions.
6. **OTel feature gate** — Dependencies compile, feature toggles work at crate level.

### ⚠️ What Needs Fixing

1. **`register_builtin_metrics()` never called** — all Prometheus counters are zeroed.
2. **OTel feature not in root `Cargo.toml`** — can't enable from binary level.
3. **OTel layers return empty** — `init_telemetry_layers()` is a no-op even with feature enabled.
4. **Tracer barely used** — only 1 `tracer().start()` call across entire codebase.

### ❌ What's Missing

1. **Correlation/request IDs** — no ID flows from channel → orchestrator → agent → tools.
2. **Metric instrumentation** — tool calls, LLM calls, exec calls, memory operations not counted.
3. **Resource monitor data** — not exposed in any API endpoint.
4. **Phase duration histogram** — defined in `register_builtin_metrics()` but never observed.

---

## 8. Recommendations (Priority Order)

| # | Action | Effort | Impact |
|---|--------|--------|--------|
| 1 | Call `register_builtin_metrics()` at startup | 1 line | Metrics endpoint works |
| 2 | Wire `MetricsHandles` into agent_lifecycle, scheduler, tools | ~20 lines | Real counters |
| 3 | Add `otel` feature to root `Cargo.toml` | 1 line | Binary can enable OTel |
| 4 | Implement real OTLP pipeline in `telemetry_otel.rs` | ~50 lines | Real distributed traces |
| 5 | Add correlation IDs to gateway → orchestrator flow | ~30 lines | Request tracing |
| 6 | Adopt `tracer().start()` across orchestrator phases | ~10 sites | Span coverage |
| 7 | Expose `ResourceMonitor` data in `/api/status` | ~10 lines | Live resource visibility |
