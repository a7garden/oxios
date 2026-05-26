# Oxios Kernel Architecture Analysis

> Generated: 2026-05-26
> Files analyzed: 7 core kernel modules

---

## 1. `lib.rs` — Module Root & Public API Surface

### Public API Surface Size
- **~38 public modules** (organized into 8 domain sections: Lifecycle, Orchestration, Security, Communication, Intelligence, Tools & Skills, State & Config, Infrastructure)
- **~120+ re-exported types/traits** across all domains
- **~80 oxi-sdk re-exports** (Agent, AgentBuilder, AgentTool, Oxi, Provider, etc.)
- **Feature gates:** `wasm-sandbox`, `embedding-gguf`, `sqlite-memory`, `otel`, `browser`

### Module Coupling
- **Zero coupling to internal implementation** — `lib.rs` only declares `pub mod` and `pub use`. No logic.
- Re-exports are extensive, creating a "facade kernel" pattern where downstream crates depend on `oxios_kernel::Type` rather than `oxios_kernel::some_module::Type`.

### Error Handling
- Re-exports `KernelError` and `KernelResult` from the `error` module.
- No error handling in this file itself (pure declarations).

### Single Responsibility
✅ **Clear single responsibility:** API surface declaration and re-exports. Well-organized into thematic sections with Korean comments explaining each group.

### Architectural Observations
- **Very wide API surface** (~200 public items). This is intentional — the kernel is the central hub.
- **Heavy oxi-sdk coupling** — ~80 types re-exported directly from `oxi_sdk`. This makes the kernel tightly coupled to a specific SDK version.
- **Circuit breaker delegation** — `ProviderCircuitBreaker` is re-exported as `CircuitBreaker`, providing a kernel-local alias for an oxi-sdk type. Clean indirection.
- **Feature-gated telemetry** — Clean `#[cfg(feature = "otel")]` / `#[cfg(not(feature = "otel"))]` pattern with `pub use telemetry_otel as telemetry` / `pub use telemetry_stub as telemetry`.

### Smells
1. **Re-export volume is very high.** Consider selective re-exports or a prelude pattern. Downstream code likely uses `oxios_kernel::` paths that could be shortened.
2. **Duplicate Korean comment block** in the Lifecycle section (lines 9-12 and 13-16 are identical).

---

## 2. `kernel_handle/mod.rs` — KernelHandle Facade

### Public API Surface Size
- **14 public submodules** (13 domain APIs + mod.rs)
- **15 public types** re-exported (13 API facades + KnowledgeLens types)
- **`KernelHandle` struct** with 15 fields + ~13 public methods
- **1 deprecated method** (`from_subsystems`)

### Module Coupling
- **Heavy coupling to every kernel subsystem:** Imports from ~22 internal modules (StateStore, EventBus, Supervisor, Scheduler, MemoryManager, GitLayer, AuditTrail, BudgetManager, ResourceMonitor, CronScheduler, SkillManager, PersonaManager, McpBridge, AuthManager, AccessManager, OxiosConfig, SpaceManager).
- **One external dependency:** `oxios_markdown::KnowledgeBase` (direct, kernel-free).

### Error Handling
- Convenience methods use `anyhow::Result<()>` consistently.
- `from_subsystems` uses `.expect()` for KnowledgeBase creation (will panic on failure).
- Git operations use `let _ =` to silently ignore errors (intentional — git is best-effort).

### Single Responsibility
✅ **Clear responsibility:** Typed facade / system call table for the kernel. Composes 13 domain APIs.

### Architectural Observations
- **Facade pattern** is clean: each domain API is its own struct with its own file (`*_api.rs`). KernelHandle composes them all.
- **`from_subsystems` is deprecated** — construction now goes through `KernelHandle::new()` with pre-built Facades, assembled in the binary crate's `kernel.rs`. Good evolution.
- **Cross-facade convenience methods** (`save_and_commit`, `flush_audit`, etc.) orchestrate across multiple facades. These are the "system calls" that span domains.
- **KnowledgeBase is direct** — web channel bypasses kernel entirely for knowledge operations. This is a deliberate design decision documented in AGENTS.md.

