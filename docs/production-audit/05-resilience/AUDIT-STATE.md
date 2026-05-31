# State Classification Audit

**Area:** Resilience — what survives restart, what doesn't  
**Date:** 2026-05-31  
**Scope:** All stateful subsystems in the Oxios kernel

---

## Methodology

Read the following source files to classify every piece of state:
- `src/kernel.rs` — what's created at `KernelBuilder::build()`
- `crates/oxios-kernel/src/state_store.rs` — filesystem persistence
- `crates/oxios-kernel/src/orchestrator.rs` — in-memory sessions
- `crates/oxios-kernel/src/scheduler.rs` — task queue state
- `surface/oxios-web/src/routes/system.rs` — existing health endpoints

For each state type, we classify it into one of three categories:

| Category | Meaning |
|----------|---------|
| **PERSISTED** | Already survives process restart |
| **EPHEMERAL-ACCEPTABLE** | Fine to lose on restart |
| **EPHEMERAL-PROBLEM** | Should survive but doesn't |

---

## PERSISTED — Survives Restart

| State | Location | Persistence Mechanism |
|-------|----------|-----------------------|
| **Sessions (StateStore)** | `StateStore.save_session()` | JSON files in `~/.oxios/workspace/sessions/{id}.json` |
| **Seeds** | `StateStore.save_json("seeds", ...)` | JSON files in `~/.oxios/workspace/seeds/{id}.json` |
| **Agent Groups** | `StateStore.save_json("agent_groups", ...)` | JSON files in `~/.oxios/workspace/agent_groups/` |
| **Configuration** | `config.toml` | Direct file I/O, hot-reloadable via `PUT /api/config` |
| **Knowledge Base** | `~/.oxios/workspace/knowledge/` | `.md` files, git-tracked via `GitLayer` |
| **Git Layer** | `~/.oxios/workspace/.git/` | Full git history via `gix` |
| **Memory (JSON)** | `StateStore.save_json("memories", ...)` | JSON files in `~/.oxios/workspace/memories/` |
| **Memory (SQLite)** | `~/.oxios/workspace/memory.db` | SQLite database (feature-gated) |
| **Audit Trail** | `StateStore.load_audit_entries()` | Persisted on flush, restored on startup |
| **Cron Jobs** | `StateStore` (via `CronScheduler`) | JSON files in `~/.oxios/workspace/cron/` |
| **MCP Tool Cache** | N/A (ephemeral by design) | Re-discovered on startup via `initialize_all()` |
| **Skills** | `~/.oxios/workspace/skills/*/SKILL.md` | Filesystem-based, loaded on startup |
| **Web UI** | `~/.oxios/web/dist/` | Downloaded by daily health check |

**Total persisted: 12 subsystems.** The filesystem-based StateStore provides good durability for the data layer.

---

## EPHEMERAL-ACCEPTABLE — Fine to Lose

| State | Location | Why Acceptable |
|-------|----------|----------------|
| **In-flight HTTP requests** | Axum connections | Clients will retry; load balancers handle this |
| **SSE event streams** | `/api/events` subscribers | Clients reconnect and re-subscribe |
| **WebSocket chat streams** | `/api/chat/stream` | Chat messages are persisted to sessions; stream can resume |
| **Rate limiter window** | `Scheduler.rate_limiter` | Window resets on restart — acceptable burst |
| **Routing stats** | `RoutingStats` (in-memory) | Stats are advisory, not critical |
| **Resource monitor history** | `ResourceMonitor.history` | Loss of recent CPU/memory samples is non-critical |
| **Conversation buffer** | `Orchestrator.conversation_buffer` | Only used for topic-shift detection within a running session |
| **MCP tool cache** | `McpClient.tool_cache` | Re-populated via `list_tools()` on next call |
| **BudgetManager state** | `BudgetManager` (in-memory) | Per-window budgets reset — acceptable for short windows |
| **A2A circuit breaker state** | `A2ACircuitBreaker` | Resets to Closed on restart — conservative behavior |
| **A2A agent registry** | `A2AProtocol` (in-memory) | Agents re-register via lifecycle on startup |

**Total acceptable: 11 subsystems.** These are caches, windows, or advisory state.

---

