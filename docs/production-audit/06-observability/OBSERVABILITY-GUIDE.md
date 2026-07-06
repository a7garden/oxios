# Observability Guide

**Version:** 0.6.0  
**Date:** 2026-05-31  
**Scope:** How to enable, configure, and use Oxios observability features

---

## 1. Overview

Oxios provides three layers of observability:

| Layer | Purpose | Default |
|-------|---------|---------|
| **Structured logging** | Development & production debugging | ✅ Enabled (`tracing`) |
| **Metrics** | Operational health monitoring | ✅ Enabled (counters/gauges) |

---

## 2. Structured Logging

### 2.1 Configuration

```toml
# ~/.oxios/config.toml

[logging]
format = "pretty"    # "pretty" | "json" | "compact"
level = "info"       # "trace" | "debug" | "info" | "warn" | "error"
```

Or via environment variable:

```bash
RUST_LOG=debug cargo run -- --foreground
```

Priority: `RUST_LOG` env var → `[logging].level` → `info` (default)

### 2.2 Log Output

Logs go to two destinations simultaneously:

1. **Stdout** — via `tracing_subscriber::fmt()` with the configured format
2. **File** — rolling daily files at `[daemon].log_dir/oxios.log` (non-blocking)

### 2.3 JSON Format (Production)

```toml
[logging]
format = "json"
```

Output example:

```json
{"timestamp":"2026-05-31T12:00:00.123Z","level":"INFO","target":"oxios_kernel::orchestrator","spans":[{"name":"handle_message"}],"message":"starting","session_id":"a1b2c3","name":"orchestrator.handle_message"}
```

Compatible with: ELK stack, Grafana Loki, AWS CloudWatch, Datadog.

### 2.4 Log Locations

| Path | Purpose |
|------|---------|
| `~/.oxios/logs/oxios.log` | Daily rolling log file |
| `~/.oxios/logs/oxios.log.YYYY-MM-DD` | Rotated logs |
| stdout | Live console output |

---

## 3. Metrics

### 3.1 Endpoints

| Endpoint | Auth | Format | Purpose |
|----------|------|--------|---------|
| `GET /metrics` | None | Prometheus text | Public scraping endpoint |
| `GET /api/metrics` | Required | Prometheus text | Authenticated scraping |
| `GET /api/status` | Required | JSON | Component health + agent counts |
| `GET /api/scheduler/stats` | Required | JSON | Queue depth, running tasks |

### 3.2 Available Metrics

**Agent metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `oxios_agents_forked_total` | Counter | Total agents spawned |
| `oxios_agents_running` | Gauge | Currently active agents |
| `oxios_agents_completed_total` | Counter | Successfully completed |
| `oxios_agents_failed_total` | Counter | Failed executions |

**Orchestration metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `oxios_messages_processed_total` | Counter | User messages processed |
| `oxios_orchestration_duration_seconds` | Histogram | Full orchestration duration |

Buckets: 0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0

**LLM metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `oxios_llm_calls_total` | Counter | Total LLM API calls |
| `oxios_llm_duration_seconds` | Histogram | LLM call latency |
| `oxios_llm_errors_total` | Counter | LLM API errors |
| `oxios_llm_circuit_breaker_state` | Gauge | 0=closed, 1=open, 2=half_open |

**Tool metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `oxios_tool_calls_total` | Counter | Tool invocations |
| `oxios_tool_duration_seconds` | Histogram | Tool execution time |
| `oxios_tool_errors_total` | Counter | Tool execution errors |

**System metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `oxios_memory_entries_total` | Gauge | Memory store entries |
| `oxios_memory_recall_total` | Counter | Memory recall operations |
| `oxios_active_sessions` | Gauge | Active sessions |
| `oxios_exec_total` | Counter | Exec tool calls |
| `oxios_exec_duration_seconds` | Histogram | Exec duration |

### 3.3 Example Prometheus Scrape

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'oxios'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:4200']
    metrics_path: '/metrics'
```

### 3.4 Example Grafana Queries

```promql
# Agents spawned per minute
rate(oxios_agents_forked_total[1m]) * 60

# P95 orchestration latency
histogram_quantile(0.95, rate(oxios_orchestration_duration_seconds_bucket[5m]))

# LLM error rate
rate(oxios_llm_errors_total[5m]) / rate(oxios_llm_calls_total[5m]) * 100

# Circuit breaker state (0=healthy)
oxios_llm_circuit_breaker_state

# Active agents
oxios_agents_running

