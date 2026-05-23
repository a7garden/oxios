# Oxios-Kernel Root Module Analysis

> Generated: 2026-05-23  
> Scope: `crates/oxios-kernel/src/` — 17 root-level `.rs` files  
> Crate version: 0.2.0

---

## 1. File-by-File Analysis

### 1.1 `orchestrator.rs` (46.9 KB — largest file)

**Purpose:** Coordinates the full Ouroboros lifecycle: Interview → Seed → Execute → Evaluate → Evolve.

| Category | Details |
|----------|---------|
| **Exported types** | `AgentRole` (enum: Worker, Manager), `SubTask` (struct), `Orchestrator` (struct), `OrchestrationResult` (struct) |
| **Internal deps** | `agent_lifecycle`, `event_bus`, `git_layer`, `metrics`, `scheduler`, `space`, `state_store`, `types`, `a2a` (in methods), `a2a_circuit_breaker` (in struct), `agent_group` (in method), `config` (in struct) |
| **External deps** | `anyhow`, `chrono`, `oxios_ouroboros`, `parking_lot`, `serde`, `uuid`, `tokio` |
| **Internal state** | **High.** `Arc<dyn OuroborosProtocol>`, `EventBus`, `Arc<StateStore>`, `Option<Arc<GitLayer>>`, `RwLock<HashMap<String, InterviewSession>>`, `AgentLifecycleManager`, `Option<Arc<A2AProtocol>>`, `RwLock<Option<Arc<SpaceManager>>>`, `RwLock<ConversationBuffer>`, `OrchestratorConfig`, `DelegationConfig`, `Arc<A2ACircuitBreaker>` |

**Structural notes:** This is the most complex module — it orchestrates the entire agent lifecycle. Contains substantial multi-agent delegation logic (A2A-based and lifecycle-based fallback) with retry/circuit-breaker support.

---

### 1.2 `scheduler.rs` (35.6 KB)

**Purpose:** Priority-based task queue with rate-limiting, zombie detection, and max concurrent enforcement.

| Category | Details |
|----------|---------|
| **Exported types** | `Priority` (enum: Low/Normal/High/Critical), `TaskStatus` (enum: Queued/Running/Completed/Failed/Cancelled), `ScheduledTask` (struct), `SchedulerStats` (struct), `AgentScheduler` (struct) |
| **Internal deps** | `budget`, `types` |
| **External deps** | `anyhow`, `chrono`, `parking_lot`, `serde`, `uuid`, `std::collections` |
| **Internal state** | **High.** `Arc<Mutex<BinaryHeap<ScheduledTask>>>`, `Arc<Mutex<HashMap<Uuid, ScheduledTask>>>`, `Arc<Mutex<RateLimiter>>`, `Arc<Mutex<HashMap<Uuid, DateTime>>>`, `Option<Arc<BudgetManager>>` |

**Structural notes:** Pure in-memory scheduling with no async I/O. Budget integration is optional (soft gate). Rate limiter uses sliding window.

---

### 1.3 `supervisor.rs` (20.5 KB)

**Purpose:** Agent lifecycle management — fork, exec, wait, kill. The "init" of Oxios.

| Category | Details |
|----------|---------|
| **Exported types** | `Supervisor` (trait: fork/exec/wait/kill/list), `BasicSupervisor` (struct), `NoOpSupervisor` (struct) |
| **Internal deps** | `agent_runtime`, `event_bus`, `resource_monitor`, `types` |
| **External deps** | `anyhow`, `async_trait`, `chrono`, `oxios_ouroboros`, `parking_lot`, `tokio` |
| **Internal state** | **High.** `RwLock<HashMap<AgentId, AgentInfo>>`, `RwLock<HashMap<AgentId, AgentHandle>>`, `EventBus`, `Arc<AgentRuntime>`, `Option<Arc<ResourceMonitor>>`. Each `AgentHandle` holds `Arc<AtomicBool>` (cancel token) + `JoinHandle<Result<ExecutionResult>>`. |

**Structural notes:** `NoOpSupervisor` breaks the KernelHandle → AgentRuntime → Supervisor cycle during build. Uses cooperative cancellation (AtomicBool) + task abortion.

---

### 1.4 `agent_runtime.rs` (27.8 KB)

