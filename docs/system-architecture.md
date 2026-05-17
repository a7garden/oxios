# Oxios Agent OS — System Architecture Reference

> **Version:** 0.1.0 · **Stack:** Rust 2021, tokio, serde (JSON+TOML), oxi-sdk · **License:** MIT
>
> This document is the authoritative technical reference for the Oxios Agent OS kernel.
> It is intended for contributors, AI agents working on the codebase, and anyone who needs
> to understand how the pieces fit together.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Layer Architecture](#2-layer-architecture)
3. [Kernel Subsystems](#3-kernel-subsystems)
4. [KernelHandle](#4-kernelhandle)
5. [Data Flow](#5-data-flow)
6. [Dependency Graph](#6-dependency-graph)
7. [Security Model](#7-security-model)
8. [Unix Philosophy Mapping](#8-unix-philosophy-mapping)
9. [Dependency Rules](#9-dependency-rules)

---

## 1. Overview

### What Is Oxios?

Oxios is an **Agent Operating System** — a Rust-based platform where AI agents execute real work on behalf of users. Agents are managed like Unix processes: they are forked, executed, waited on, and killed. The system applies the rigor of OS design to the chaos of LLM-driven autonomy.

### Design Philosophy

Two foundational ideas shape every design decision:

| Philosophy | Meaning in Oxios |
|---|---|
| **Unix Philosophy** | Every component does one thing. Compose small pieces. Pipes (EventBus) connect them. Agents are processes with lifecycles. |
| **Ouroboros First** | Never execute without a spec. Every user request passes through: Interview → Seed → Execute → Evaluate → Evolve. |

### Key Principles

```
┌─────────────────────────────────────────────────────────────────┐
│  No reimplementation  — Reuse oxi-sdk. Never reimplement what  │
│                          oxi already provides.                  │
│  Channel agnostic    — Gateway doesn't care where messages     │
│                          come from (Web, CLI, Telegram).        │
│  User invisible      — Users don't know how many agents are    │
│                          running. They talk; the OS handles it. │
│  No containers       — Direct host execution. Security via     │
│                          AccessManager (RBAC + path sandboxing).│
└─────────────────────────────────────────────────────────────────┘
```

### Project Layout

```
oxios/                         # Main binary (src/main.rs, src/kernel.rs)
├── crates/
│   ├── oxios-kernel/          # Core: supervisor, scheduler, event bus, state store, tools, memory
│   ├── oxios-ouroboros/       # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   └── oxios-gateway/         # Channel-agnostic message hub
├── channels/
│   ├── oxios-web/             # Web dashboard (Axum + Dioxus/WASM)
│   ├── oxios-cli/             # CLI channel
│   └── oxios-telegram/        # Telegram channel
├── .programs/                 # OS-level programs (code-review, debug, deploy, guardian, refactor…)
├── share/                     # Default skills, programs, config
└── docs/                      # Architecture docs, RFCs, design docs
```

---

## 2. Layer Architecture

Oxios is structured as a five-layer system, inspired by how an OS kernel
sits between user-facing interfaces and hardware (LLM providers).

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        LAYER 5: TERMINAL                                │
│   Web Dashboard · CLI · Telegram Bot · `oxios run` JSON API            │
│   (Feature-gated channel plugins)                                       │
├─────────────────────────────────────────────────────────────────────────┤
│                        LAYER 4: APPLICATION                             │
│   Gateway — channel-agnostic message routing and fan-out                │
│   Programs — installable OS-level capabilities                          │
│   Skills — markdown instruction templates                               │
├─────────────────────────────────────────────────────────────────────────┤
│                        LAYER 3: KERNEL                                  │
│   ┌──────────────┐ ┌───────────┐ ┌────────────┐ ┌───────────────────┐ │
│   │ Orchestrator  │ │ Supervisor│ │ Scheduler  │ │ AccessManager     │ │
│   │ (Ouroboros)   │ │ (init)    │ │ (queue)    │ │ (RBAC/sandbox)    │ │
│   └──────┬───────┘ └─────┬─────┘ └──────┬─────┘ └────────┬──────────┘ │
│          │               │              │                 │             │
│   ┌──────┴───────┐ ┌─────┴─────┐ ┌──────┴─────┐ ┌───────┴──────────┐ │
│   │ EventBus     │ │ AgentLife │ │ AuditTrail │ │ MemoryManager    │ │
│   │ (broadcast)  │ │ cycleMgr │ │ (Merkle)   │ │ (TF-IDF + HNSW) │ │
│   └──────────────┘ └───────────┘ └────────────┘ └──────────────────┘ │
│                                                                         │
│   KernelHandle — 11 typed Facades (the syscall table)                   │
├─────────────────────────────────────────────────────────────────────────┤
│                        LAYER 2: RUNTIME                                 │
│   AgentRuntime — wraps oxi-agent tool-calling loop                      │
│   A2AProtocol — agent-to-agent communication (Google A2A)               │
│   CircuitBreaker — 3-state LLM provider protection                      │
│   McpBridge — Model Context Protocol client                             │
├─────────────────────────────────────────────────────────────────────────┤
│                        LAYER 1: ENGINE                                  │
│   oxi-sdk (crates.io) — Provider, Model, AgentLoop, ToolRegistry       │
│   oxi-ai — Provider construction, streaming, tool execution             │
│   LLM Providers (Anthropic, OpenAI, Google, Ollama, …)                 │
└─────────────────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

| Layer | Responsibility | Key Types |
|---|---|---|
| **Terminal** | User-facing channels | `Channel` trait, CLI args, HTTP handlers |
| **Application** | Message routing, programs, skills | `Gateway`, `ProgramManager`, `SkillStore` |
| **Kernel** | Agent lifecycle, security, scheduling, state | `Kernel`, `KernelHandle`, 11 Facades |
| **Runtime** | LLM interaction, tool calling, inter-agent | `AgentRuntime`, `A2AProtocol`, `CircuitBreaker` |
| **Engine** | Low-level LLM provider abstraction | `oxi_sdk::Provider`, `oxi_sdk::AgentLoop` |

---

## 3. Kernel Subsystems

The kernel is the heart of Oxios. It is assembled in `src/kernel.rs` via a
builder pattern and exposes all operations through the `KernelHandle` facade.

### 3.1 Supervisor

> *"The init of Oxios."* — `supervisor.rs`

The Supervisor manages agent **process lifecycles**: fork, exec, wait, kill.
It is the most direct analogy to Unix process management.

```
                    Supervisor
                 ┌──────────────┐
   fork(spec) ──▶│ AgentId      │──▶ AgentCreated event
                 │  (Starting)  │
   exec(id)  ──▶│  (Running)   │──▶ AgentStarted event
                 │              │
   kill(id)  ──▶│  (Stopped)   │──▶ AgentStopped event
                 │              │
   wait(id)  ──▶│ AgentStatus  │──▶ Starting|Running|Idle|Failed|Stopped
                 └──────────────┘

   run_with_seed(id, seed) ──▶ tokio::spawn ──▶ AgentRuntime.execute()
                          ──▶ JoinHandle (abortable)
                          ──▶ AtomicBool (cooperative cancellation)
```

**Key design decisions:**
- Agents are tracked in-memory (`RwLock<HashMap<AgentId, AgentInfo>>`)
- Each running agent has an `AtomicBool` cancellation flag and a `JoinHandle` for task abortion
- `run_with_seed` spawns a tokio task, making `kill()` both cooperative (flag) and forced (abort)
- `NoOpSupervisor` exists as a build-time placeholder to break the KernelHandle→AgentRuntime→Supervisor cycle

### 3.2 Orchestrator

> *"The brain."* — `orchestrator.rs`

The Orchestrator coordinates the full **Ouroboros lifecycle** end-to-end.
It does NOT know about channels or HTTP — it only coordinates Ouroboros +
Supervisor + EventBus + StateStore + Scheduler + AccessManager.

```
  User Message
       │
       ▼
 ┌─────────────┐   ambiguity > 0.2
 │  Interview   │──────────────────▶ Return questions to user
 │  (Phase 1)   │                        (multi-turn session)
 └──────┬──────┘
        │ ready_for_seed
        ▼
 ┌─────────────┐
 │  Seed (2)    │  Generate Seed spec from interview
 └──────┬──────┘
        │
        ▼
 ┌─────────────┐   3+ acceptance criteria?
 │  Split?      │──────────────────────────▶ Multi-agent (A2A / lifecycle)
 └──────┬──────┘
        │ single agent
        ▼
 ┌─────────────┐
 │  Execute (3) │  Lifecycle: fork → register → schedule → run
 └──────┬──────┘
        │
        ▼
 ┌─────────────┐
 │  Evaluate(4) │  Score against acceptance criteria
 └──────┬──────┘
        │ score < 0.8 && iterations < 3
        ▼
 ┌─────────────┐
 │  Evolve (5)  │  Mutate seed, re-execute, re-evaluate
 └──────┬──────┘
        │
        ▼
   OrchestrationResult
```

**Multi-agent delegation:**
- Seeds with 3+ acceptance criteria are split into `SubTask`s
- Delegation prefers A2A protocol (capability-based routing)
- Falls back to `AgentLifecycleManager` when A2A is unavailable
- Uses `tokio::task::JoinSet` for parallel execution

**Chat bypass:** Simple conversational messages (greetings, small talk, short
messages without action verbs) get a direct LLM response, bypassing the full
Ouroboros pipeline.

### 3.3 AgentLifecycleManager

> *"Full lifecycle: fork → A2A register → permissions → schedule → run → cleanup"*
> — `agent_lifecycle.rs`

Extracted from Orchestrator to reduce god-object scope. Manages the complete
journey of a single agent:

```
 spawn_and_run(seed, priority)
       │
       ├── 1. Fork              supervisor.fork(seed)
       ├── 2. Register A2A      a2a.registry().register_agent(card)
       ├── 2b. Deliver pending   a2a.deliver_pending_messages()
       ├── 3. Permissions        access_manager.get_or_create_permissions()
       ├── 4. Submit + Start     scheduler.submit(task) → scheduler.start_task()
       ├── 5. Run (timeout)      supervisor.run_with_seed() with max_execution_time_secs
       └── 6. Cleanup            unregister A2A, complete/fail scheduler task
```

**Timeout enforcement:** If `max_execution_time_secs > 0`, the execution is
wrapped in `tokio::time::timeout`. On timeout, cleanup still runs to prevent
resource leaks.

### 3.4 AgentRuntime

> *"Wraps oxi-agent's tool-calling loop."* — `agent_runtime.rs`

The AgentRuntime creates a fresh `oxi_sdk::AgentLoop` session for each seed,
configures it with a CSpace-determined `ToolRegistry`, and runs it to completion.

```
 AgentRuntime.execute(agent_id, seed)
       │
       ├── Resolve CSpace (persona role / seed hint / default "worker")
       ├── Build system prompt (goal + constraints + persona + capabilities)
       ├── Semantic tool retrieval (ToolRetriever for relevant capabilities)
       ├── Recall memories (MemoryManager.recall)
       ├── Blend memories into system prompt
       │
       ├── Register tools from CSpace
       │   ├── Tier 1 (always-on): read, write, edit, grep, find, ls, web_search
       │   └── Tier 2 (CSpace-driven): exec, browser, memory, space, agent, a2a…
       │
       ├── Register program tools (from ProgramManager)
       ├── Register MCP tools (from McpBridge)
       │
       ├── spawn_blocking → run AgentLoop
       │   ├── AgentLoop::run(prompt, event_callback)
       │   └── Events: ToolExecutionEnd, AgentEnd, Error, Compaction
       │
       └── Return ExecutionResult { output, steps_completed, success }
```

**Key design notes:**
- `AgentLoop::run()` produces a `!Send` future, so execution happens inside `spawn_blocking`
- Compaction events are auto-stored as `MemoryType::Conversation` entries
- Circuit breaker protects against cascading LLM failures

### 3.5 Scheduler

> *"Priority-based task queue inspired by AIOS / AgentRM."* — `scheduler.rs`

```
 ┌─────────────────────────────────────────────┐
 │              AgentScheduler                  │
 │                                              │
 │   ┌──────────────────────────────────────┐  │
 │   │    BinaryHeap<ScheduledTask>          │  │
 │   │    ┌────────┐ ┌────────┐ ┌────────┐  │  │
 │   │    │Critical│ │ High   │ │ Normal │  │  │
 │   │    │ (3)    │ │ (2)    │ │ (1)    │  │  │
 │   │    └────────┘ └────────┘ └────────┘  │  │
 │   │                  ┌────────┐           │  │
 │   │                  │  Low   │           │  │
 │   │                  │ (0)    │           │  │
 │   │                  └────────┘           │  │
 │   └──────────────────────────────────────┘  │
 │                                              │
 │   Rate Limiter (sliding window)              │
 │   ┌─────────────────────────────┐           │
 │   │ window: Vec<DateTime>        │           │
 │   │ max_requests: 60/min         │           │
 │   └─────────────────────────────┘           │
 │                                              │
 │   Zombie Detection                           │
 │   ┌─────────────────────────────┐           │
 │   │ task_start_times: HashMap    │           │
 │   │ zombie_timeout_secs: 300     │           │
 │   └─────────────────────────────┘           │
 │                                              │
 │   Budget Gate (optional)                     │
 │   ┌─────────────────────────────┐           │
 │   │ BudgetManager integration    │           │
 │   │ can_schedule() soft gate     │           │
 │   └─────────────────────────────┘           │
 └─────────────────────────────────────────────┘
```

**Priority levels:** Critical (3) > High (2) > Normal (1) > Low (0)

**next_task() flow:**
1. Check `running.len() < max_concurrent`
2. Check rate limiter `allow()`
3. Pop highest-priority task from BinaryHeap
4. If budget manager attached, check `can_schedule(agent_id)` — skip if exhausted
5. Track start time for zombie detection

**Zombie reaping:** Tasks running longer than `zombie_timeout_secs` are
automatically marked as Failed and cleaned up.

### 3.6 StateStore

> *"JSON-on-disk persistence."*

The StateStore provides durable storage via a filesystem-based JSON store.
Every kernel subsystem that needs persistence goes through StateStore.

```
 ~/.oxios/workspace/
 ├── seeds/           ← Ouroboros seed specifications
 │   └── {uuid}.json
 ├── evals/           ← Evaluation results
 │   └── {uuid}-eval.json
 ├── memory/          ← Agent memory entries
 │   ├── conversations/
 │   ├── sessions/
 │   ├── facts/
 │   ├── episodes/
 │   └── knowledge/
 ├── programs/        ← Installed programs
 ├── skills/          ← Skill definitions
 ├── agent_groups/    ← Multi-agent group state
 └── audit/
     └── trail.json   ← Persisted audit trail
```

APIs: `save_json(category, key, value)`, `load_json(category, key)`,
`save_markdown(category, key, content)`, `delete(category, key)`.

### 3.7 EventBus

> *"The pipe of Oxios."* — `event_bus.rs`

All inter-component communication flows through the EventBus, implemented
as a tokio broadcast channel.

```
 ┌─────────────── publish() ──────────────┐
 │                                        │
 │  Orchestrator  Supervisor  Lifecycle   │
 │  MemoryMgr     SpaceMgr    A2A         │
 │                                        │
 └─────────── broadcast::channel ─────────┘
                          │
              ┌───────────┼───────────┐
              ▼           ▼           ▼
         Subscriber   AuditTrail   Channel
          (agents)    (attached)   (plugins)
```

**Event types (KernelEvent):**

| Event | Produced By |
|---|---|
| `AgentCreated` | Supervisor, A2A Registry |
| `AgentStarted` | Supervisor |
| `AgentStopped` | Supervisor |
| `AgentFailed` | Supervisor |
| `SeedCreated` | Orchestrator |
| `EvaluationComplete` | Orchestrator |
| `PhaseStarted` / `PhaseCompleted` | Orchestrator |
| `MessageReceived` | A2A Protocol |
| `AgentOutput` | AgentRuntime |
| `MemoryStored` / `MemoryRecalled` | MemoryManager |
| `ApprovalRequested` / `ApprovalResolved` | AccessManager RBAC |
| `SpaceCreated` / `SpaceActivated` / `SpaceArchived` | SpaceManager |
| `SpacesMerged` | SpaceManager |
| `AgentGroupCreated` / `AgentGroupMemberCompleted` | Orchestrator |

**Audit integration:** `attach_audit_trail()` spawns a background task that
converts every `KernelEvent` to an `AuditAction` and appends it to the
tamper-evident chain.

### 3.8 AccessManager

> *"OWASP-inspired least-privilege security."* — `access_manager/`

Every agent starts with minimal permissions and must be explicitly granted
access to tools, paths, and network resources.

```
 ┌───────────────────────────────────────────────────┐
 │                AccessManager                       │
 │                                                    │
│  ┌─────────────────────┐  ┌─────────────────────┐ │
│  │  AgentPermissions    │  │    RbacManager       │ │
│  │  ├─ allowed_tools    │  │  ├─ policies[]       │ │
│  │  ├─ allowed_paths[]  │  │  ├─ pending_approvals│ │
│  │  ├─ denied_paths[]   │  │  └─ audit_log[]      │ │
│  │  ├─ network_access   │  │                      │ │
│  │  ├─ can_fork         │  │  Subject → Role      │ │
│  │  ├─ max_exec_time    │  │    → Action → allow  │ │
│  │  └─ max_memory_mb    │  │                      │ │
│  └─────────────────────┘  └─────────────────────┘ │
│                                                     │
│  ┌─────────────────────────────────────────────────┐│
│  │  Workspace Sandbox                               ││
│  │  ├─ workspace_paths: name → PathBuf             ││
│  │  ├─ agent_workspaces: agent → workspace_name     ││
│  │  └─ workspace_agents: workspace → Set<agent>     ││
│  └─────────────────────────────────────────────────┘│
│                                                     │
│  ┌─────────────────────────────────────────────────┐│
│  │  Audit Log (bounded, async-persisted)            ││
│  │  ├─ max_audit_entries: 10,000                    ││
│  │  ├─ bounded channel (capacity 1000)              ││
│  │  └─ background writer task                       ││
│  └─────────────────────────────────────────────────┘│
└───────────────────────────────────────────────────────┘
```

**Three-layer sandbox check (`can_access_path_in_workspace`):**
1. **RBAC** — Does the agent's role allow the action?
2. **Path permissions** — Is the path in allowed_paths AND not in denied_paths?
3. **Workspace boundary** — Is the path within the agent's assigned workspace?

**Permission defaults for new agents:**
- Tools: `{bash, read, write, edit, grep, find}`
- Network: disabled
- Forking: disabled
- Execution time: 300 seconds
- Memory: 512 MB

### 3.9 AuditTrail

> *"Merkle-chain tamper-evident audit log."* — `audit_trail.rs`

Every security-relevant action is recorded in a cryptographic hash chain.
Each entry's hash is computed over all fields plus the previous entry's hash.

```
 ┌──────────────────────────────────────────────────────┐
 │                  Audit Trail Chain                    │
 │                                                      │
 │  [genesis]                                            │
 │     │                                                 │
 │     ▼ hash = blake3(seq + ts + actor + action + prev)│
 │  ┌──────────────────────────────────────────┐        │
 │  │ Entry #1                                 │        │
 │  │  seq: 1, actor: "agent-001"              │        │
 │  │  action: AgentSpawn { task_type: "…" }   │        │
 │  │  prev_hash: "genesis"                    │        │
 │  │  hash: "a3f8…c4d2"                      │        │
 │  └────────────────────┬─────────────────────┘        │
 │                       │                               │
 │                       ▼                               │
 │  ┌──────────────────────────────────────────┐        │
 │  │ Entry #2                                 │        │
 │  │  seq: 2, actor: "agent-001"              │        │
 │  │  action: ToolCall { tool: "exec", … }    │        │
 │  │  prev_hash: "a3f8…c4d2"                  │        │
 │  │  hash: "7b2e…f1a9"                      │        │
 │  └────────────────────┬─────────────────────┘        │
 │                       │                               │
 │                      …                                │
 │                                                      │
 │  Auto-prune: entries.len() > max_entries              │
 │    → drain oldest, mark first remaining as "pruned"   │
 │    → O(1) — no hash recomputation needed              │
 └──────────────────────────────────────────────────────┘
```

**Hash computation:** `blake3("oxios-audit-v1" || seq_be || timestamp || actor || action_json || prev_hash || resource)`

**Verification:** `verify()` walks the chain, recomputes each hash, and checks
prev_hash linkage. Detects any tampering with historical entries.

**Action types:** AgentSpawn, AgentExit, ToolCall, ToolResult, MemoryWrite,
MemoryRead, ConfigChange, ProgramInstall, CronTrigger, GitCommit,
AccessDenied, Other.

### 3.10 BudgetManager

> *"Token/cost limits per agent."* — `budget.rs`

Enforces per-agent budget limits on LLM API calls:

```
 BudgetLimit {
     agent_id:  Uuid,
     token_budget:  u32,      // max tokens per window
     calls_budget:  u32,      // max API calls per window
     window_secs:   u64,      // sliding window duration
 }
```

The scheduler checks `can_schedule(agent_id)` before admitting tasks.
When budget is exhausted, the agent's tasks are skipped in the queue.

### 3.11 ResourceMonitor

> *"CPU/memory tracking."* — `resource_monitor.rs`

Tracks system resource usage at configurable intervals:

```
 ResourceSnapshot {
     cpu_percent:     f32,
     memory_used_mb:  f64,
     memory_total_mb: f64,
     active_agents:   usize,
     uptime_secs:     u64,
 }
```

Maintains a bounded history ring buffer. The Guardian daemon checks
`is_overloaded()` every 300 seconds and logs to the audit trail.

### 3.12 GitLayer

> *"In-process version control via gix."* — `git_layer.rs`

Provides version control for all kernel state changes:

```
 GitLayer
 ├── new(workspace_path, auto_commit: bool)
 ├── commit_file(rel_path, message) → CommitInfo
 ├── remove_file(rel_path, message)
 ├── log(limit) → Vec<CommitInfo>
 ├── tag(name, message)
 ├── restore(commit_hash)
 └── verify() → bool  (repository integrity check)
```

Used by Orchestrator (seed/eval saves), MemoryManager (memory entries),
CronScheduler (state saves), and KernelHandle (convenience `commit_all`).

### 3.13 CronScheduler

> *"Scheduled job execution with persistent state."* — `cron.rs`

```
 CronScheduler
 ├── new(state_store, tick_interval_secs)
 ├── set_git_layer(git_layer)
 ├── add_job(CronJob) → job_id
 ├── remove_job(job_id)
 ├── list_jobs() → Vec<CronJob>
 └── tick()  ← called periodically
```

Jobs are persisted via StateStore and auto-committed to git.
Each tick evaluates pending jobs and spawns execution via AgentLifecycleManager.

### 3.14 MemoryManager

> *"TF-IDF, HNSW, hyperbolic embeddings, reasoning bank."* — `memory/`

The memory subsystem provides persistent, searchable memory for agents across
sessions.

```
 ┌─────────────────────────────────────────────────────────┐
 │                  MemoryManager                           │
 │                                                          │
 │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
 │  │  StateStore   │  │  VectorIndex  │  │  HNSW Index  │ │
 │  │  (JSON disk)  │  │  (TF-IDF)     │  │  (ANN search)│ │
 │  └──────────────┘  └──────────────┘  └──────────────┘ │
 │                                                          │
 │  Memory Types:                                           │
 │  ├── Conversation  (auto compaction summaries)           │
 │  ├── Session       (session-end summaries)               │
 │  ├── Fact          (agent-stored facts)                  │
 │  ├── Episode       (event/experience memories)           │
 │  └── Knowledge     (static knowledge)                    │
 │                                                          │
 │  Search Pipeline:                                        │
 │  1. TF-IDF embedding → EmbeddingVector                   │
 │  2. Cosine similarity ranking                            │
 │  3. Optional HNSW for fast ANN search                    │
 │  4. Effective importance = base × (1 + ln(1 + accesses))│
 │                                                          │
 │  Sub-modules:                                            │
 │  ├── hyperbolic/    — Poincaré ball embeddings           │
 │  ├── flash_attention/ — memory attention scoring         │
 │  ├── graph/         — MemoryGraph (entity relationships) │
 │  ├── hnsw/          — HNSW approximate nearest neighbor  │
 │  ├── chunking/      — text chunking (fixed, paragraph)   │
 │  ├── budget/        — MemoryBudget curation              │
 │  ├── normalizer/    — L2 normalize, cosine similarity    │
 │  └── auto_memory_bridge/ — auto knowledge extraction     │
 └─────────────────────────────────────────────────────────┘
```

**Key APIs:** `remember(entry)`, `recall(query)`, `search(query, type, limit)`,
`forget(id, type)`, `blend_into_prompt(memories, base_prompt)`, `curate(budget)`.

**Curation:** Background task prunes low-importance memories based on
`MemoryBudget` limits per type. Effective importance scores combine base
importance with access frequency.

### 3.15 ProgramManager

> *"Installable OS-level programs."* — `program/`

Programs are the "applications" of Oxios — installable capabilities that
extend agent abilities.

```
 .programs/
 ├── code-review/
 │   ├── PROGRAM.toml    ← metadata, tools, dependencies, MCP servers
 │   └── SKILL.md        ← instruction template
 ├── debug/
 ├── deploy/
 ├── guardian/
 ├── refactor/
 └── program-creator/
```

**Program metadata (PROGRAM.toml):**
- `tools[]` — commands the program provides
- `dependencies[]` — required host tools
- `mcp_servers[]` — MCP server configurations
- `host_requirements` — required/optional system tools

### 3.16 SkillStore

> *"Markdown instruction templates."* — `skill/`

Skills are markdown files that provide instruction templates for agents.
Stored in `~/.oxios/workspace/skills/` and initialized from `share/default-skills/`.

### 3.17 PersonaManager

> *"Multiple AI characters."* — `persona_manager.rs`

Supports multiple personas with different system prompts, roles, and
behaviors. The active persona's system prompt is injected into both
Ouroboros engine and AgentRuntime.

```
 Persona {
     name:         String,
     role:         String,     // e.g., "coder", "reviewer", "planner"
     system_prompt: String,
     enabled:      bool,
 }
```

### 3.18 McpBridge

> *"Model Context Protocol."* — `mcp/`

Provides integration with external MCP (Model Context Protocol) servers:

```
 McpBridge
 ├── register_server(McpServer)
 ├── initialize_all()         ← starts all registered servers
 ├── list_tools()             ← enumerates all server tools
 ├── cached_tools(server)     ← returns cached tool definitions
 └── call_tool(server, tool, args)  ← executes an MCP tool
```

MCP servers are registered from:
1. `config.toml` `[mcp.servers]` section
2. Environment variables (`OXIOS_MCP_{NAME}_COMMAND`)
3. Program metadata (`PROGRAM.toml` `mcp_servers[]`)

### 3.19 A2AProtocol

> *"Agent-to-agent communication (Google A2A)."* — `a2a.rs`

A2A is the horizontal communication layer. Unlike MCP (agent→tool),
A2A enables agent→agent discovery, delegation, and result sharing.

```
 ┌─────────────────────────────────────────────────────┐
 │                  A2AProtocol                         │
 │                                                      │
 │  ┌──────────────────────────────────────────────┐   │
 │  │            AgentCardRegistry                  │   │
 │  │  agent_id → AgentCard {                       │   │
 │  │    name, description,                         │   │
 │  │    capabilities[], skills[],                  │   │
 │  │    endpoint, status                           │   │
 │  │  }                                            │   │
 │  └──────────────────────────────────────────────┘   │
 │                                                      │
 │  Message Types:                                      │
 │  ├── TaskDelegation   { task_id, description, … }    │
 │  ├── StatusUpdate     { task_id, progress, message } │
 │  ├── ResultSharing    { task_id, result, summary }   │
 │  ├── CapabilityQuery  { query, required_capabilities}│
 │  └── Handshake        { agent_id, name, capabilities}│
 │                                                      │
 │  Per-Agent Queues (Notify-based):                    │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐    │
 │  │ agent-001  │  │ agent-002  │  │ agent-003  │    │
 │  │ messages[] │  │ messages[] │  │ messages[] │    │
 │  │ Notify     │  │ Notify     │  │ Notify     │    │
 │  └────────────┘  └────────────┘  └────────────┘    │
 │                                                      │
 │  DelegationHandler:                                   │
 │  TaskDelegation → spawn agent → return result         │
 └─────────────────────────────────────────────────────┘
```

**Routing patterns:**
- `send_message(from, to, message)` — fire-and-forget
- `delegate_task(from, to, task)` — enqueue for processing
- `send_and_wait(from, to, message, timeout)` — RPC-style with response matching
- `query_capabilities(capability)` — discover agents by capability

### 3.20 SpaceManager

> *"Conversation context with 3-layer detection."* — `space/manager.rs`

Spaces partition conversation context into isolated domains, each with its
own workspace directory, memory, and knowledge.

```
 ┌─────────────────────────────────────────────────────────┐
 │                   SpaceManager                           │
 │                                                          │
 │  3-Layer Detection Strategy:                             │
 │                                                          │
 │  Layer 1: Filesystem Path                                │
 │  ┌──────────────────────────────────────────────┐       │
 │  │ Extract path from message → PathMatcher       │       │
 │  │ "/projects/oxios/main.rs" → oxios Space       │       │
 │  └──────────────────────────────────────────────┘       │
 │         │ (miss)                                         │
 │         ▼                                                │
 │  Layer 2: Keyword/Tag Matching                           │
 │  ┌──────────────────────────────────────────────┐       │
 │  │ Match message keywords against Space tags      │       │
 │  │ "debug the auth module" → auth Space           │       │
 │  └──────────────────────────────────────────────┘       │
 │         │ (miss)                                         │
 │         ▼                                                │
 │  Layer 3: Topic Classification (LLM-based)               │
 │  ┌──────────────────────────────────────────────┐       │
 │  │ classify_topic(message) → Topic                │       │
 │  │ Topic shift? → Create new or switch Space      │       │
 │  └──────────────────────────────────────────────┘       │
 │                                                          │
 │  Space Lifecycle:                                        │
 │  ├── Default Space (always exists, unnamed)              │
 │  ├── Auto-created from path/topic detection              │
 │  ├── Manual creation via SpaceTool                       │
 │  ├── Merge (survivor absorbs another)                    │
 │  ├── Archive (stale after 30 days)                       │
 │  └── Restore from archive                                │
 └─────────────────────────────────────────────────────────┘
```

**Space structure:**
```
 ~/.oxios/spaces/
 ├── _index.json            ← list of all Space IDs
 ├── 00000000-…-0001/       ← default Space
 │   ├── space.json
 │   └── workspace/
 ├── {uuid}/                ← auto or manual Spaces
 │   ├── space.json
 │   └── workspace/
 └── _archived/             ← archived Spaces
```

### 3.21 CircuitBreaker

> *"3-state LLM provider protection."* — `circuit_breaker.rs`

Protects against cascading LLM provider failures using the classic
circuit breaker pattern:

```
         success            failures ≥ threshold
   ┌──────────────┐    ┌──────────────────────────┐
   │              │    │                          │
   │    CLOSED    │───▶│         OPEN             │
   │  (normal)    │    │  (rejecting requests)    │
   │              │    │                          │
   └──────────────┘    └────────────┬─────────────┘
         ▲                          │
         │ success                  │ timeout elapsed
         │                          ▼
         │              ┌──────────────────────────┐
         └──────────────│       HALF-OPEN          │
                        │  (single probe request)  │
                        │                          │
                        └────────────┬─────────────┘
                                     │ failure
                                     │
                                     ▼
                              back to OPEN
```

**Defaults:** 5 consecutive failures → open, 30 second timeout → half-open.
A single probe request tests recovery; success closes, failure reopens.

### 3.22 HostToolValidator

> *"Validates required/optional host tools."* — `host_tools.rs`

Checks the host system for required and optional tools that programs
depend on. Reports missing tools during program installation.

### 3.23 AuthManager

> *"SHA-256 hashed key storage."* — `auth.rs`

Manages API keys and authentication tokens. Keys are stored as SHA-256
hashes. API key resolution follows a priority chain: `config.toml` engine
section → `~/.oxi/auth.json` → environment variables.

### 3.24 WasmSandbox

> *"WASM-based sandbox for executing untrusted code."* — `wasm_sandbox.rs`

Provides a WebAssembly-based execution sandbox for running untrusted
code in isolation. Used for safe execution of user-provided scripts.

### 3.25 CredentialStore

> *"Multi-source credential resolution."* — `credential.rs`

Resolves credentials from multiple sources in priority order:

```
 1. config.toml [engine] section
      │
      ▼ (not found)
 2. ~/.oxi/auth.json (oxi-cli credentials)
      │
      ▼ (not found)
 3. Environment variables (ANTHROPIC_API_KEY, OPENAI_API_KEY, …)
```

### 3.26 ContextManager

> *"3-tier context hierarchy."*

Manages the context window hierarchy for agent conversations:

```
 Tier 1: System Prompt (persona + constraints + capabilities)
 Tier 2: Memory Context (recalled memories blended into prompt)
 Tier 3: Conversation History (messages within current session)
```

---

## 4. KernelHandle

> *"The syscall table of the Agent OS."* — `kernel_handle/`

The KernelHandle is the primary API surface for all kernel operations.
It is composed of **11 typed Facades**, each encapsulating a domain.

```
 ┌─────────────────────────────────────────────────────────────────┐
 │                      KernelHandle                                │
 │                                                                  │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐                │
 │  │  StateApi   │  │  AgentApi   │  │ SecurityApi │                │
 │  │             │  │             │  │             │                │
 │  │ save()      │  │ supervisor  │  │ auth_mgr    │                │
 │  │ load()      │  │ budget_mgr  │  │ audit_trail │                │
 │  │ delete()    │  │ memory_mgr  │  │ access_mgr  │                │
 │  │ sessions    │  │             │  │ state_store │                │
 │  └────────────┘  └────────────┘  └────────────┘                │
 │                                                                  │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐                │
 │  │ PersonaApi  │  │ ExecApi     │  │ BrowserApi  │                │
 │  │             │  │             │  │             │                │
 │  │ persona_mgr │  │ exec_config │  │ (feature    │                │
 │  │             │  │ access_mgr  │  │  gated)     │                │
 │  └────────────┘  └────────────┘  └────────────┘                │
 │                                                                  │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐                │
 │  │   McpApi    │  │ExtensionApi │  │  InfraApi   │                │
 │  │             │  │             │  │             │                │
 │  │ mcp_bridge  │  │ program_mgr │  │ git_layer   │                │
 │  │             │  │ skill_store │  │ scheduler   │                │
 │  │             │  │ host_tools  │  │ cron        │                │
 │  └────────────┘  └────────────┘  │ resource_mon│                │
 │                                   │ event_bus   │                │
 │  ┌────────────┐  ┌────────────┐  │ config      │                │
 │  │  A2aApi     │  │  SpaceApi   │  └────────────┘                │
 │  │             │  │             │                                │
 │  │ a2a_proto   │  │ space_mgr   │                                │
 │  │             │  │ event_bus   │                                │
 │  └────────────┘  └────────────┘                                │
 └─────────────────────────────────────────────────────────────────┘
```

### Facade Summary

| Facade | Domain | Key Subsystems |
|---|---|---|
| `StateApi` | Data persistence | StateStore, sessions |
| `AgentApi` | Agent lifecycle | Supervisor, BudgetManager, MemoryManager |
| `SecurityApi` | Auth & audit | AuthManager, AuditTrail, AccessManager |
| `PersonaApi` | AI characters | PersonaManager |
| `ExecApi` | Execution config | ExecConfig, AccessManager |
| `BrowserApi` | Browser backend | Feature-gated, zero-sized when disabled |
| `McpApi` | MCP protocol | McpBridge |
| `ExtensionApi` | Programs & skills | ProgramManager, SkillStore, HostToolValidator |
| `InfraApi` | Infrastructure | GitLayer, AgentScheduler, CronScheduler, ResourceMonitor, EventBus |
| `A2aApi` | Agent-to-agent | A2AProtocol |
| `SpaceApi` | Context spaces | SpaceManager, EventBus |

### Cross-Facade Convenience Methods

The KernelHandle provides convenience methods that orchestrate across facades:

| Method | Facades Used | Description |
|---|---|---|
| `save_and_commit()` | State + Infra | Save JSON + git commit |
| `save_markdown_and_commit()` | State + Infra | Save markdown + git commit |
| `delete_and_commit()` | State + Infra | Delete + git remove |
| `commit_all()` | State + Infra | Commit all pending changes |
| `flush_audit()` | Security + Infra | Flush audit trail + git commit |
| `schedule()` | Infra | Add cron job |
| `load_json()` | State | Load typed JSON |

### Caching

The KernelHandle is created once per `Kernel` instance and cached in a
`OnceLock`. All access goes through `kernel.handle()` which returns
`Arc<KernelHandle>`.

---

## 5. Data Flow

### How a User Message Flows Through the System

```
 User types: "Fix the auth bug in main.rs"
                    │
 ═══════════════════╪═════════════════════════════════════════
 LAYER 5: TERMINAL  │
 ═══════════════════╪═════════════════════════════════════════
                    ▼
            ┌──────────────┐
            │  CLI / Web / │
            │  Telegram    │
            └──────┬───────┘
                   │
 ══════════════════╪═════════════════════════════════════════
 LAYER 4: APP      │
 ══════════════════╪═════════════════════════════════════════
                   ▼
            ┌──────────────┐
            │   Gateway    │  Route to Orchestrator
            └──────┬───────┘
                   │
 ══════════════════╪═════════════════════════════════════════
 LAYER 3: KERNEL   │
 ══════════════════╪═════════════════════════════════════════
                   ▼
        ┌─────────────────────┐
        │    Orchestrator      │
        │                      │
        │  1. Space Detection  │──▶ SpaceManager.detect_or_create()
        │     (3-layer)        │     → /projects/myapp Space
        │                      │
        │  2. Chat Bypass?     │──▶ No → continue
        │                      │
        │  3. Interview        │──▶ OuroborosProtocol.interview()
        │     (ambiguity <0.2) │     → ready_for_seed = true
        │                      │
        │  4. Generate Seed    │──▶ OuroborosProtocol.generate_seed()
        │                      │     → Seed { goal, constraints, … }
        │                      │
        │  5. Execute          │──▶ AgentLifecycleManager.spawn_and_run()
        └──────────┬───────────┘
                   │
                   ▼
        ┌─────────────────────┐
        │  AgentLifecycleMgr   │
        │                      │
        │  fork() ────────────▶│ Supervisor.fork() → AgentId
        │  register A2A ──────▶│ A2AProtocol.registry().register()
        │  permissions ───────▶│ AccessManager.get_or_create()
        │  submit task ───────▶│ AgentScheduler.submit() + start()
        │  run_with_seed() ───▶│ Supervisor.run_with_seed()
        └──────────┬───────────┘
                   │
 ══════════════════╪═════════════════════════════════════════
 LAYER 2: RUNTIME  │
 ══════════════════╪═════════════════════════════════════════
                   ▼
        ┌─────────────────────┐
        │    AgentRuntime      │
        │                      │
        │  resolve CSpace      │──▶ capability::resolve_cspace()
        │  build system prompt │──▶ persona + seed + memories
        │  register tools      │──▶ CSpace → ToolRegistry mapping
        │  recall memories     │──▶ MemoryManager.recall()
        │                      │
        │  spawn_blocking ────▶│ AgentLoop::run()
        │                      │  ├── LLM generates tool calls
        │                      │  ├── Tools execute via KernelHandle
        │                      │  ├── CircuitBreaker protects LLM calls
        │                      │  └── Compaction saves conversation memory
        └──────────┬───────────┘
                   │
 ══════════════════╪═════════════════════════════════════════
 LAYER 1: ENGINE   │
 ══════════════════╪═════════════════════════════════════════
                   ▼
        ┌─────────────────────┐
        │     oxi-sdk          │
        │                      │
        │  Provider.stream()   │──▶ Anthropic / OpenAI / Google / Ollama
        │  AgentLoop.run()     │──▶ Multi-turn tool-calling loop
        │  ToolRegistry        │──▶ Tool dispatch
        └──────────┬───────────┘
                   │
                   ▼
         LLM Provider API
```

### Response Path

```
 AgentLoop completes
       │
       ▼
 AgentRuntime returns ExecutionResult { output, steps_completed, success }
       │
       ▼
 AgentLifecycleManager.cleanup() — unregister A2A, complete scheduler task
       │
       ▼
 Orchestrator evaluates result
       │
       ├── Evaluation passed ──▶ Return OrchestrationResult
       │
       └── Evaluation failed ──▶ Evolve seed, re-execute (up to 3 iterations)
                                       │
                                       ▼
                                  Return OrchestrationResult {
                                    session_id, space_id, space_tag,
                                    response, seed_id, evaluation_passed,
                                    phase_reached, output
                                  }
```

---

## 6. Dependency Graph

### Crate-Level Dependencies

```
                    ┌──────────────┐
                    │    oxios      │  (main binary)
                    │  src/main.rs  │
                    │  src/kernel.rs│
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
     ┌────────────┐ ┌──────────┐ ┌───────────────┐
     │oxios-kernel│ │ oxios-   │ │ oxios-web /   │
     │            │ │ ouroboros│ │ oxios-cli /   │
     │            │ │          │ │ oxios-telegram│
     └─────┬──────┘ └─────┬────┘ └───────────────┘
           │              │
           │              │
     ┌─────┼──────────────┤
     │     │              │
     ▼     ▼              ▼
 ┌──────────┐  ┌──────────────────┐
 │ oxi-sdk  │  │    oxi-ai        │
 │(crates.io│  │ (provider        │
 │ NOT path │  │  construction)   │
 │  dep)    │  │                  │
 └──────────┘  └──────────────────┘
```

### Detailed Crate Dependencies

```
 oxios
 ├── oxios-kernel
 │   ├── oxi-sdk              (crates.io)
 │   ├── oxi-ai               (provider construction)
 │   ├── oxios-ouroboros      (path dep)
 │   ├── tokio                (async runtime)
 │   ├── serde / serde_json   (serialization)
 │   ├── parking_lot          (fast mutexes)
 │   ├── blake3               (audit trail hashing)
 │   ├── gix                  (in-process git)
 │   ├── chrono               (timestamps)
 │   ├── uuid                 (IDs)
 │   ├── anyhow / thiserror   (errors)
 │   ├── tracing              (logging)
 │   └── hnsw                 (ANN search)
 │
 ├── oxios-ouroboros
 │   ├── oxi-sdk              (LLM calls)
 │   ├── serde / serde_json
 │   ├── tokio
 │   └── chrono
 │
 ├── oxios-gateway
 │   ├── tokio
 │   └── async-trait
 │
 ├── oxios-web                (feature-gated)
 │   ├── axum                 (HTTP server)
 │   ├── dioxus               (WASM frontend)
 │   └── oxios-kernel
 │
 ├── oxios-cli                (feature-gated)
 │   ├── clap                 (CLI parsing)
 │   └── oxios-kernel
 │
 └── oxios-telegram           (feature-gated)
     ├── teloxide             (Telegram bot)
     └── oxios-kernel
```

### Internal Kernel Module Map

```
 oxios-kernel
 ├── src/
 │   ├── lib.rs              ← public re-exports
 │   ├── supervisor.rs       ← Supervisor trait + BasicSupervisor
 │   ├── orchestrator.rs     ← Orchestrator (Ouroboros coordinator)
 │   ├── agent_lifecycle.rs  ← AgentLifecycleManager
 │   ├── agent_runtime.rs    ← AgentRuntime (oxi-agent wrapper)
 │   ├── scheduler.rs        ← AgentScheduler
 │   ├── event_bus.rs        ← EventBus + KernelEvent
 │   ├── circuit_breaker.rs  ← CircuitBreaker
 │   ├── audit_trail.rs      ← AuditTrail (Merkle chain)
 │   ├── budget.rs           ← BudgetManager
 │   ├── resource_monitor.rs ← ResourceMonitor
 │   ├── git_layer.rs        ← GitLayer (gix)
 │   ├── cron.rs             ← CronScheduler
 │   ├── auth.rs             ← AuthManager
 │   ├── credential.rs       ← CredentialStore
 │   ├── config.rs           ← OxiosConfig
 │   ├── persona_manager.rs  ← PersonaManager
 │   ├── host_tools.rs       ← HostToolValidator
 │   ├── wasm_sandbox.rs     ← WasmSandbox
 │   ├── onboarding.rs       ← Interactive setup wizard
 │   ├── daemon.rs           ← PID file, launchd/systemd
 │   ├── agent_group.rs      ← OxiosAgentGroup
 │   ├── metrics.rs          ← OpenTelemetry metrics
 │   ├── state_store.rs      ← StateStore (JSON disk)
 │   │
 │   ├── access_manager/     ← AccessManager, RBAC, Permissions
 │   │   ├── mod.rs
 │   │   ├── permissions.rs
 │   │   └── rbac.rs
 │   │
 │   ├── memory/             ← MemoryManager
 │   │   ├── mod.rs
 │   │   ├── store.rs        (HNSW index)
 │   │   ├── budget.rs       (MemoryBudget curation)
 │   │   ├── chunking.rs     (text chunking)
 │   │   ├── graph.rs        (MemoryGraph)
 │   │   ├── hnsw.rs         (HNSW implementation)
 │   │   ├── hyperbolic.rs   (Poincaré embeddings)
 │   │   ├── flash_attention.rs
 │   │   ├── normalizer.rs
 │   │   └── auto_memory_bridge.rs
 │   │
 │   ├── mcp/                ← McpBridge
 │   │   ├── mod.rs
 │   │   ├── client.rs
 │   │   ├── protocol.rs
 │   │   └── server.rs
 │   │
 │   ├── space/              ← SpaceManager
 │   │   ├── mod.rs
 │   │   ├── manager.rs
 │   │   ├── detection.rs
 │   │   ├── conversation_buffer.rs
 │   │   └── knowledge_bridge.rs
 │   │
 │   ├── program/            ← ProgramManager
 │   ├── skill/              ← SkillStore
 │   ├── capability/         ← CSpace resolution
 │   ├── a2a.rs              ← A2AProtocol
 │   │
 │   ├── tools/              ← Agent tool implementations
 │   │   ├── registration.rs (CSpace → ToolRegistry mapping)
 │   │   ├── retrieval.rs    (ToolRetriever, TF-IDF)
 │   │   ├── exec_tool.rs    (shell + structured execution)
 │   │   ├── wasm_tool.rs
 │   │   ├── mcp_tool.rs     (McpToolWrapper)
 │   │   ├── program_tool.rs (ProgramTool)
 │   │   ├── kernel/         ← Kernel domain tools
 │   │   │   ├── agent_tool.rs
 │   │   │   ├── space_tool.rs
 │   │   │   ├── persona_tool.rs
 │   │   │   ├── security_tool.rs
 │   │   │   ├── budget_tool.rs
 │   │   │   ├── cron_tool.rs
 │   │   │   ├── resource_tool.rs
 │   │   │   └── mcp_tool.rs
 │   │   └── memory/
 │   │       ├── read_tool.rs
 │   │       ├── write_tool.rs
 │   │       └── search_tool.rs
 │   │
 │   ├── kernel_handle/      ← 11 Facades
 │   │   ├── mod.rs
 │   │   ├── state_api.rs
 │   │   ├── agent_api.rs
 │   │   ├── security_api.rs
 │   │   ├── persona_api.rs
 │   │   ├── exec_api.rs
 │   │   ├── browser_api.rs
 │   │   ├── mcp_api.rs
 │   │   ├── extension_api.rs
 │   │   ├── infra_api.rs
 │   │   ├── space_api.rs
 │   │   └── a2a_api.rs
 │   │
 │   └── embedding/          ← TF-IDF embedding provider
```

---

## 7. Security Model

Oxios follows an **OWASP Agentic AI** security posture: least privilege by
default, defense in depth, and comprehensive audit logging.

### 7.1 Security Layers

```
 ┌─────────────────────────────────────────────────────────┐
 │                  Request Flow                            │
 │                                                          │
 │  User Message                                            │
 │       │                                                  │
 │       ▼                                                  │
 │  ┌──────────┐                                            │
 │  │ Gateway  │  Channel auth (API keys, tokens)           │
 │  └────┬─────┘                                            │
 │       ▼                                                  │
 │  ┌──────────┐                                            │
 │  │AuthMgr   │  Identity verification (SHA-256 hashed)    │
 │  └────┬─────┘                                            │
 │       ▼                                                  │
 │  ┌──────────────┐                                        │
 │  │AccessManager │  Three-layer sandbox:                  │
 │  │              │  1. RBAC (role → action → allow/deny)  │
 │  │  RBAC ──────▶│  2. Path permissions (allow + deny)    │
 │  │  Paths ─────▶│  3. Workspace boundary (canonicalize)  │
 │  │  Workspace──▶│                                       │
 │  └────┬─────────┘                                        │
 │       │ (allowed)                                         │
 │       ▼                                                  │
 │  ┌──────────┐                                            │
 │  │ExecTool  │  Two execution modes:                      │
 │  │          │  ├── shell: bash -c (RBAC-enforced)        │
 │  │          │  └── structured: binary allowlist           │
 │  │          │      + metacharacter blocking               │
 │  └────┬─────┘                                            │
 │       │                                                  │
 │       ▼                                                  │
 │  ┌──────────┐                                            │
 │  │AuditTrail│  Cryptographic hash chain (blake3)         │
 │  │          │  Tamper-evident, queryable, exportable     │
 │  └──────────┘                                            │
 └─────────────────────────────────────────────────────────┘
```

### 7.2 RBAC Model

```
 Subject (AgentId)
    │
    ▼
 Role (admin, worker, restricted)
    │
    ▼
 Action (AccessPath, UseTool, NetworkRequest, Fork, …)
    │
    ▼
 Policy (allow/deny + resource pattern)
```

The RBAC system supports Human-in-the-Loop (HitL) approvals:
high-risk actions can require explicit user approval before execution.

### 7.3 Path Sandboxing

```
 Agent assigned to workspace "project-alpha"
         │
         ▼
 Workspace path: /workspace/alpha/
         │
         ├── /workspace/alpha/src/main.rs     ✅ allowed
         ├── /workspace/alpha/tests/mod.rs    ✅ allowed
         ├── /etc/passwd                      ❌ outside workspace
         ├── /workspace/beta/main.rs          ❌ outside workspace
         └── /workspace/alpha/.secret/key     ❌ denied by deny pattern
```

Path matching uses glob patterns with deny-lists taking precedence over
allow-lists. Canonical path resolution prevents symlink-based escapes.

### 7.4 Execution Security

| Mode | Mechanism | Protection |
|---|---|---|
| **Shell** (`bash -c`) | RBAC-enforced | Agent must have `exec:shell` permission |
| **Structured** | Binary allowlist | Only pre-approved binaries; metacharacter blocking |
| **WASM** | WebAssembly sandbox | Memory-limited, capability-restricted |

### 7.5 Audit Trail Integrity

- Every access decision is logged
- Hash chain makes retroactive tampering detectable
- `verify()` recomputes all hashes and checks linkage
- Guardian daemon runs `verify_chain()` every 300 seconds
- Persisted to disk via StateStore + optional file log

---

## 8. Unix Philosophy Mapping

Oxios explicitly maps Unix OS concepts to the Agent OS domain:

```
 ┌─────────────────────────────────────────────────────────────┐
 │                  Unix → Oxios Mapping                        │
 │                                                              │
 │  Unix Concept        Oxios Equivalent                        │
 │  ─────────────       ────────────────                        │
 │  Process             Agent (forked from Seed)                │
 │  PID                 AgentId (UUID)                          │
 │  fork()              Supervisor.fork(seed)                   │
 │  exec()              Supervisor.exec(id) / run_with_seed()   │
 │  wait()              Supervisor.wait(id)                     │
 │  kill()              Supervisor.kill(id)                     │
 │  init (PID 1)        Supervisor (manages all agents)         │
 │  pipe                EventBus (broadcast channel)            │
 │  signal              KernelEvent enum                        │
 │  filesystem          StateStore (JSON-on-disk)               │
 │  /proc               AgentInfo status struct                 │
 │  chmod/chown         AccessManager permissions               │
 │  chroot              Workspace sandboxing                    │
 │  syslog              AuditTrail (Merkle hash chain)          │
 │  cron                CronScheduler                           │
 │  git                 GitLayer (in-process via gix)           │
 │  sysctl              KernelHandle (syscall table)            │
 │  daemon              Oxios daemon (launchd/systemd)          │
 │  stdout/stderr       OrchestrationResult.response            │
 │  exit code           OrchestrationResult.evaluation_passed   │
 │  init.d scripts      Programs (.programs/)                   │
 │  man pages           Skills (SKILL.md templates)             │
 │  IPC                 A2A Protocol (agent-to-agent)           │
 │  network             McpBridge (Model Context Protocol)      │
 │  swap                MemoryManager (compaction + curation)   │
 │  load avg            ResourceMonitor (CPU/memory tracking)   │
 └─────────────────────────────────────────────────────────────┘
```

### Ouroboros as "Never exec Without a Spec"

Where Unix says "never execute arbitrary code," Oxios says "never execute
without a specification." The Ouroboros loop enforces this:

```
 Interview → understand what the user wants
 Seed      → formalize into a specification
 Execute   → run with the spec as contract
 Evaluate  → verify against acceptance criteria
 Evolve    → mutate and retry if needed (max 3 iterations)
```

### Composability

Like Unix pipes, Oxios agents compose through the EventBus and A2A:

```
 User Request
      │
      ├──▶ Orchestrator
      │       │
      │       ├──▶ Agent A (code-review)
      │       │       │
      │       │       └──▶ A2A → Agent B (testing)
      │       │                        │
      │       │                        └──▶ Result back to A
      │       │
      │       └──▶ Agent C (refactoring)
      │
      └──▶ Combined Result
```

---

## 9. Dependency Rules

### Layer Dependencies (Top-Down Only)

```
 Terminal  ──▶  Application  ──▶  Kernel  ──▶  Runtime  ──▶  Engine
   │               │               │            │            │
   │               │               │            │            │
   ▼               ▼               ▼            ▼            ▼
 Web/CLI/TG     Gateway       KernelHandle   AgentRuntime  oxi-sdk
 Programs       SkillStore    Orchestrator   A2AProtocol
                              Supervisor     CircuitBreaker
                              Scheduler
                              AccessManager
                              EventBus
                              StateStore
```

**Rule:** Dependencies flow downward. No reverse dependencies.

### Crate Dependency Rules

```
 1. oxios-kernel depends on oxi-sdk (crates.io, NOT path dep)
 2. oxios-kernel depends on oxi-ai (provider construction)
 3. oxios-kernel depends on oxios-ouroboros (path dep)
 4. oxios depends on oxios-kernel, oxios-ouroboros, oxios-gateway
 5. oxios depends on channel plugins (feature-gated: web, cli, telegram)
 6. Channel plugins depend on oxios-gateway (NOT directly on kernel)
 7. No circular dependencies between crates
```

### Internal Module Rules

```
 1. KernelHandle depends on all subsystems — it is the top-level facade
 2. Tools depend on KernelHandle (injected at registration time)
 3. AgentRuntime depends on KernelHandle (injected at construction)
 4. Supervisor depends on AgentRuntime (owns the runtime instance)
 5. AgentLifecycleManager depends on Supervisor, Scheduler, AccessManager, A2A
 6. Orchestrator depends on OuroborosProtocol, AgentLifecycleManager, StateStore, EventBus
 7. No subsystem depends on Orchestrator — it is a leaf consumer
```

### No-No List

| ❌ Forbidden | ✅ Correct |
|---|---|
| Tools importing Orchestrator | Tools use KernelHandle APIs |
| KernelHandle importing AgentRuntime | AgentRuntime receives KernelHandle via constructor |
| Channels importing kernel internals | Channels go through Gateway |
| Reimplementing oxi-sdk features | Use oxi-sdk directly |
| Adding lifecycle logic to Orchestrator | Use AgentLifecycleManager |
| Circular crate dependencies | DAG only |

### Feature Gates

```toml
# Cargo.toml features
[features]
web      = ["oxios-web"]
cli      = ["oxios-cli"]
telegram = ["oxios-telegram"]
browser  = ["chromiumoxide"]  # BrowserTool
otel     = ["opentelemetry"] # Telemetry
default  = ["cli"]
```

Feature-gated code compiles to no-ops when disabled (e.g., `BrowserApi::default()`
is zero-sized without the browser feature).

---

## Appendix A: Key Configuration

```toml
# ~/.oxios/config.toml

[engine]
default_model = "anthropic/claude-sonnet-4-20250514"
api_key = "sk-..."                    # or use ~/.oxi/auth.json

[kernel]
workspace = "~/.oxios/workspace"
event_bus_capacity = 256

[scheduler]
max_concurrent = 5
rate_limit_per_minute = 60
zombie_timeout_secs = 300

[security]
audit_log_path = "~/.oxios/audit.log"
max_execution_time_secs = 300

[resource_monitor]
interval_secs = 60
history_max = 100

[audit]
max_entries = 100000

[git]
auto_commit = true

[cron]
tick_interval_secs = 60

[browser]
enabled = false
engine = "chromium"

[mcp.servers.fetch]
command = "uvx"
args = ["mcp-server-fetch"]
enabled = true
```

## Appendix B: CLI Quick Reference

```bash
# Build & Test
cargo build                          # Build everything
cargo test --workspace               # Run all tests
cargo clippy --workspace             # Lint

# Run
oxios                                # Start daemon (background)
oxios --foreground                   # Start in foreground

# Execute
oxios run --json "prompt"            # Single-shot, JSON output
oxios run --exit-code --json "…"     # Exit code: 0=passed, 1=failed
cat file.rs | oxios run --json --context-file - "describe this"

# Multi-turn
SID=$(oxios run --json "initial" | jq -r '.session_id')
oxios run --json --session "$SID" "follow-up"
```

## Appendix C: File Locations

| Path | Purpose |
|---|---|
| `~/.oxios/` | Oxios home directory |
| `~/.oxios/config.toml` | Main configuration |
| `~/.oxios/workspace/` | Agent working directory |
| `~/.oxios/workspace/sessions/` | Session data |
| `~/.oxios/workspace/seeds/` | Ouroboros seed specs |
| `~/.oxios/workspace/programs/` | Installed programs |
| `~/.oxios/workspace/skills/` | Skill definitions |
| `~/.oxios/spaces/` | Space data and workspaces |
| `~/.oxi/auth.json` | oxi-cli credentials |

---

*This document is maintained as the single source of truth for Oxios system
architecture. When modifying kernel structure or adding modules, update this
document accordingly.*