# Scheduler queue depth (from /api/scheduler/stats)
# (Not in Prometheus format — use JSON endpoint)
```

---

## 4. Distributed Tracing (Removed)

OpenTelemetry/OTLP export has been **removed** from Oxios. The feature was
never implemented end-to-end (the exporter was a no-op stub and no span
instrumentation existed), and the optional `otel` feature gate carried heavy
dependencies for no runtime value. Local observability — structured logs
(`tracing_subscriber::fmt`) and Prometheus metrics — remains fully supported
(see §2 and §3).

---

## 5. Health Checks

### 5.1 Endpoints

| Endpoint | Purpose | Auth |
|----------|---------|------|
| `GET /health` | Liveness — is the process running? | None |
| `GET /health/ready` | Readiness — are subsystems healthy? | None |
| `GET /api/status` | Detailed status with component health | Required |

### 5.2 Response Examples

**Liveness (`GET /health`):**

```json
{
  "status": "ok",
  "version": "0.6.0"
}
```

**Readiness (`GET /health/ready`):**

```json
{
  "status": "healthy",
  "version": "0.6.0",
  "components": {
    "state_store": { "healthy": true },
    "git": { "healthy": true },
    "memory": { "healthy": true, "index_size": 12, "total_entries": 34 }
  }
}
```

Possible values for `status`: `"healthy"`, `"degraded"`.

### 5.3 Load Balancer Configuration

```nginx
# nginx upstream health check
upstream oxios {
    server 127.0.0.1:4200;
}

# Liveness check
location /health {
    proxy_pass http://oxios;
}

# Readiness check (for Kubernetes)
# readinessProbe:
#   httpGet:
#     path: /health/ready
#     port: 4200
```

---

## 6. What to Monitor in Production

### 6.1 Critical Alerts

| Alert | Condition | Severity |
|-------|-----------|----------|
| Liveness | `/health` returns non-200 | 🔴 Critical |
| Readiness | `/health/ready` returns `"degraded"` | 🟡 Warning |
| Circuit breaker open | `oxios_llm_circuit_breaker_state == 1` | 🔴 Critical |
| High error rate | `rate(oxios_agents_failed_total[5m]) > 0.1` | 🟡 Warning |
| Queue buildup | `scheduler.queue_depth > 20` | 🟡 Warning |
| LLM latency | `histogram_quantile(0.99, oxios_llm_duration_seconds) > 30` | 🟡 Warning |

### 6.2 Dashboards

**Recommended panels:**

1. **Agent Overview** — active agents, spawn rate, completion rate
2. **LLM Performance** — request rate, latency P50/P95/P99, error rate
3. **Circuit Breaker** — state over time (closed/open/half_open)
4. **Scheduler** — queue depth, running tasks, zombie detection
5. **Orchestration** — phase durations, evaluation scores
6. **Memory** — entry count, recall rate, tier distribution

### 6.3 Log Queries (JSON format)

```bash
# All logs for a session
cat ~/.oxios/logs/oxios.log | jq 'select(.spans[]?.session_id == "abc123")'

# All LLM errors
cat ~/.oxios/logs/oxios.log | jq 'select(.level == "ERROR" and .target | contains("engine"))'

# Tool calls for a specific agent
cat ~/.oxios/logs/oxios.log | jq 'select(.spans[]?.agent_id == "xyz789")'
```

---

## 7. Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Observability Stack                        │
│                                                              │
│  ┌──────────────────┐                                        │
│  │ tracing crate     │── 440 structured log calls             │
│  │ (always active)   │                                        │
│  └──────┬───────────┘                                        │
│         │                                                    │
│    ┌────┴─────┐                                              │
│    │ Subscriber│── EnvFilter → fmt (pretty/json/compact)      │
│    │          │── file appender (rolling daily)                │
│    └──────────┘                                              │
│                                                              │
│  ┌──────────────────┐                                        │
│  │ MetricsRegistry   │── 22 metrics (counters/gauges/hists)   │
│  │ (always active)   │                                        │
│  └──────┬───────────┘                                        │
│         │                                                    │
│    GET /metrics ──────► Prometheus text export                 │
│    GET /api/metrics ──► Prometheus text export (auth)          │
│    GET /api/status ───► JSON (agents, health, uptime)         │
│                                                              │
│  ┌──────────────────┐                                        │
│  │ oxi-sdk Tracer    │── Distributed spans (1 call site)       │
│  │ CostTracker       │── Per-agent token tracking (1 site)     │
│  │ AuditLog          │── Structured security events (0 sites)  │
│  └──────────────────┘                                        │
│                                                              │
│  ┌──────────────────┐                                        │
│  │ AuditTrail        │── Merkle-chain tamper-evident log       │
│  │ (always active)   │                                        │
│  └──────────────────┘                                        │
│                                                              │
│  ┌──────────────────┐                                        │
│  │ ResourceMonitor   │── CPU, memory, load, disk sampling      │
│  │ (always active)   │                                        │
│  └──────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. Changelog (from this audit)

| Change | File | Description |
|--------|------|-------------|
| Metrics init | `src/kernel.rs` | Call `register_builtin_metrics()` + `observability::init()` at startup |