**Purpose:** Wraps oxi-agent's `AgentLoop` for tool-calling execution. Resolves CSpace, registers tools, runs the LLM loop.

| Category | Details |
|----------|---------|
| **Exported types** | `AgentRuntimeConfig` (struct), `AgentRuntime` (struct) |
| **Internal deps** | `capability::resolve`, `circuit_breaker`, `memory`, `persona_manager`, `tools::registration`, `types`, `KernelHandle`, `tools::retrieval` (in methods), `tools::ExecTool` (in methods), `tools::McpToolWrapper` (in methods) |
| **External deps** | `anyhow`, `oxi_sdk` (AgentLoop, ToolRegistry, Provider, etc.), `parking_lot`, `oxios_ouroboros`, `tokio` |
| **Internal state** | **High.** `Arc<dyn Provider>`, `AgentRuntimeConfig`, `Arc<KernelHandle>`, `Option<Arc<PersonaManager>>`, `Option<Arc<ToolRetriever>>`. Static `OnceLock<CircuitBreaker>` for global LLM circuit breaker. `Arc<Mutex<ExecuteState>>` per execution. |

**Structural notes:** This is the bridge between the kernel's capability system (CSpace) and oxi-sdk's tool-calling loop. Handles memory recall, knowledge blending, program tool loading, and MCP tool registration.

---

### 1.5 `agent_lifecycle.rs` (8.3 KB)

**Purpose:** Full agent lifecycle — fork → register A2A → check permissions → schedule → run → unregister → cleanup.

| Category | Details |
|----------|---------|
| **Exported types** | `AgentLifecycleManager` (struct, `#[derive(Clone)]`) |
| **Internal deps** | `a2a`, `access_manager`, `event_bus`, `metrics`, `scheduler`, `supervisor`, `types` |
| **External deps** | `anyhow`, `tokio`, `oxios_ouroboros` |
| **Internal state** | **Medium.** `Arc<dyn Supervisor>`, `Arc<AgentScheduler>`, `Arc<parking_lot::Mutex<AccessManager>>`, `Arc<A2AProtocol>`, `EventBus`, `max_execution_time_secs: u64` |

**Structural notes:** Extracted from Orchestrator to reduce its scope. Uses `tokio::time::timeout` for execution deadlines. Clonable — fields are all `Arc`-wrapped.

---

### 1.6 `engine.rs` (7.3 KB)

**Purpose:** Thin wrapper around oxi-sdk's `Oxi` for provider/model resolution.

| Category | Details |
|----------|---------|
| **Exported types** | `OxiosEngine` (struct), `EngineProvider` (trait), `OxiEngineProvider` (struct) |
| **Internal deps** | `credential` (in `register_compatible_providers`) |
| **External deps** | `anyhow`, `oxi_sdk` |
| **Internal state** | **Low.** `OxiosEngine` holds `Oxi` + `default_model_id: String`. `OxiEngineProvider` wraps `OxiosEngine`. |

**Structural notes:** Factory pattern for OpenAI-compatible providers with lazy credential resolution via `CredentialStore`. No async methods.

---

### 1.7 `state_store.rs` (18.2 KB)

**Purpose:** Filesystem-based persistent state — JSON/Markdown files organized by category.

| Category | Details |
|----------|---------|
| **Exported types** | `SessionId` (struct), `UserMessage` (struct), `AgentResponse` (struct), `SessionMetadata` (type alias), `Session` (struct), `StateStore` (struct), `SessionSummary` (struct) |
| **Internal deps** | None |
| **External deps** | `anyhow`, `chrono`, `serde`, `serde_json`, `tokio` |
| **Internal state** | **Minimal.** `StateStore` holds only `base_path: PathBuf`. Stateless — all operations are file I/O. |

**Structural notes:** Leaf module. Atomic writes via temp file + rename pattern. Path traversal validation on category/name inputs.

---

### 1.8 `event_bus.rs` (11.7 KB)

**Purpose:** Broadcast-based inter-agent communication via tokio channels.

| Category | Details |
|----------|---------|
| **Exported types** | `KernelEvent` (enum — 20 variants), `EventBus` (struct) |
| **Internal deps** | `audit_trail`, `types` |
| **External deps** | `anyhow`, `serde`, `tokio`, `oxios_ouroboros` (for Phase in event variants) |
| **Internal state** | **Low.** `broadcast::Sender<KernelEvent>`. Clone-able sender; receivers are created on subscribe. |

