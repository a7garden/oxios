# Metrics Endpoint Design

**Date:** 2026-05-31  
**Status:** Design (implementation ready)  
**Scope:** `GET /api/metrics` — JSON operational metrics for dashboards and monitoring

---

## 1. Current State

Oxios already has:

| Component | Status |
|-----------|--------|
| `MetricsRegistry` (`metrics.rs`) | ✅ Full Prometheus-compatible registry with counters/gauges/histograms |
| `MetricsHandles` | ✅ Convenience struct with `inc_*()`/`observe_*()` methods |
| `GET /api/metrics` | ✅ Returns Prometheus text format via `registry().export()` |
| `GET /metrics` | ✅ Public (no auth) alias |
| `register_builtin_metrics()` | ✅ Defines 22 metrics across 7 categories — **but never called** |
| `get_metrics()` usage | ⚠️ 4 call sites: `agent_lifecycle`, `orchestrator`, `agent_runtime` (circuit breaker) |

### Critical Fix Applied

`register_builtin_metrics()` was never called during startup. **Fixed:** Added to `Kernel::builder().build()` in `src/kernel.rs` alongside `observability::init()`.

---

## 2. Design: JSON Metrics Endpoint

The Prometheus text format works for Prometheus scraping, but a JSON endpoint is more convenient for:
- Web dashboard widgets
- Health check systems
- Custom monitoring scripts
- Grafana JSON data source

### 2.1 Endpoint Specification

```
GET /api/metrics/json
```

Response shape (already proposed in the brief):

```json
{
  "agents": {
    "active": 3,
    "total_spawned": 47,
    "completed": 45,
    "failed": 2
  },
  "scheduler": {
    "queue_depth": 2,
    "running": 3,
    "max_concurrent": 5,
    "completed": 45,
    "rate_remaining": 58
  },
  "llm": {
    "requests": 230,
    "errors": 3,
    "circuit_breaker": "closed"
  },
  "memory": {
    "index_size": 128,
    "total_entries": 89,
    "recall_operations": 34
  },
  "uptime_secs": 86400,
  "version": "0.6.0"
}
```

### 2.2 Data Sources

| Field | Source | Implementation |
|-------|--------|----------------|
| `agents.active` | `kernel.agents.list()` → filter by status | Already in `/api/status` |
| `agents.total_spawned` | `oxios_agents_forked_total` counter | Already instrumented |
| `agents.completed` | `oxios_agents_completed_total` counter | Needs wiring |
| `agents.failed` | `oxios_agents_failed_total` counter | Needs wiring |
| `scheduler.*` | `kernel.infra.scheduler_stats()` | Already exposed at `/api/scheduler/stats` |
| `llm.requests` | `oxios_llm_calls_total` counter | Needs wiring |
| `llm.errors` | `oxios_llm_errors_total` counter | Needs wiring |
| `llm.circuit_breaker` | `oxios_llm_circuit_breaker_state` gauge | Already updated |
| `memory.*` | `kernel.agents.memory_stats()` | Already in `/api/status` |
| `uptime_secs` | `state.start_time.elapsed()` | Already in `/api/status` |
| `version` | `env!("CARGO_PKG_VERSION")` | Already used |

### 2.3 Implementation Notes

No new dependencies needed. The data already exists across:

1. **`MetricsRegistry`** — Prometheus counters/gauges (parse from `export()` or read directly)
2. **`AppState`** — `kernel.agents`, `kernel.infra`, `start_time` are all accessible
3. **`ResourceMonitor`** — `snapshot()` returns `ResourceSnapshot` with CPU/memory/load

The handler should:

```rust
// surface/oxios-web/src/routes/infra.rs
pub(crate) async fn handle_metrics_json(
    state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let active_count = /* from kernel.agents.list() */;
    let scheduler = state.kernel.infra.scheduler_stats();
    let (mem_index, mem_total) = state.kernel.agents.memory_stats().await;
    let uptime = state.start_time.elapsed().as_secs();

    // Parse Prometheus export for counter values
    let export = registry().export();
    let forked = parse_counter(&export, "oxios_agents_forked_total");
    let completed = parse_counter(&export, "oxios_agents_completed_total");
    let failed = parse_counter(&export, "oxios_agents_failed_total");
    let cb_state = parse_gauge(&export, "oxios_llm_circuit_breaker_state");

    Json(json!({
        "agents": { "active": active_count, "total_spawned": forked, "completed": completed, "failed": failed },
        "scheduler": { "queue_depth": scheduler.queued, "running": scheduler.running, ... },
        "llm": { "circuit_breaker": match cb_state { 0.0 => "closed", 1.0 => "open", _ => "half_open" } },
        "memory": { "index_size": mem_index, "total_entries": mem_total },
        "uptime_secs": uptime,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
```

### 2.4 Route Registration

Add to `surface/oxios-web/src/routes/mod.rs`:

```rust
.route("/api/metrics/json", get(handle_metrics_json))
```

---

## 3. Wiring Plan (Existing Metrics Handles)

The `MetricsHandles` are partially wired. Full activation requires:

### 3.1 Already Wired

| Handle | Location |
|--------|----------|
| `agents_forked.inc()` | `agent_lifecycle.rs:76` |
| `messages.inc()` | `orchestrator.rs:264` |
| `orch_duration.observe()` | `orchestrator.rs:677` |
| `llm_circuit_breaker_state.set()` | `agent_runtime.rs:664,667` |

### 3.2 Needs Wiring

| Handle | Where to add | What to do |
|--------|-------------|-----------|
| `agents_completed.inc()` | `agent_lifecycle.rs` — in the `complete` path | `get_metrics().inc_agents_completed()` |
| `agents_failed.inc()` | `agent_lifecycle.rs` — in the `fail` path | `get_metrics().inc_agents_failed()` |
| `llm_calls_total` | `engine.rs` or `agent_runtime.rs` — at each LLM call | Add counter inc |
| `llm_errors_total` | `agent_runtime.rs` — at error handling | Add counter inc |
| `tool_calls_total` | `gated_tool.rs` — in execute wrapper | Add counter inc |
| `tool_errors_total` | `gated_tool.rs` — in error path | Add counter inc |
| `tool_duration_seconds` | `gated_tool.rs` — observe elapsed time | Add histogram observe |

### 3.3 gauges to Update Periodically

| Gauge | Source | Schedule |
|-------|--------|----------|
| `oxios_agents_running` | `supervisor.list()` | Per `/api/status` call or 10s tick |
| `oxios_memory_entries_total` | `memory_stats()` | Per `/api/status` call |
| `oxios_active_sessions` | Session count | Per `/api/status` call |

---

## 4. What NOT to Do

Per the brief constraints:

- ❌ Do NOT add `prometheus` crate — we have our own `MetricsRegistry`
- ❌ Do NOT implement a Prometheus scraper — just export text format
- ❌ Do NOT change the `AuditTrail` — it's working as designed
- ❌ Do NOT add alerting rules — out of scope