### Smells
1. **`from_subsystems` is 50+ lines of construction code** — still present though deprecated. Should be removed or gated behind a feature flag.
2. **`knowledge: Arc<oxios_markdown::KnowledgeBase>`** is a direct dependency on an external crate inside the kernel facade. Slightly breaks the "kernel is self-contained" principle, but is documented as intentional.
3. **`unschedule` swallows errors** — returns `Ok(false)` on any failure, including UUID parse errors that should probably be surfaced.
4. **No `Clone` derivation on KernelHandle** — some API facades may not be `Clone`, but this means Arc<KernelHandle> is mandatory everywhere. Intentional but constraining.

---

## 3. `orchestrator.rs` — The Brain

### Public API Surface Size
- **3 public types:** `AgentRole`, `SubTask`, `Orchestrator`
- **1 public result type:** `OrchestrationResult` (9 fields)
- **~12 public methods** on `Orchestrator` (constructor, setters, `handle_message`, `delegate_subtasks`)
- **5 private helper functions** (format_questions, format_execution_result, should_split_seed, split_into_subtasks, format_result_combined)

### Module Coupling
- **Ouroboros protocol** (`oxios_ouroboros`): `ExecutionResult`, `InterviewResult`, `OuroborosProtocol`, `Phase`, `Seed`
- **AgentLifecycleManager** — delegates all actual execution
- **A2A protocol** — for inter-agent delegation
- **SpaceManager** — for context partitioning and space detection
- **EventBus** — publishes phase lifecycle events
- **StateStore** — persists seeds and agent groups
- **GitLayer** — auto-commits after state saves
- **ConversationBuffer** — tracks multi-turn dialogue
- **A2ACircuitBreaker** — protects delegation reliability
- **Metrics** — tracks orchestration timing

### Error Handling
- Uses `anyhow::Result` throughout.
- `handle_message` returns `Result<OrchestrationResult>` — errors propagate to the caller (channel).
- Git commit failures are silently ignored (`let _ =`).
- Event publish failures are silently ignored (`let _ =`).
- **`#[allow(clippy::await_holding_lock)]`** on `handle_message` — acknowledges lock holding across await points.

### Single Responsibility
⚠️ **Partially clear.** The orchestrator handles the full Ouroboros lifecycle (interview → seed → execute → evaluate), but also:
- Space detection and conversation buffering
- Multi-agent delegation (A2A + lifecycle fallback)
- Seed splitting heuristics
- Session management for multi-turn interviews

This is a complex module but the complexity is bounded and well-structured.

### Architectural Observations
- **Complexity routing:** Simple tasks get an ad-hoc seed (no LLM call), complex tasks get full LLM generation. Good optimization.
- **Multi-agent split heuristic:** Seeds with ≥5 acceptance criteria are split into subtasks. This is a simple heuristic that could be made configurable.
- **Capability inference** is duplicated between `orchestrator.rs` (`split_into_subtasks`) and `agent_lifecycle.rs` (`build_agent_card`) — both do keyword matching for "review", "test", "refactor", "write", "debug".
- **Three delegation paths:** A2A with retry → A2A direct → lifecycle fallback. Clean degradation.
- **Session cleanup** is done after execution but not on error paths in `handle_message` — sessions could leak if an unexpected error occurs between seed creation and execution.

### Smells
1. **`handle_message` is ~250 lines** — the main method is very long. Consider extracting phases into separate methods (already partially done with `delegate_subtasks`).
2. **Capability inference duplication** — keyword matching appears in both `split_into_subtasks` and `AgentLifecycleManager::build_agent_card`. Should be a shared utility.
3. **Lock holding across await** — explicitly acknowledged with `#[allow(clippy::await_holding_lock)]`. The code works around it by extracting data from locks before awaiting, but the annotation suggests some risk.
4. **Session cleanup on error paths** — if `spawn_and_run` fails, the session is removed from `sessions` map, but if an earlier error occurs (e.g., seed generation), the session may remain.
5. **Dead code:** `DelegationConfig::timeout_ms` is marked `#[allow(dead_code)]`.
6. **`InterviewSession` struct** has `#[allow(unused)]` on all fields — suggests the struct may not be fully utilized.

---

## 4. `supervisor.rs` — Agent Lifecycle (Low-Level)

### Public API Surface Size
- **1 public trait:** `Supervisor` (6 methods: fork, exec, run_with_seed, wait, kill, list)
- **2 public types:** `BasicSupervisor`, `NoOpSupervisor`
- **1 private struct:** `AgentHandle` (cancellation token + JoinHandle)