**Structural notes:** `KernelEvent` is the central event type — 20 variants covering agent lifecycle, seeds, evaluations, spaces, approvals, memory, and groups. `attach_audit_trail()` spawns a background task that forwards all events to the audit log.

---

### 1.9 `config.rs` (35.5 KB)

**Purpose:** Configuration loading from TOML files. 20+ config structs.

| Category | Details |
|----------|---------|
| **Exported types** | `CronConfig`, `InlineCronJob`, `MemoryConfig`, `ChannelsConfig`, `TelegramChannelConfig`, `EngineConfig`, `DaemonConfig`, `OxiosConfig`, `KernelConfig`, `GatewayConfig`, `ExecMode`, `ExecConfig`, `SchedulerConfig`, `OrchestratorConfig`, `ContextConfig`, `SecurityConfig`, `PersonaConfig`, `McpConfig`, `McpServerDef`, `GitConfig`, `BrowserConfig`, `LoggingConfig` |
| **Internal deps** | `scheduler::Priority` |
| **External deps** | `cron`, `serde`, `std` |
| **Internal state** | **None.** Pure data structs with serde derive. |

**Structural notes:** Second largest file. All config structs are `Serialize + Deserialize`. `OxiosConfig` is the top-level config that nests all sub-configs. Includes `expand_home()` helper for `~` paths.

---

### 1.10 `daemon.rs` (10.3 KB)

**Purpose:** Daemon lifecycle management — PID file, start/stop, system service install (launchd/systemd).

| Category | Details |
|----------|---------|
| **Exported types** | `DaemonStatus` (enum: Running/Stale/Stopped), `DaemonManager` (struct) |
| **Internal deps** | `config` (for `expand_home`) |
| **External deps** | `anyhow` |
| **Internal state** | **Minimal.** `pid_file: PathBuf`, `log_dir: PathBuf`. |

**Structural notes:** Platform-specific — generates launchd plist (macOS) or systemd unit (Linux). No Arc/Mutex.

---

### 1.11 `budget.rs` (19.0 KB)

**Purpose:** Agent-level token and call budget tracking with sliding window.

| Category | Details |
|----------|---------|
| **Exported types** | `BudgetLimit` (struct), `Usage` (struct), `BudgetInfo` (struct), `BudgetKind` (enum: Token/Call), `BudgetExceeded` (struct), `BudgetManager` (struct) |
| **Internal deps** | `types::AgentId` |
| **External deps** | `chrono`, `parking_lot`, `serde`, `std` |
| **Internal state** | **Medium.** `RwLock<HashMap<AgentId, BudgetEntry>>`. Filesystem persistence optional. |

**Structural notes:** Near-leaf module. Sliding window resets after configurable duration. Persistable to disk via JSON.

---

### 1.12 `circuit_breaker.rs` (9.0 KB)

**Purpose:** 3-state circuit breaker (Closed → Open → Half-Open) for LLM provider fault tolerance.

| Category | Details |
|----------|---------|
| **Exported types** | `CircuitBreaker` (struct) |
| **Internal deps** | `metrics` (in record_success/record_failure) |
| **External deps** | `std::sync::atomic` (AtomicU32, AtomicU64, AtomicBool) |
| **Internal state** | **High (lock-free).** 4 atomics: `state`, `failure_count`, `last_failure_ts`, `half_open_probe_sent`. No locks — fully lock-free implementation. |

**Structural notes:** Leaf module. Pure lock-free atomic operations. Only internal dep is `metrics` for gauge updates. Used by `agent_runtime` (global LLM breaker) and `orchestrator` (A2A delegation breaker).

---

### 1.13 `auth.rs` (8.2 KB)

**Purpose:** API key authentication manager — SHA-256 hashed key storage.

| Category | Details |
|----------|---------|
| **Exported types** | `KeyMeta` (struct), `AuthManager` (struct) |
| **Internal deps** | None |
| **External deps** | `anyhow`, `serde`, `sha2`, `std` |
| **Internal state** | **Medium.** `HashMap<String, KeyMeta>`, `HashSet<String>`, `Option<PathBuf>`. No thread-safe wrappers — caller wraps in Mutex. |

**Structural notes:** Leaf module. Keys stored as SHA-256 hashes. Supports persistence to JSON file.

