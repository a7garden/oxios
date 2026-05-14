# Oxios Core Infrastructure Analysis Report

**Date:** 2026-05-14  
**Scope:** Main binary, kernel assembly, and core oxios-kernel modules  
**Total Lines Analyzed:** 3,647

---

## Executive Summary

Oxios presents a well-structured Agent OS with clean separation between the binary (assembly/wiring), the kernel library (components), and the gateway (message routing). The codebase follows Rust best practices: builder pattern for kernel assembly, trait-based supervisor abstraction, broadcast-based event bus, and tiered tool registration in the agent runtime. Error handling is consistent (`thiserror` for library, `anyhow` for binary). Documentation quality is high on public APIs. The main gaps are in test coverage (only 2 of 8 files have tests) and a few concurrency anti-patterns in the agent runtime's workspace management.

---

## File-by-File Analysis

### 1. `src/main.rs` — Main Binary Entry Point

| Metric | Value |
|--------|-------|
| **Lines** | 731 |
| **Key Structs** | `Cli`, `Command`, `ConfigAction`, `PkgAction`, `AgentAction`, `GitAction`, `DaemonAction` |
| **Key Functions** | `main()`, `cmd_run_async()`, `cmd_pkg()`, `cmd_config()`, `cmd_status()`, `cmd_serve()`, `activate_channels()` |
| **Tests** | 0 |

**Architectural Patterns:**
- **CLI with clap derive:** Declarative command hierarchy with subcommands covering run, chat, backup, restore, config, pkg, agent, audit, git, budget, daemon, program.
- **Feature-gated channels:** `#[cfg(feature = "web")]`, `#[cfg(feature = "cli")]`, `#[cfg(feature = "telegram")]` — clean compile-time channel selection.
- **Plugin architecture:** `build_channel_plugins()` → `activate_channels()` pattern where each channel is a `Box<dyn ChannelPlugin>` activated from config.
- **Graceful shutdown:** `ctrl_c` handler kills agents, aborts channel tasks, shuts down MCP servers.

**Code Quality:**
- ✅ Good module-level doc comment explaining the binary's role.
- ✅ Clean separation: `ensure_workspace()` handles first-run bootstrap with `DEFAULT_CONFIG` embedded via `include_str!`.
- ✅ Audit logging compensates for `cmd_run` bypassing the Gateway.
- ✅ `Box::leak(Box::new(_guard))` for tracing guard lifetime — pragmatic and documented by the pattern.
- ⚠️ `cmd_config` → `ConfigAction::Set` is stubbed (`bail!("not yet implemented")`). Should be tracked.
- ⚠️ `DaemonAction::Restart` is a `println!` suggesting `pkill` — not a real restart. This is misleading UX.
- ⚠️ The `main()` function is large (~200 lines of match arms). Could benefit from extracting subcommand handlers into a `commands/` module.

**Observations:**
- Default mode (no subcommand) starts the full server via `cmd_serve()`, which initializes MCP servers, default skills/programs, activates channels, starts the guardian daemon, and waits for ctrl+c.
- The `activate_channels()` function builds a plugin map from available plugins and activates only those listed in `config.channels.enabled` — good defensive design with logging for missing channels.

---

### 2. `src/kernel.rs` — Kernel Assembly (Builder Pattern)

| Metric | Value |
|--------|-------|
| **Lines** | 493 |
| **Key Structs** | `Kernel`, `KernelBuilder` |
| **Key Traits** | (none — consumer of kernel traits) |
| **Key Functions** | `Kernel::builder()`, `Kernel::handle()`, `Kernel::execute_prompt()`, `KernelBuilder::build()`, `init_mcp_bridge()` |
| **Tests** | 0 |

**Architectural Patterns:**
- **Builder pattern:** `Kernel::builder().config_path(p).build().await?` — clean, fluent API.
- **Facade pattern:** `KernelHandle` is the primary API surface, cached via `OnceLock`. The `Kernel` struct itself has private fields.
- **Composition over inheritance:** Kernel holds `Arc` references to ~20 independently testable components.
- **Dependency injection:** All kernel components are wired in `KernelBuilder::build()` — no hidden global state.

