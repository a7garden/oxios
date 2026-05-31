# Brief 05: Resilience — Session Persistence & Retry Hardening

**Area:** Session lifecycle, retry logic, state persistence  
**Severity:** 🟡 Medium  
**Estimated scope:** In-memory sessions, partial retry coverage, no state recovery  

---

## Context

Oxios processes are long-running daemons. A crash, restart, or upgrade
currently loses:

- **All active sessions** — stored in `Orchestrator` memory
- **Agent runtime state** — not persisted
- **In-flight task queue** — scheduler state is ephemeral

From AGENTS.md: *"Sessions live in orchestrator memory. Process restart
loses them. Use `--session` only within a single CLI session chain."*

**Current resilience mechanisms:**
- ✅ Circuit Breaker for LLM providers (3-state)
- ✅ Graceful shutdown (SIGINT → gateway shutdown → MCP cleanup)
- ✅ Rate limiting (scheduler + ProviderPool)
- ✅ Budget enforcement (BudgetManager)
- ✅ Config validation
- ⚠️ Telegram retry with exponential backoff (only channel with retry)
- ❌ No session persistence
- ❌ No agent state checkpointing
- ❌ No scheduler state recovery after restart
- ❌ No health check endpoint (HTTP `/health`)

---

## Objective

1. **Design (not implement)** session persistence
2. **Audit** existing retry/fallback mechanisms for completeness
3. **Add a health check endpoint** to the web server
4. **Document** the resilience model and its gaps

This does NOT mean:
- ❌ Implementing a full database layer for sessions
- ❌ Adding Redis/Postgres/etc. for state management
- ❌ Building a distributed consensus system
- ❌ Over-engineering failure scenarios

It DOES mean:
- ✅ Using SQLite (already a dependency, feature-gated) for session
  persistence if the design calls for it
- ✅ Leveraging the existing `StateStore` which already persists to disk
- ✅ Keeping session recovery simple: load on startup, save on change
- ✅ Adding `/api/health` for load balancer / monitoring probes

---

## Approach

### Phase 1: Resilience Audit

Map every point where state can be lost:

1. Read `src/kernel.rs` — understand what's created at startup
2. Read `crates/oxios-kernel/src/state_store.rs` — what's already persisted?
3. Read the Orchestrator — what's only in memory?
4. Read the Scheduler — what's the queue state?

For each state type, classify:
- **PERSISTED** — already survives restart (StateStore, GitLayer, etc.)
- **EPHEMERAL-ACCEPTABLE** — fine to lose (in-flight HTTP requests)
- **EPHEMERAL-PROBLEM** — should survive but doesn't (sessions, schedule)

Write results to `docs/production-audit/05-resilience/AUDIT-STATE.md`

### Phase 2: Session Persistence Design

Design a session persistence mechanism:

```
Option A: StateStore expansion
- StateStore already has save/load for JSON blobs
- Add session serialization to StateStore on every state transition
- Load sessions on startup
- Pro: Uses existing infrastructure
- Con: StateStore may not have the right query patterns

Option B: Dedicated session file
- Serialize session state to ~/.oxios/workspace/sessions/{id}.json
- Write on state change, load on startup
- Pro: Simple, file-per-session, easy to debug
- Con: Yet another persistence mechanism
```

Choose the simpler option. Document the design in
`docs/production-audit/05-resilience/SESSION-PERSISTENCE-DESIGN.md`.

Include:
- Data model (what gets persisted)
- Write timing (when to save — every turn? end of session?)
- Recovery flow (what happens on startup)
- Cleanup (stale session handling)
- Migration (if the session schema changes)

### Phase 3: Health Check Endpoint

Add a simple `/api/health` endpoint to `surface/oxios-web/src/routes/`:

```rust
// Returns 200 if kernel is operational
// Returns 503 if any critical subsystem is degraded
{
  "status": "ok" | "degraded",
  "kernel": true,
  "gateway": true,
  "scheduler": true,
  "uptime_secs": 12345
}
```

This should be a thin read of existing health signals — do NOT add
new health monitoring infrastructure.

### Phase 4: Retry Gap Analysis

Audit retry/fallback coverage per channel:

| Component | Has Retry? | Notes |
|-----------|-----------|-------|
| LLM Provider Call | ✅ Circuit Breaker | oxi-sdk handles |
| Telegram Bot | ✅ Exponential backoff | |
| Web API | ❌ | Client-side concern |
| MCP Client | ? | Audit needed |
| Tool Execution | ? | Audit needed |
| Session Recovery | ❌ | |

Write analysis to `docs/production-audit/05-resilience/RETRY-GAP-ANALYSIS.md`

### Phase 5: Resilience Model Document

Write `docs/production-audit/05-resilience/RESILIENCE-MODEL.md`:

- What fails gracefully
- What doesn't
- What's by design (ephemeral sessions)
- What needs fixing
- Priority order for implementation

---

## Constraints

- **Do not** implement the session persistence (design only)
- **Do not** add new crate dependencies for state management
- **Do not** change the Orchestrator architecture
- **Do not** add distributed system features
- **Do** implement the health check endpoint (it's small and high-value)
- **Preserve** the existing graceful shutdown flow

## Verification

1. `cargo test --workspace` — all tests pass
2. Health endpoint responds: `curl http://localhost:4200/api/health`
3. Design documents are reviewable and actionable