---

### 1.14 `credential.rs` (4.8 KB)

**Purpose:** Multi-source credential resolution (env → config → oxi auth.json → oxi-ai fallback).

| Category | Details |
|----------|---------|
| **Exported types** | `CredentialSource` (enum: Config/OxiAuthStore/EnvVar), `CredentialStore` (unit struct) |
| **Internal deps** | None |
| **External deps** | `anyhow`, `oxi_sdk`, `chrono` |
| **Internal state** | **None.** `CredentialStore` is a unit struct — all methods are associated functions. |

**Structural notes:** Leaf module. Stateless. Uses `oxi_sdk::load_token` and `oxi_sdk::get_env_api_key`.

---

### 1.15 `error.rs` (8.8 KB)

**Purpose:** Typed error types for the kernel public API.

| Category | Details |
|----------|---------|
| **Exported types** | `KernelError` (enum — 12 variants), `HttpStatus` (enum), `KernelResult<T>` (type alias), `ErrorCategory` (enum) |
| **Internal deps** | `types::AgentId` (in AgentNotFound variant) |
| **External deps** | `thiserror` |
| **Internal state** | **None.** Pure enum definitions. |

**Structural notes:** Leaf module. Provides `http_status()`, `category()`, and `is_retryable()` methods on `KernelError`.

---

### 1.16 `types.rs` (1.5 KB — smallest file)

**Purpose:** Core type aliases and basic data types.

| Category | Details |
|----------|---------|
| **Exported types** | `AgentId` (type alias = `uuid::Uuid`), `AgentStatus` (enum: Starting/Running/Idle/Stopped/Failed), `AgentInfo` (struct) |
| **Internal deps** | None |
| **External deps** | `chrono`, `serde`, `uuid` |
| **Internal state** | **None.** Pure data. |

**Structural notes:** Leaf module. Foundational type used by nearly every other module.

---

### 1.17 `lib.rs` (10.3 KB)

**Purpose:** Crate root — module declarations, sectioned re-exports, oxi-sdk re-exports.

| Category | Details |
|----------|---------|
| **Module sections** | Lifecycle (6 modules), Orchestration (5), Security (5), Communication (3), Intelligence (6), Tools & Programs (4+1), State & Config (6), Infrastructure (4+2 conditional), API Surface (1) |
| **Total modules** | 38 public modules (40 with feature-gated wasm-sandbox + telemetry variants) |
| **Features** | `default = ["browser"]`, `otel` (OpenTelemetry), `wasm-sandbox` (WASM execution), `browser` |
| **Re-exports** | ~100+ public items across all sections + oxi-sdk types |

---

## 2. Dependency Matrix

### Internal Dependencies (`crate::` references)

Rows depend on columns. `●` = direct import.

| Module ↓ \ → | types | event_bus | state_store | config | error | budget | scheduler | supervisor | agent_runtime | agent_lifecycle | orchestrator | engine | circuit_breaker | memory | credential | auth | audit_trail | a2a | access_manager | git_layer | metrics | resource_monitor | persona_manager | capability | tools | space | agent_group | a2a_circuit_breaker |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **orchestrator** | ● | ● | ● | ● | | | ● | | | ● | | | | | | | | ● | | ● | ● | | | | | ● | ● | ● |
| **scheduler** | ● | | | | | ● | | | | | | | | | | | | | | | | | | | | | | |
| **supervisor** | ● | ● | | | | | | | ● | | | | | | | | | | | | | ● | | | | | | |
| **agent_runtime** | ● | | | | | | | | | | | ● | ● | | | | | | | | | | ● | ● | ● | | | |
| **agent_lifecycle** | ● | ● | | | | | ● | ● | | | | | | | | | | ● | ● | | ● | | | | | | | |
| **engine** | | | | | | | | | | | | | | | ● | | | | | | | | | | | | | |
| **state_store** | | | | | | | | | | | | | | | | | | | | | | | | | | | | |
| **event_bus** | ● | | | | | | | | | | | | | | | | ● | | | | | | | | | | | |
| **config** | | | | | | | ● | | | | | | | | | | | | | | | | | | | | | |
| **daemon** | | | | ● | | | | | | | | | | | | | | | | | | | | | | | | |
| **budget** | ● | | | | | | | | | | | | | | | | | | | | | | | | | | | |
| **circuit_breaker** | | | | | | | | | | | | | | | | | | | | | ● | | | | | | | |
| **auth** | | | | | | | | | | | | | | | | | | | | | | | | | | | | |
| **credential** | | | | | | | | | | | | | | | | | | | | | | | | | | | | |
| **error** | ● | | | | | | | | | | | | | | | | | | | | | | | | | | |
| **types** | | | | | | | | | | | | | | | | | | | | | | | | | | | | |