### Module Coupling
- **AgentRuntime** — delegates tool-calling execution
- **EventBus** — publishes agent lifecycle events
- **ResourceMonitor** — tracks active agent count
- **oxios_ouroboros:** `Seed`, `ExecutionResult`

### Error Handling
- Uses `anyhow` bail for "not found" errors.
- Task join errors (aborted/panicked) are caught and converted to `ExecutionResult { success: false }`.
- **Never returns Err from `run_with_seed`** in the happy path — execution failures become `ExecutionResult`.

### Single Responsibility
✅ **Clear single responsibility:** Low-level agent process management (fork, exec, wait, kill). Maps to Unix's init + process table.

### Architectural Observations
- **Trait-based design** — `Supervisor` is a trait, allowing alternative implementations. `NoOpSupervisor` breaks the circular dependency during KernelHandle construction.
- **Cooperative cancellation** — uses `AtomicBool` flag + `JoinHandle::abort()`. Two mechanisms for safety.
- **`run_with_seed` is the workhorse** — it spawns a tokio task, stores the handle, and awaits it. This is where actual execution happens.
- **Status tracking** is in-memory only (`RwLock<HashMap<AgentId, AgentInfo>>`). No persistence — agent state is lost on restart.

### Smells
1. **`NoOpSupervisor` is in the same file** — could be in its own module for clarity.
2. **Event publish errors silently ignored** — `let _ = self.event_bus.publish(...)`. If event bus is full, events are lost.
3. **`update_agent_count()` acquires a read lock** on agents map. If called frequently, this could become a contention point. However, agent count changes are infrequent.
4. **Test helper `make_supervisor` is ~80 lines** — constructing a full KernelHandle for a supervisor test. This suggests the test infrastructure could benefit from a builder or factory.

---

## 5. `agent_lifecycle.rs` — Full Lifecycle Management

### Public API Surface Size
- **1 public struct:** `AgentLifecycleManager` (derives Clone)
- **6 public methods:** `new`, `spawn_and_run`, `terminate`, `reap_zombies`, + 3 private helpers

### Module Coupling
- **Supervisor** (trait object) — for fork/exec/run/kill
- **AgentScheduler** — for task submission and completion
- **AccessManager** — for RBAC permission grants
- **A2AProtocol** — for agent card registration/unregistration
- **EventBus** — for lifecycle events
- **Metrics** — for fork/complete/fail counters

### Error Handling
- Uses `anyhow::{bail, Result}`.
- **Timeout support:** wraps execution in `tokio::time::timeout` when `max_execution_time_secs > 0`.
- **Guaranteed cleanup** — `cleanup_on_failure` is called on error paths, `cleanup` on success paths.
- A2A registration failures are logged but don't prevent execution.

### Single Responsibility
✅ **Clear single responsibility:** Orchestrated agent lifecycle — fork → register A2A → check permissions → schedule → run → cleanup.

### Architectural Observations
- **Cleanly extracted from Orchestrator** — this module handles the "how" of agent execution while the Orchestrator handles the "what and when".
- **6-step lifecycle** (fork → register A2A → permissions → schedule → run → cleanup) is well-documented and easy to follow.
- **`reap_zombies()`** is called by the Orchestrator after execution — periodic cleanup.
- **Permission grants are hardcoded** — the default tool set (`bash`, `read`, `write`, `edit`, `grep`, `find`, `exec`, `ls`) is baked into `ensure_permissions`. Should come from configuration.

### Smells
1. **Hardcoded default tools** in `ensure_permissions` — should be configurable via `OxiosConfig`.
2. **`Clone` derivation** — `AgentLifecycleManager` contains `Arc<dyn Supervisor>`, `Arc<AgentScheduler>`, `Arc<Mutex<AccessManager>>`, `Arc<A2AProtocol>`, `EventBus`. Cloning is cheap (all Arc), but EventBus is not behind an Arc — cloning EventBus clones the channel sender. This works but should be documented.
3. **`build_agent_card` capability inference** duplicates logic from `orchestrator.rs` (same keyword matching).

---

## 6. `scheduler.rs` — Task Scheduling

