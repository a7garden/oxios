# Brief 06: Observability — OpenTelemetry & Metrics

**Area:** Tracing, metrics, alerting, operational visibility  
**Severity:** 🟢 Normal  
**Estimated scope:** OTel stub, 439 tracing calls, no metrics endpoint  

---

## Context

Oxios has solid tracing infrastructure:

- **439 structured `tracing!` calls** across the codebase
- **OpenTelemetry integration** exists in `src/otel.rs` — but it's a
  **stub** when disabled (which is the default)
- **CostTracker** — token/cost tracking per agent
- **AuditTrail** — Merkle-chain tamper-evident audit log
- **ResourceMonitor** — system resource tracking
- **Daily health check** — runs at 03:00 (internal, not exposed)

**What's missing:**
- No metrics endpoint (Prometheus `/metrics`)
- No alerting mechanism
- OTel is stub-only — no real spans exported in default config
- No request tracing (correlation IDs across tool calls)
- No dashboard for operational metrics (agent count, queue depth, error
  rate)

**Current config:**
```toml
[otel]
# OpenTelemetry configuration. Disabled by default.
```

---

## Objective

1. **Assess** what observability is already in place and working
2. **Activate** OTel spans for the default configuration (or make
  activation trivial)
3. **Design** a metrics endpoint (do NOT implement a full Prometheus
  integration)
4. **Document** the observability model

This does NOT mean:
- ❌ Building a monitoring dashboard
- ❌ Adding Prometheus as a dependency
- ❌ Creating custom metrics SDK infrastructure
- ❌ Adding alerting rules or pager integration

It DOES mean:
- ✅ Making OTel work with a real exporter (OTLP)
- ✅ Adding a `/api/metrics` endpoint that exposes basic counters
- ✅ Adding correlation/request IDs to the tracing spans
- ✅ Documenting what to monitor and how

---

## Approach

### Phase 1: Observability Audit

1. Read `src/otel.rs` — understand the stub vs real implementation
2. Read `crates/oxios-kernel/src/observability.rs` if it exists
3. Read the `otel` feature gate in `Cargo.toml`
4. Catalog all existing tracing spans by module
5. Write audit to `docs/production-audit/06-observability/AUDIT-OBSERVABILITY.md`

### Phase 2: Activate OTel

1. The OTel feature exists but is likely never tested. Verify:
   - `cargo build --features otel` compiles
   - What happens when you enable it in config.toml?
2. If it compiles, test with a local OTLP collector (or document the
   setup instructions)
3. If it doesn't compile, fix the issues
4. Ensure the default (no OTel) path still uses `tracing_subscriber`
   for stdout logging — verify log output is structured and useful

### Phase 3: Basic Metrics Design

Design a simple metrics endpoint. No new dependencies — use atomic
counters that already exist or can be trivially added:

```json
GET /api/metrics
{
  "agents": { "active": 3, "total_spawned": 47 },
  "scheduler": { "queue_depth": 2, "completed": 45 },
  "llm": { "requests": 230, "errors": 3, "circuit_breaker": "closed" },
  "memory": { "hot": 12, "warm": 34, "cold": 89 },
  "uptime_secs": 86400,
  "version": "0.6.0"
}
```

Write design to `docs/production-audit/06-observability/METRICS-DESIGN.md`

### Phase 4: Correlation IDs

Assess whether request/correlation IDs flow through the system:

1. When a user sends a message via Web/CLI/Telegram → is there a
   trace ID that follows it through orchestrator → agent → tools?
2. If not, design where to add them (likely: gateway assigns ID,
   passes through Orchestrator → AgentRuntime)
3. This is a design document only — `docs/production-audit/06-observability/CORRELATION-ID-DESIGN.md`

### Phase 5: Observability Guide

Write `docs/production-audit/06-observability/OBSERVABILITY-GUIDE.md`:

- How to enable OTel
- What metrics are available
- What to monitor in production
- Example Grafana queries (if OTel is active)
- Log format reference

---

## Constraints

- **Do not** add new crate dependencies for metrics
- **Do not** implement Prometheus exporter
- **Do not** change the tracing span structure
- **Do not** modify the AuditTrail (it's working as designed)
- **Keep** OTel as a feature gate — never mandatory

## Verification

1. `cargo build --features otel` — compiles
2. `cargo build` (default features) — still compiles
3. `cargo test --workspace` — all tests pass