### External Dependencies per File

| Module | anyhow | serde | tokio | chrono | parking_lot | oxi_sdk | oxi_ai | oxios_ouroboros | thiserror | sha2 | async_trait | uuid | cron |
|--------|:------:|:-----:|:-----:|:------:|:-----------:|:-------:|:------:|:----------------:|:---------:|:----:|:-----------:|:----:|:----:|
| **orchestrator** | ● | ● | ● | ● | ● | | | ● | | | | ● | |
| **scheduler** | ● | ● | | ● | ● | | | | | | | ● | |
| **supervisor** | ● | | ● | ● | ● | | | ● | | | ● | | |
| **agent_runtime** | ● | | ● | | ● | ● | | ● | | | | | |
| **agent_lifecycle** | ● | | ● | | | | | ● | | | | | |
| **engine** | ● | | | | | ● | ● | | | | | | |
| **state_store** | ● | ● | ● | ● | | | | | | | | | |
| **event_bus** | ● | ● | ● | | | | | ● | | | | | |
| **config** | | ● | | | | | | | | | | | ● |
| **daemon** | ● | | | | | | | | | | | | |
| **budget** | | ● | | ● | ● | | | | | | | | |
| **circuit_breaker** | | | | | | | | | | | | | |
| **auth** | ● | ● | | | | | | | | ● | | | |
| **credential** | ● | | | | | ● | | | | | | | |
| **error** | | | | | | | | | ● | | | | |
| **types** | | ● | | ● | | | | | | | | | |

---

## 3. Module Classification

### Leaf Modules (no internal `crate::` dependencies)

These modules are self-contained and import only external crates:

| Module | Exported Types | Notes |
|--------|---------------|-------|
| **types.rs** | `AgentId`, `AgentStatus`, `AgentInfo` | Foundational — used by ~12 other modules |
| **error.rs** | `KernelError`, `HttpStatus`, `KernelResult`, `ErrorCategory` | Error surface — depends only on `types` (for AgentId in one variant) |
| **state_store.rs** | `StateStore`, `Session`, `SessionId`, `AgentResponse`, `SessionSummary` | Filesystem I/O only — no kernel deps |
| **auth.rs** | `AuthManager`, `KeyMeta` | SHA-256 key management — no kernel deps |
| **credential.rs** | `CredentialStore`, `CredentialSource` | Stateless unit struct — pure oxi_sdk integration |
| **circuit_breaker.rs** | `CircuitBreaker` | Lock-free atomics — only `metrics` dep for gauge updates |
| **config.rs** | 20+ config structs | Pure serde data — only `scheduler::Priority` dep |
| **daemon.rs** | `DaemonManager`, `DaemonStatus` | PID/service management — only `config::expand_home` dep |

**Note:** `circuit_breaker`, `config`, and `daemon` each have exactly 1 trivial internal dep (metrics, Priority, expand_home respectively) — effectively leaf modules.

### Hub Modules (many internal dependencies)

These are the central coordination modules:

| Module | # Internal Deps | Role |
|--------|:---------------:|------|
| **orchestrator** | **12** | Central brain — depends on nearly every subsystem |
| **agent_runtime** | **6** | Tool bridge — connects kernel services to LLM loop |
| **agent_lifecycle** | **7** | Lifecycle coordinator — chains supervisor → scheduler → A2A → access |
| **supervisor** | **4** | Process manager — wraps agent_runtime + event_bus |
| **scheduler** | **2** | Task queue — depends on budget + types |
| **event_bus** | **2** | Message hub — depends on audit_trail + types |
| **engine** | **1** | Provider factory — depends on credential |

---

## 4. Key Architectural Observations

### 4.1 Layered Architecture