**Code Quality:**
- ✅ Excellent doc comment: *"This module lives in the binary crate (not oxios-kernel) because it's responsible for assembling kernel components, not providing them."* — clear architectural intent.
- ✅ `KernelHandle` facade is cached once and reused — efficient.
- ✅ `init_mcp_bridge()` reads from both config file AND environment variables (`OXIOS_MCP_*`) — flexible for containerized deployments.
- ✅ A2A delegation handler is registered with a cloned `AgentLifecycleManager`, creating a clean closure.
- ⚠️ `KernelBuilder::build()` is ~200 lines of sequential initialization. This is typical for a composition root but could benefit from sub-functions (e.g., `init_security()`, `init_orchestrator()`).
- ⚠️ Several `#[allow(dead_code)]` annotations on `gateway()` and `run_gateway()` suggest these may be unused or prematurely exposed.

**Component Wiring Order (in `build()`):**
1. Config → 2. EventBus → 3. StateStore → 4. Engine/Provider → 5. OuroborosProtocol → 6. AccessManager → 7. Scheduler → 8. PersonaManager → 9. A2AProtocol → 10. GitLayer → 11. SkillStore/ProgramManager → 12. McpBridge → 13. AgentRuntime → 14. MemoryManager → 15. Supervisor → 16. AgentLifecycleManager → 17. A2A handler → 18. Orchestrator → 19. Gateway → 20. AuthManager → 21. CronScheduler → 22. AuditTrail → 23. BudgetManager → 24. ResourceMonitor

---

### 3. `crates/oxios-kernel/src/supervisor.rs` — Agent Supervisor

| Metric | Value |
|--------|-------|
| **Lines** | 223 |
| **Key Structs** | `BasicSupervisor` |
| **Key Traits** | `Supervisor` (async_trait) |
| **Key Functions** | `fork()`, `exec()`, `run_with_seed()`, `wait()`, `kill()`, `list()` |
| **Tests** | 0 |

**Architectural Patterns:**
- **Trait-based abstraction:** `Supervisor` trait allows swapping implementations (e.g., distributed supervisor later).
- **In-memory agent registry:** `RwLock<HashMap<AgentId, AgentInfo>>` using `parking_lot::RwLock` for efficiency.
- **Event-driven:** State transitions publish `KernelEvent`s to the event bus.
- **Delegation:** Actual execution is delegated to `AgentRuntime::execute()`.

**Code Quality:**
- ✅ Clean `async_trait` interface with Unix-named lifecycle methods (fork/exec/wait/kill).
- ✅ `run_with_seed()` handles both success and failure paths, publishing appropriate events.
- ✅ Resource monitor integration via `set_resource_monitor()` — optional, non-invasive.
- ✅ Good tracing at info level for key lifecycle events.
- ⚠️ **No tests.** This is a critical module — at minimum, unit tests for fork→exec→kill state transitions and kill-on-unknown-agent should exist.
- ⚠️ `kill()` only sets status to `Stopped` — there's no actual task cancellation (no `CancellationToken` or `JoinHandle::abort()`). If an agent is running a long computation, `kill()` won't stop it.
- ⚠️ `publish()` results are discarded with `let _ =` — if the event bus is full, events are silently lost.

**State Machine:**
```
Starting → Running → Idle
                   → Failed
         → Stopped (via kill)
```

---

### 4. `crates/oxios-kernel/src/event_bus.rs` — Event Bus

| Metric | Value |
|--------|-------|
| **Lines** | 262 |
| **Key Structs** | `EventBus` |
| **Key Enums** | `KernelEvent` (17 variants) |
| **Key Functions** | `EventBus::new()`, `subscribe()`, `publish()`, `attach_audit_trail()`, `kernel_event_to_audit_action()` |
| **Tests** | 0 |

**Architectural Patterns:**
- **Broadcast channel:** `tokio::sync::broadcast` — all subscribers receive all events. Late subscribers miss historical events.
- **Event-sourced audit trail:** `attach_audit_trail()` spawns a background task that forwards all events to the audit trail.
- **Central event taxonomy:** `KernelEvent` enum covers the full lifecycle: agent CRUD, messages, seeds, evaluations, Ouroboros phases, approvals, memory, and agent groups.

