# Resilience Model

**Area:** How Oxios handles failures — what works, what doesn't, what needs fixing  
**Date:** 2026-05-31

---

## Overview

Oxios is a long-running daemon that orchestrates AI agents. It faces several categories of failure:

1. **External service failures** — LLM provider down, MCP server crash, Telegram API error
2. **Internal state loss** — process restart, crash, upgrade
3. **Resource exhaustion** — memory pressure, disk full, API rate limits
4. **Agent failures** — tool execution error, LLM hallucination, evaluation failure

This document maps the resilience posture for each category.

---

## What Fails Gracefully ✅

### LLM Provider Failures
- **Circuit Breaker** (3-state: Closed → Open → Half-Open) protects against cascading failures
- **Provider failover** via oxi-sdk — if one provider fails, alternatives are tried
- **Rate limiting** in the scheduler prevents API overload
- **Budget enforcement** prevents runaway token consumption
- **Graceful degradation** — the agent loop handles provider errors without crashing

### MCP Server Crashes
- **Auto-restart** on communication errors (broken pipe, timeout, no response)
- **Tool cache** means the server doesn't need to be fully re-initialized on restart
- **Non-blocking** — MCP errors don't crash the orchestrator; they're returned as tool errors

### Telegram Channel Disruptions
- **Exponential backoff** (5s → 10s → 20s → 40s → 80s) on polling failures
- **Automatic recovery** — the long-poll loop self-heals after transient errors

### A2A Delegation Failures
- **Three-layer protection:**
  1. Retry with exponential backoff (3 attempts, 100ms–5s)
  2. Circuit breaker (5 failures → Open, 30s timeout → Half-Open)
  3. Fallback to direct lifecycle execution (guaranteed completion)

### Resource Pressure
- **ResourceMonitor** tracks CPU, memory, disk usage
- **Guardian daemon** checks every 5 minutes and logs warnings
- **`is_overloaded()` gate** — scheduler can reject new tasks under pressure
- **BudgetManager** — per-agent token/call limits prevent resource hogging

### Process Shutdown
- **Graceful shutdown** via SIGINT → Ctrl+C:
  1. Gateway stops accepting new messages
  2. Surface and channel tasks are aborted
  3. Gateway drains with 10s timeout
  4. Running agents are killed (parallel, with error logging)
  5. MCP servers are shut down
  6. Audit trail is flushed to disk
- **Atomic file writes** — StateStore uses temp-file + rename pattern
- **Audit trail flush** — all entries persisted before exit

### Configuration Errors
- **Config validation** — `load_config()` validates the TOML structure
- **Hot reload** — `PUT /api/config` validates before persisting
- **API key masking** — never exposed in GET responses

---

## What Doesn't Fail Gracefully ❌

### Active Session Loss on Restart
- **Impact:** All multi-turn interviews break. CLI `--session` becomes unusable.
- **Current mitigation:** None. Documented as known limitation.
- **Fix:** Session persistence design (see `SESSION-PERSISTENCE-DESIGN.md`).
- **Priority:** 🟡 Medium (not critical because sessions are typically short-lived)

### MCP Tool Call Loss After Restart
- **Impact:** When an MCP server crashes and auto-restarts, the original tool call returns an error. The agent must retry in its next turn.
- **Current mitigation:** The agent's LLM may retry, but this depends on prompt engineering.
- **Fix:** Add 1-2 automatic retries in `send_request()` after successful restart.
- **Priority:** 🟢 Low (agent retry is a reasonable fallback)

### Scheduler Queue Loss on Restart
- **Impact:** Queued tasks are dropped. No retry or notification.
- **Current mitigation:** In practice, the queue is usually empty between orchestrations.
- **Fix:** Persist queue to StateStore, reload on startup.
- **Priority:** 🟢 Low (minimal real-world impact)

### Partial Multi-Agent Results
- **Impact:** When `delegate_via_lifecycle()` is running and the process crashes, some subtasks may have completed while others are lost. The parent seed gets no result.
- **Current mitigation:** The `agent_groups/` directory stores group state, but only after all tasks complete.
- **Fix:** Write partial results incrementally (one file per subtask).
- **Priority:** 🟢 Low (rare scenario, user can retry the original prompt)

---

## What's By Design (Ephemeral)

These are explicitly ephemeral and should stay that way:

| State | Reason |
|-------|--------|
| In-flight HTTP connections | Clients handle reconnection |
| SSE event streams | Subscribers reconnect |
| Rate limiter windows | Reset on restart is conservative |
| Routing statistics | Advisory metrics, not critical |
| Resource monitor history | Short-lived samples, non-critical |
| Conversation buffer | Topic-shift cache, session-scoped |
| Budget manager state | Per-window budgets, short-lived |
| A2A circuit breaker | Resets to Closed (conservative) |
| A2A agent registry | Agents re-register on startup |

---

## What Needs Fixing (Priority Order)

| # | Issue | Severity | Effort | ROI |
|---|-------|----------|--------|-----|
| 1 | **Health check endpoint** (`/api/health`) | 🟡 Medium | 🟢 Small | 🔴 High |
| 2 | **Session persistence** | 🟡 Medium | 🟡 Medium | 🟡 Medium |
| 3 | **MCP retry after restart** | 🟢 Low | 🟢 Small | 🟡 Medium |
| 4 | **Scheduler queue persistence** | 🟢 Low | 🟡 Medium | 🟢 Low |
| 5 | **Partial multi-agent checkpointing** | 🟢 Low | 🔴 Large | 🟢 Low |

### Rationale

1. **Health check** — Essential for production deployment. Load balancers, Kubernetes, and monitoring systems need a `/api/health` endpoint. The existing `/health` (liveness) and `/health/ready` (readiness) are good but `/api/health` with kernel component status is missing.

2. **Session persistence** — The design is documented. Implementation is straightforward (StateStore already has the methods). The main risk is the TTL heuristic (too short = sessions lost, too long = stale state restored).

3. **MCP retry** — A 10-line change with significant reliability improvement for MCP-heavy workloads.

4. **Scheduler persistence** — Low ROI because the queue is usually empty. Only matters during high-throughput multi-agent scenarios.

5. **Partial checkpointing** — High effort for a rare scenario. The orchestrator's evaluate/evolve loop handles failed executions; the user can always retry.

---

## Resilience Invariants

These are properties that should always hold true:

1. **Data never silently lost.** All writes go through StateStore's atomic rename. Audit trail is flushed on shutdown.
2. **No orphan processes.** Agents are Tokio tasks, not OS processes. They die with the process.
3. **Circuit breakers protect external calls.** LLM provider and A2A delegation are circuit-breaker protected.
4. **Graceful shutdown completes.** The SIGINT handler ensures all cleanup runs within timeouts.
5. **Config changes are safe.** Validation before persist, hot-reload after persist.
6. **MCP servers auto-recover.** Crashed MCP servers are detected and restarted on next call.

---

## Monitoring Recommendations

For production deployment, monitor these signals:

| Signal | Source | Alert Threshold |
|--------|--------|-----------------|
| `/api/health` != 200 | Health endpoint | Immediate |
| Circuit breaker Open | `A2ACircuitBreaker.state()` | After 2 Open events in 5 min |
| Agent failure rate | `oxios_agents_failed_total` | >30% failure rate |
| Queue depth | `SchedulerStats.queued` | >10 for >5 min |
| Memory usage | `ResourceMonitor.snapshot()` | >90% of system RAM |
| Uptime | `InfraApi.uptime()` | Alert on restart (uptime reset) |
| MCP restart count | Logs (auto-restart messages) | >3 restarts per server per hour |