### Public API Surface Size
- **4 public types:** `Priority` (4 variants), `TaskStatus` (5 variants), `ScheduledTask` (8 fields), `SchedulerStats` (8 fields)
- **1 public struct:** `AgentScheduler` (~13 public methods)
- **Rate limiter** is private
- **Comprehensive test suite:** 20+ unit tests

### Module Coupling
- **BudgetManager** — optional, for budget-aware scheduling
- **No coupling to Supervisor, EventBus, or Orchestrator** — the scheduler is purely a data structure.

### Error Handling
- Uses `anyhow::Result`.
- "Task not found" errors for unknown task IDs.
- Zombie detection returns IDs of reaped tasks (no error for zombie detection itself).

### Single Responsibility
✅ **Excellent single responsibility:** Pure priority-based task queue with rate limiting and zombie detection. No knowledge of agents, seeds, or execution.

### Architectural Observations
- **BinaryHeap-based priority queue** — `ScheduledTask` implements `Ord` with higher-priority-first, same-priority-LIFO ordering. This is unusual (LIFO within priority) but documented.
- **Budget integration** is optional and clean — `Option<Arc<BudgetManager>>`. When set, tasks for budget-exhausted agents are skipped.
- **Rate limiter** is a sliding window implementation — simple and effective.
- **`start_task` drains the entire BinaryHeap** to find a task by ID, then rebuilds it. This is O(n log n) — acceptable for small queues but could be slow with thousands of tasks.

### Smells
1. **`start_task` is O(n log n)** — drains and rebuilds the BinaryHeap to find a task by ID. Consider a secondary index (`HashMap<Uuid, bool>` in queue).
2. **`cancel_task` has the same O(n log n) issue** — drains the entire heap.
3. **`stats()` returns hardcoded zeros** for `completed` and `failed` — `_completed = 0usize` and `_failed = 0usize` suggest these were intended to be tracked but aren't. The variables are assigned but never used.
4. **LIFO within same priority** — BinaryHeap doesn't guarantee FIFO within the same priority level (it's a max-heap, not a stable queue). The test acknowledges this with `descriptions.sort()`. For fairness, consider a secondary sort key.

---

## 7. `agent_runtime.rs` — Tool Calling Loop

### Public API Surface Size
- **2 public types:** `AgentRuntime`, `AgentRuntimeConfig` (12 configuration fields)
- **4 builder methods:** `new`, `with_persona_manager`, `with_config`, `with_tool_retriever`
- **1 public method:** `execute(agent_id, seed) -> Result<ExecutionResult>`
- **Global:** `LLM_CIRCUIT_BREAKER` (OnceLock singleton)

### Module Coupling
- **OxiosEngine** — LLM provider/model resolution
- **KernelHandle** — single path to all kernel services
- **PersonaManager** — system prompt injection
- **ToolRetriever** — semantic capability discovery
- **MemoryManager** — memory recall and blending
- **KnowledgeLens** — knowledge note recall (RFC-003 Phase 3)
- **CSpace resolution** — capability-based tool registration
- **oxios_ouroboros:** `Seed`, `ExecutionResult`
- **oxi_sdk:** `Agent`, `AgentConfig`, `AgentEvent`, `ToolRegistry`, `Provider`, `ProviderResolver`, etc.

### Error Handling
- Uses `anyhow::Result`.
- Provider/model resolution errors propagate.
- Memory recall failures are logged but don't block execution.
- Knowledge recall failures are logged but don't block execution.
- Circuit breaker records success/failure after execution.
- Agent execution errors become `ExecutionResult { success: false }`.

### Single Responsibility
✅ **Clear single responsibility:** Build and run an oxi-sdk Agent for a given Seed. Handles all the wiring (tools, prompts, memories, knowledge).

### Architectural Observations
- **CSpace-driven tool registration** — tools are resolved from the agent's capability space, not hardcoded. This is a sophisticated design that enables role-based tool access.
- **Prompt construction is layered:** base prompt → persona → capabilities XML → kernel manifest → memory blend → knowledge blend. Each layer is optional and additive.
- **Streaming event processing** — uses `run_streaming` with a callback that accumulates state (`ExecuteState` behind `Mutex`).
- **Compaction events** are captured and stored as memories — elegant way to preserve context across turns.
- **Provider resolution** happens through `OxiosEngine` → `AgentBuilder` pipeline, with middleware support (rate limiting, token budget).
- **Token usage tracking** goes to `cost_tracker()` in the observability module.