**Code Quality:**
- ✅ Thorough doc comments on all `KernelEvent` variants.
- ✅ `kernel_event_to_audit_action()` maps events to structured audit actions (not just `Other`).
- ✅ `EventBus` is `Clone` — can be passed around cheaply.
- ✅ `publish()` silently succeeds when no subscribers — correct behavior for fire-and-forget events.
- ⚠️ **No tests.** Should have tests for publish/subscribe, event ordering, and audit trail forwarding.
- ⚠️ `publish()` returns `Result<()>` but always returns `Ok(())` — the `Result` return type is misleading since the `send()` result is discarded. Should either propagate lagged-receiver errors or return `()`.
- ⚠️ The `extract_agent_id()` function returns `"system"` for events without an agent ID. This is a reasonable default but should be documented.

**Event Categories:**
| Category | Variants |
|----------|----------|
| Agent lifecycle | `AgentCreated`, `AgentStarted`, `AgentStopped`, `AgentFailed` |
| Messaging | `MessageReceived`, `AgentOutput` |
| Ouroboros | `SeedCreated`, `EvaluationComplete`, `PhaseStarted`, `PhaseCompleted` |
| Human-in-the-loop | `ApprovalRequested`, `ApprovalResolved` |
| Memory | `MemoryStored`, `MemoryRecalled` |
| Agent groups | `AgentGroupCreated`, `AgentGroupMemberCompleted` |

---

### 5. `crates/oxios-kernel/src/agent_runtime.rs` — Agent Runtime

| Metric | Value |
|--------|-------|
| **Lines** | 810 |
| **Key Structs** | `AgentRuntime`, `AgentRuntimeConfig`, `ExecuteState` |
| **Key Functions** | `AgentRuntime::execute()`, `run_agent_loop()`, `build_system_prompt()`, `build_user_prompt()` |
| **Tests** | 4 (`test_requires_tools_validation_passes`, `test_requires_tools_validation_fails`, `test_build_system_prompt_includes_skills`, `test_build_system_prompt_empty_skills`) |

**Architectural Patterns:**
- **Builder-style configuration:** `AgentRuntime::new().with_program_manager().with_mcp_bridge()...` — fluent API.
- **Tiered tool registration:** 7 tiers of tools registered into the `ToolRegistry`:
  1. Tier 1: oxi native tools (Read, Write, Edit, Grep, Find, Ls, WebSearch)
  2. ExecTool (per-agent instance)
  3. Tier 3: Program tools (dynamic, from installed programs)
  4. Tier 4: MCP server tools (from pre-configured bridge)
  5. Tier 5: Memory tools (write, read, search)
  6. Tier 6: A2A inter-agent tools (delegate, send, query)
  7. Tier 7: Browser tool (feature-gated)
- **spawn_blocking bridge:** `AgentLoop::run()` produces a `!Send` future, so execution happens in `spawn_blocking` with `Handle::block_on`.
- **Circuit breaker:** Global `LLM_CIRCUIT_BREAKER` (OnceLock) tracks LLM call success/failure rates.

**Code Quality:**
- ✅ Comprehensive system prompt builder with goal, constraints, acceptance criteria, ontology, skill injection, persona, and execution environment guidance.
- ✅ Per-agent workspace isolation under `/tmp/oxios-agent-workspace/<agent_id>/`.
- ✅ Program dependency validation (`requires_tools`) — programs are skipped if their required tools aren't registered.
- ✅ Compaction events are captured as conversation memory entries.
- ✅ Tests cover the tool validation and system prompt building logic.
- ⚠️ **Critical: CWD race condition.** The code explicitly acknowledges this: `std::env::set_current_dir()` is process-global. Concurrent agents in separate `spawn_blocking` threads WILL race. The TODO to add `workspace_dir` to `AgentLoopConfig` in oxi-agent is the correct fix.
- ⚠️ `run_agent_loop()` has `#[allow(clippy::too_many_arguments)]` with 13 parameters. This should be refactored into a struct (e.g., `AgentLoopContext`).
- ⚠️ Nested `block_on` calls inside `spawn_blocking`: `rt.block_on(async { ... })` is used to collect program skill contents, list programs, and initialize MCP. This creates a nested runtime context which can panic if not careful.
- ⚠️ The `ExecuteState` pattern (Arc<Mutex<>> shared between callback and main flow) is functional but fragile — if the callback contract changes, state can become inconsistent.