```
Layer 4: orchestrator.rs                    (12 deps — the brain)
Layer 3: agent_lifecycle.rs (7), agent_runtime.rs (6)  (lifecycle + execution)
Layer 2: supervisor.rs (4), scheduler.rs (2)           (process + queue mgmt)
Layer 1: event_bus.rs (2), engine.rs (1)               (communication + AI)
Layer 0: types, error, state_store, auth, credential,  (foundation)
         circuit_breaker, config, daemon, budget
```

### 4.2 Dependency Flow

```
orchestrator
├── agent_lifecycle
│   ├── supervisor
│   │   ├── agent_runtime → KernelHandle → tools, capability, memory
│   │   └── event_bus → audit_trail
│   ├── scheduler → budget → types
│   ├── a2a → event_bus
│   └── access_manager
├── state_store (leaf)
├── space → state_store
├── git_layer (leaf)
├── a2a_circuit_breaker → circuit_breaker
└── metrics
```

### 4.3 Shared Mutable State

| Pattern | Modules Using |
|---------|--------------|
| `Arc<Mutex<T>>` (parking_lot) | scheduler, agent_lifecycle |
| `Arc<RwLock<T>>` (parking_lot) | orchestrator, supervisor, budget |
| `Arc<Atomic*>` (lock-free) | circuit_breaker, supervisor (cancel tokens) |
| `broadcast::Sender` (tokio) | event_bus |
| `OnceLock` (static) | agent_runtime (global circuit breaker) |

### 4.4 Cycle-Breaking Patterns

The kernel uses two patterns to avoid dependency cycles:

1. **Trait-based indirection**: `Supervisor` trait allows `NoOpSupervisor` during `KernelHandle` construction, breaking the `KernelHandle → AgentRuntime → Supervisor → KernelHandle` cycle.
2. **Late binding via setters**: `Orchestrator` sets `SpaceManager`, `A2AProtocol`, and `GitLayer` after construction via `set_*()` methods.

### 4.5 Module Sizes

| Size | Modules |
|------|---------|
| >40 KB | `orchestrator.rs` (46.9 KB), `config.rs` (35.5 KB) |
| 20-40 KB | `scheduler.rs` (35.6 KB), `agent_runtime.rs` (27.8 KB), `audit_trail.rs` (35.0 KB) |
| 10-20 KB | `supervisor.rs` (20.5 KB), `state_store.rs` (18.2 KB), `budget.rs` (19.0 KB), `event_bus.rs` (11.7 KB), `resource_monitor.rs` (11.8 KB), `daemon.rs` (10.3 KB), `metrics.rs` (11.7 KB) |
| 5-10 KB | `circuit_breaker.rs` (9.0 KB), `auth.rs` (8.2 KB), `engine.rs` (7.3 KB), `space.rs` (7.6 KB), `agent_lifecycle.rs` (8.3 KB) |
| <5 KB | `types.rs` (1.5 KB), `credential.rs` (4.8 KB), `error.rs` (8.8 KB) |

---

## 5. Cargo.toml Dependency Summary

### Required Dependencies (28)

| Category | Dependencies |
|----------|-------------|
| **Internal crates** | `oxios-ouroboros`, `oxios-markdown` |
| **oxi ecosystem** | `oxi-sdk`, `oxi-ai` |
| **Async runtime** | `tokio`, `futures`, `async-trait` |
| **Serialization** | `serde`, `serde_json`, `toml` |
| **Error handling** | `anyhow`, `thiserror` |
| **Time** | `chrono`, `cron` |
| **Crypto** | `sha2`, `hex`, `blake3`, `getrandom` |
| **Git** | `gix` (with `tree-editor` feature) |
| **Synchronization** | `parking_lot`, `once_cell` |
| **Logging** | `tracing`, `tracing-subscriber` |
| **Data structures** | `lru` |
| **System** | `sysinfo`, `dirs`, `libc` |
| **Search** | `usearch` (HNSW) |
| **Browser** | `oxibrowser-core` |
| **CLI/UI** | `inquire`, `console` |
| **Misc** | `uuid`, `regex`, `glob`, `tempfile` |

### Optional Dependencies (Feature-gated)

| Feature | Dependencies |
|---------|-------------|
| `otel` | `tracing-opentelemetry`, `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `opentelemetry-stdout` |
| `wasm-sandbox` | `wasmtime` (with cranelift), `wasmtime-wasi` |
| `browser` | (flag only — enables browser tool at compile time) |