### Smells
1. **`execute` is ~100 lines** — could benefit from extracting sub-functions for memory recall, knowledge recall, and prompt building.
2. **`run_agent` function is ~120 lines** — handles agent construction, middleware wiring, event processing. Could be split.
3. **Event callback captures many variables** — `memory_for_callback`, `session_id_for_callback`, `model_id_for_callback`, `agent_id_for_callback` are cloned into the closure. This is necessary but verbose.
4. **`build_system_prompt` has a large hardcoded "Execution Protocol" section** (~30 lines of string literals). This should be in a template file or configurable.
5. **Global `LLM_CIRCUIT_BREAKER`** — OnceLock singleton means all AgentRuntime instances share one breaker. This is intentional (shared LLM resilience) but should be documented.

---

## Cross-Cutting Analysis

### Dependency Flow (Simplified)
```
Orchestrator
  ├── AgentLifecycleManager
  │     ├── Supervisor (trait)
  │     │     └── AgentRuntime
  │     │           └── KernelHandle
  │     ├── AgentScheduler
  │     ├── AccessManager
  │     └── A2AProtocol
  ├── SpaceManager
  ├── EventBus
  ├── StateStore
  ├── GitLayer
  └── A2ACircuitBreaker
```

### Error Handling Patterns
| Pattern | Usage | Verdict |
|---------|-------|---------|
| `anyhow::Result` | Universal across all modules | ✅ Consistent |
| `.expect()` | Only in deprecated `from_subsystems` | ⚠️ One panic path |
| `let _ =` (ignore errors) | Event publish, git commit | ✅ Acceptable for best-effort ops |
| `bail!()` | "Not found" errors in Supervisor | ✅ Clean |
| Silent logging | Memory/knowledge recall failures | ✅ Non-blocking |

### Locking Strategy
| Module | Lock Type | Contention Risk |
|--------|-----------|-----------------|
| Orchestrator | `RwLock` on sessions, space_manager, conversation_buffer | Low — brief holds |
| Supervisor | `RwLock` on agents, handles | Low — brief holds |
| Scheduler | `Mutex` on queue, running, rate_limiter, start_times | **Medium** — `start_task`/`cancel_task` drain the heap |
| AgentRuntime | `Mutex` on ExecuteState (event callback) | Low — brief holds |

### Key Architectural Strengths
1. **Trait-based Supervisor** — enables testing (NoOpSupervisor) and alternative implementations
2. **CSpace-driven tool registration** — capabilities are resolved, not hardcoded
3. **Layered prompt construction** — clean, additive, each layer optional
4. **Clean separation:** Orchestrator (what/when) → AgentLifecycleManager (how) → Supervisor (process) → AgentRuntime (execution)
5. **Three-tier agent delegation:** A2A with retry → A2A direct → lifecycle fallback
6. **Budget-aware scheduling** — optional BudgetManager integration in scheduler

### Key Architectural Concerns
1. **Capability inference duplication** — same keyword matching in `orchestrator.rs` and `agent_lifecycle.rs`. Extract to shared utility.
2. **Scheduler O(n log n) for task lookup** — `start_task` and `cancel_task` drain the entire heap.
3. **`stats()` returns zeros for completed/failed** — unfinished tracking.
4. **Hardcoded default tools** in `AgentLifecycleManager::ensure_permissions`.
5. **Very wide public API surface** in `lib.rs` — ~200 re-exported items.
6. **Session cleanup gaps** in Orchestrator error paths.
7. **`handle_message` is ~250 lines** — could benefit from phase extraction methods.

### Module Responsibility Summary
| Module | Responsibility | Lines | Complexity |
|--------|---------------|-------|------------|
| `lib.rs` | API surface declaration | ~250 | Low |
| `kernel_handle/mod.rs` | Facade composition (13 APIs) | ~230 | Low |
| `orchestrator.rs` | Ouroboros lifecycle + delegation | ~650 | **High** |
| `supervisor.rs` | Agent process management | ~330 | Medium |
| `agent_lifecycle.rs` | Full lifecycle orchestration | ~190 | Medium |
| `scheduler.rs` | Priority task queue | ~500 (incl. tests) | Medium |
| `agent_runtime.rs` | Tool-calling agent execution | ~470 | **High** |