---

### 6. `crates/oxios-kernel/src/config.rs` — Configuration

| Metric | Value |
|--------|-------|
| **Lines** | 914 |
| **Key Structs** | `OxiosConfig`, `KernelConfig`, `GatewayConfig`, `SchedulerConfig`, `ContextConfig`, `SecurityConfig`, `PersonaConfig`, `MemoryConfig`, `CronConfig`, `McpConfig`, `GitConfig`, `AuditConfig`, `BudgetConfig`, `ExecConfig`, `ResourceMonitorConfig`, `OtelConfig`, `ChannelsConfig`, `BrowserConfig`, `InlineCronJob`, `McpServerDef`, `TelegramChannelConfig` |
| **Key Functions** | `load_config()`, `expand_home()`, `OxiosConfig::validate()`, `ExecConfig::is_binary_allowed()` |
| **Tests** | 0 |

**Architectural Patterns:**
- **Layered configuration:** TOML file → `OxiosConfig` struct → validation → consumers.
- **Serde derive:** All config structs implement `Serialize + Deserialize` with `#[serde(default)]` for backward compatibility.
- **Validation with errors/warnings:** `validate()` returns `(Vec<String>, Vec<String>)` — separates hard errors from soft warnings.
- **Default functions:** Each field uses a named default function (e.g., `default_gateway_port`) for clarity.