## EPHEMERAL-PROBLEM — Should Survive But Doesn't

| State | Location | Impact | Severity |
|-------|----------|--------|----------|
| **Orchestrator interview sessions** | `Orchestrator.sessions` (`HashMap<String, InterviewSession>`) | Active multi-turn interviews lost. User's `--session` flag stops working. The CLI session chain breaks. | 🔴 High |
| **Scheduler running tasks** | `AgentScheduler.running` (`HashMap<Uuid, ScheduledTask>`) | Tasks tracked as "Running" are lost. No completion/failure recorded. Agents may be orphaned. | 🟡 Medium |
| **Scheduler task queue** | `AgentScheduler.queue` (`BinaryHeap<ScheduledTask>`) | Queued tasks dropped. No recovery or retry. | 🟡 Medium |
| **Scheduler start times** | `AgentScheduler.task_start_times` | Zombie detection resets — won't catch tasks that started before restart. | 🟢 Low |
| **Supervisor agent tracking** | `BasicSupervisor` (in-memory) | Running agents lose their tracked state. They continue executing (Tokio tasks) but can't be listed/killed via API. | 🟡 Medium |
| **Orchestrator A2A delegation state** | In-flight `JoinSet` tasks | Delegated subtasks in progress are lost. Parent seed may report partial results. | 🟡 Medium |

### Detailed Analysis

#### 1. Interview Sessions (🔴 Critical)

```rust
// orchestrator.rs
sessions: RwLock<HashMap<String, InterviewSession>>,
```

**Current behavior:** When the user sends a message with `session_id`, the orchestrator looks up the session in this in-memory HashMap. On restart, the HashMap is empty, so:
- Follow-up messages with `--session` create a NEW session
- Multi-turn interview context (ambiguity resolution) is lost
- The CLI `--session` feature documented in AGENTS.md breaks

**Mitigating factor:** The `StateStore` already has `save_session()` and `load_session()` methods. The `Session` struct in `state_store.rs` is fully serializable. However, `InterviewSession` in the orchestrator is a different struct (contains `InterviewResult`, `Phase`, etc.) that is NOT the same as the `Session` in StateStore.

**Gap:** The orchestrator's `InterviewSession` is richer than `StateStore.Session`. The StateStore session only has messages/responses, not interview state (ambiguity, phase, readiness).

#### 2. Scheduler State (🟡 Medium)

```rust
// scheduler.rs
queue: Arc<Mutex<BinaryHeap<ScheduledTask>>>,
running: Arc<Mutex<HashMap<Uuid, ScheduledTask>>>,
```

**Current behavior:** Both the priority queue and running task map are purely in-memory. On restart:
- Queued tasks vanish — the user never gets a result
- Running tasks' state is lost — no completion or failure callbacks
- If agents were executing via Tokio tasks, they continue but become untracked

**Mitigating factor:** The scheduler is typically used for short-lived operations. In practice, the queue is usually empty between orchestrations.

#### 3. Supervisor Agent Tracking (🟡 Medium)

```rust
// supervisor.rs (BasicSupervisor)
```

The supervisor tracks running agents in memory. On restart:
- `list()` returns empty
- `kill()` has no targets
- Agents may still be running as Tokio tasks (they're not OS processes)

**Mitigating factor:** Since agents are Tokio tasks (not OS processes), they die with the process. No orphan processes.

---

## Summary

| Category | Count | Percentage |
|----------|-------|------------|
| PERSISTED | 12 | 52% |
| EPHEMERAL-ACCEPTABLE | 11 | 48% |
| EPHEMERAL-PROBLEM | 6 | — |

**Key finding:** The data layer is well-persisted. The operational layer (scheduler, supervisor, orchestrator sessions) is entirely ephemeral. This is an intentional design choice — sessions are documented as ephemeral in AGENTS.md. The gap is that the orchestrator's `InterviewSession` is richer than what `StateStore.Session` captures, so even though sessions ARE persisted, the interview state is not.

**Recommendation priority:**
1. 🟡 Persist interview state in the orchestrator (bridges `InterviewSession` → `StateStore`)
2. 🟡 Persist scheduler queue (nice-to-have, low real-world impact)
3. 🟢 Accept supervisor/scheduler running state as ephemeral (agents die with process)