**Code Quality:**
- ✅ 22 config structs covering every subsystem — comprehensive.
- ✅ Validation covers cross-field checks (e.g., `default_timeout_secs > max_timeout_secs`).
- ✅ Cron expressions are validated using the `cron` crate, with 5-field → 6-field normalization.
- ✅ `expand_home()` utility handles `~/` expansion.
- ⚠️ **No tests.** Config parsing, default values, validation, and `expand_home()` are all eminently testable and critical.
- ⚠️ `default_true()` is defined twice (once in config.rs scope, once in a different module's scope). The function naming is generic and could collide.
- ⚠️ `ExecConfig::is_binary_allowed()` has a permissive default (empty `allowed_commands` = all allowed). This should be clearly documented as "development mode" with a warning.
- ⚠️ `BrowserConfig::default()` has `enabled: true` — browser enabled by default may surprise users who don't expect headless browser usage.

---

### 7. `crates/oxios-kernel/src/types.rs` — Core Types

| Metric | Value |
|--------|-------|
| **Lines** | 51 |
| **Key Types** | `AgentId` (type alias for `uuid::Uuid`), `AgentStatus` (enum), `AgentInfo` (struct) |
| **Tests** | 0 |

**Architectural Patterns:**
- **Type alias for identity:** `AgentId = uuid::Uuid` — clean, swappable.
- **Status enum with Display impl:** Machine-readable string representation.

**Code Quality:**
- ✅ Minimal, focused file — exactly what a types module should be.
- ✅ All fields documented.
- ✅ `AgentStatus` derives `PartialEq` and `Eq` for comparisons.
- ⚠️ **No tests** (though `Display` impl could use a trivial test).
- ⚠️ `AgentInfo` uses `DateTime<Utc>` for `created_at` — good, but no `updated_at` field for tracking status changes.

---

### 8. `crates/oxios-kernel/src/error.rs` — Error Handling

| Metric | Value |
|--------|-------|
| **Lines** | 163 |
| **Key Types** | `KernelError` (enum, 9 variants), `HttpStatus` (enum), `KernelResult<T>` |
| **Key Functions** | `KernelError::http_status()` |
| **Tests** | 4 (`test_error_display`, `test_all_http_status_mappings`, `test_internal_error_wrapping`, `test_io_error_conversion`) |

**Architectural Patterns:**
- **thiserror derive:** Clean error definitions with `#[from]` for automatic conversion.
- **HTTP status mapping:** `KernelError → HttpStatus → u16` — framework-agnostic error → HTTP translation.
- **Error/warning hierarchy:** `Internal` variant wraps `anyhow::Error` for implementation errors; typed variants for API errors.

**Code Quality:**
- ✅ Well-documented variants with field-level doc comments.
- ✅ HTTP status mapping is comprehensive and sensible (404 for not-found, 409 for already-exists, 403 for permission denied, etc.).
- ✅ `KernelResult<T>` alias for convenience.
- ✅ Good test coverage: all HTTP mappings tested, error display tested, `From` conversions tested.
- ⚠️ `Memory` variant uses a `String` reason instead of wrapping a typed memory error — loses structured error info.
- ⚠️ No `Timeout` or `RateLimited` error variants, which would be useful for budget/scheduler enforcement.

---

## Cross-Cutting Analysis

### Architecture Patterns

| Pattern | Where Used |
|---------|-----------|
| **Builder** | `KernelBuilder`, `AgentRuntime` (with_* methods) |
| **Facade** | `KernelHandle` wraps 7 sub-APIs (`StateApi`, `AgentApi`, `SecurityApi`, etc.) |
| **Strategy** | `Supervisor` trait, `OuroborosProtocol` trait |
| **Observer/Event** | `EventBus` with broadcast channel |
| **Plugin** | `ChannelPlugin` trait for channels |
| **Tiered Registry** | 7-tier tool registration in `AgentRuntime` |
| **Circuit Breaker** | Global `LLM_CIRCUIT_BREAKER` for LLM fault tolerance |

### Test Coverage Summary

| File | Lines | # Tests | Coverage |
|------|-------|---------|----------|
| `main.rs` | 731 | 0 | ❌ None |
| `kernel.rs` | 493 | 0 | ❌ None |
| `supervisor.rs` | 223 | 0 | ❌ None |
| `event_bus.rs` | 262 | 0 | ❌ None |
| `agent_runtime.rs` | 810 | 4 | ⚠️ Partial (prompt building, tool validation) |
| `config.rs` | 914 | 0 | ❌ None |
| `types.rs` | 51 | 0 | ❌ None (trivially testable) |
| `error.rs` | 163 | 4 | ✅ Good |
| **Total** | **3,647** | **8** | **Low** |

### Key Issues and Risks

| Severity | Issue | Location |
|----------|-------|----------|
| 🔴 **High** | CWD race condition with concurrent agents | `agent_runtime.rs` L435 |
| 🟡 **Medium** | `kill()` doesn't cancel running tasks | `supervisor.rs` L167 |
| 🟡 **Medium** | No test coverage on supervisor, event bus, config | Multiple files |
| 🟡 **Medium** | `run_agent_loop()` takes 13 parameters | `agent_runtime.rs` L395 |
| 🟢 **Low** | `publish()` returns misleading `Result<()>` | `event_bus.rs` L189 |
| 🟢 **Low** | `ConfigAction::Set` not implemented | `main.rs` L265 |
| 🟢 **Low** | `DaemonAction::Restart` is a no-op | `main.rs` L410 |

### Strengths

1. **Clean architectural boundaries** — binary assembles, kernel provides, gateway routes.
2. **Consistent error handling** — `thiserror` for library, `anyhow` for binary.
3. **Good documentation** — module-level doc comments explain intent and rationale.
4. **Feature-gated compilation** — channels, browser, OTel are opt-in.
5. **Defensive config validation** — separates errors from warnings, validates cron expressions.
6. **Audit-first design** — all events flow to audit trail, guardian daemon verifies integrity.
7. **Memory integration** — compaction summaries captured as memories, memories blended into system prompts.

### Recommended Improvements

1. **Add `workspace_dir` to oxi-agent's `AgentLoopConfig`** — eliminates the CWD race condition.
2. **Refactor `run_agent_loop()` parameters** into an `AgentLoopContext` struct.
3. **Add `CancellationToken` support** to the supervisor for real task cancellation on `kill()`.
4. **Add unit tests** for supervisor state machine, event bus publish/subscribe, config parsing/validation.
5. **Extract subcommand handlers** from `main()` into a `commands/` module.
6. **Add `Timeout` and `RateLimited` variants** to `KernelError`.
7. **Consider making `publish()` return `()`** or properly handle broadcast lag errors.
