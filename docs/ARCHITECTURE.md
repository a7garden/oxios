# Oxios Architecture Reference

> **Authoritative reference** for understanding how the Oxios Agent OS works internally.
> Read this before modifying kernel structure, adding subsystems, or extending the API surface.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Layer Architecture](#2-layer-architecture)
3. [Kernel Subsystems](#3-kernel-subsystems)
4. [KernelHandle — The System Call Facade](#4-kernelhandle--the-system-call-facade)
5. [Data Flow](#5-data-flow)
6. [Dependency Graph](#6-dependency-graph)
7. [Security Model](#7-security-model)
8. [Unix Philosophy Mapping](#8-unix-philosophy-mapping)
9. [Dependency Rules](#9-dependency-rules)

---

## 1. Overview

### What is Oxios?

Oxios is an **Agent Operating System** written in Rust. It is an OS where AI agents execute real work on behalf of users — fork, exec, wait, kill — just like Unix processes. Instead of managing CPU cores and memory pages, Oxios manages LLM-powered agents, their lifecycles, inter-agent communication, and persistent state.

**Stack:** Rust 2021 · tokio async · serde (JSON + TOML) · oxi-sdk + oxi-ai (crates.io)
**Scale:** ~52K lines across 179 source files

```
User → Channel (Web/CLI/Telegram) → Gateway → Kernel (supervisor + scheduler + ouroboros + agent_runtime)
```

### Design Philosophy

Oxios is built on two foundational ideas:

**1. Unix Philosophy Applied to Agents**

Every component does one thing well. Small tools compose into powerful workflows. The kernel exposes a clean system call interface; applications compose those calls; the internals are hidden behind the facade.

**2. Ouroboros Protocol — Spec-First Execution**

Never execute without a spec. The Ouroboros protocol guarantees that every agent action follows:

```
Interview → Seed → Execute → Evaluate → Evolve
```

A user's ambiguous request is first interviewed until clarity is sufficient, then crystallized into a **Seed** (a formal specification), executed by an agent, evaluated for success, and evolved if the result doesn't meet acceptance criteria.

### Key Principles

| Principle | Meaning |
|-----------|---------|
| **Unix philosophy** | Every component does one thing. Compose small pieces. |
| **Ouroboros first** | Never execute without a spec. Interview → seed → execute → evaluate → evolve. |
| **No reimplementation** | Reuse oxi-sdk. Never reimplement what oxi already provides. |
| **Channel agnostic** | Gateway doesn't care where messages come from. |
| **User invisible** | Users don't know how many agents are running. They talk, the OS handles the rest. |
| **No containers** | Direct host execution. Security via AccessManager (RBAC + path sandboxing). |

---

## 2. Layer Architecture

Oxios is organized into five layers. Each layer only depends on the layer directly below it (with the Engine as a horizontal dependency used by both Kernel and Runtime).

```
┌─────────────────────────────────────────────────────────────────────┐
│                            TERMINAL                                  │
│               Web  │  CLI  │  Telegram  │  Slack                     │
│                                                                      │
│   The user's point of entry. "Connecting to a terminal" means       │
│   opening the web dashboard, running the CLI, or sending a          │
│   message through a chat integration.                                │
└────────────────────────────┬────────────────────────────────────────┘
                             │ user request
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          APPLICATION                                 │
│           code-review  │  deploy  │  monitor  │  git-sync             │
│                                                                      │
│   Complete workflows composed from Kernel System Calls.              │
│   Dynamically loaded: install via program.toml + SKILL.md.           │
│   Each Application uses ONLY System Calls.                           │
│   Kernel internals are invisible.                                    │
└────────────────────────────┬────────────────────────────────────────┘
                             │ kernel.save(), kernel.spawn(), ...
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                            KERNEL                                    │
│                                                                      │
│  ┌── System Call Interface (pub fn) ─────────────────────────────┐  │
│  │                                                                │  │
│  │   save()   load()   delete()        State management           │  │
│  │   spawn()  wait()   kill()         Agent lifecycle             │  │
│  │   remember()  recall()             Memory                      │  │
│  │   commit()  tag()   restore()       Version control            │  │
│  │   schedule()  unschedule()         Scheduling                  │  │
│  │   audit()   verify()               Audit                       │  │
│  │   exec()                             Execution                  │  │
│  │   subscribe()                       Events                      │  │
│  │   resources()  check_budget()       Resources                  │  │
│  │                                                                │  │
│  │   The ONLY interface used by Application, Daemon, Terminal.    │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌── Subsystems (pub(crate)) ────────────────────────────────────┐  │
│  │                                                                │  │
│  │   Supervisor         Agent lifecycle (the "init")              │  │
│  │   Orchestrator       Ouroboros protocol execution              │  │
│  │   AgentLifecycleMgr  Full orchestrated lifecycle               │  │
│  │   AgentRuntime       oxi-agent tool-calling loop wrapper       │  │
│  │   Scheduler          Priority queue, rate-limit, zombie reap   │  │
│  │   StateStore         Persistent state (JSON on disk)           │  │
│  │   EventBus           Broadcast events (tokio broadcast)        │  │
│  │   AccessManager      RBAC, path sandboxing, audit log          │  │
│  │   AuditTrail         Merkle-chain tamper-evident log (blake3)  │  │
│  │   BudgetManager      Token/cost limits per agent               │  │
│  │   ResourceMonitor    CPU/memory tracking                       │  │
│  │   GitLayer           In-process version control (gix)          │  │
│  │   CronScheduler      Time-based job execution                  │  │
│  │   MemoryManager      Vector store, TF-IDF, HNSW               │  │
│  │   ProgramManager     Installable programs                      │  │
│  │   SkillStore         Markdown instruction templates            │  │
│  │   PersonaManager     Multiple AI characters                    │  │
│  │   McpBridge          Model Context Protocol                    │  │
│  │   A2AProtocol        Agent-to-agent communication              │  │
│  │   SpaceManager       Conversation context partitioning         │  │
│  │   CircuitBreaker     LLM provider failure protection           │  │
│  │   HostToolValidator  Host tool availability checking            │  │
│  │   AuthManager        Authentication                            │  │
│  │   WasmSandbox        WASM-based untrusted code execution       │  │
│  │   CredentialStore    Multi-source credential resolution        │  │
│  │   ContextManager     3-tier context hierarchy                  │  │
│  │                                                                │  │
│  │   NOT directly accessible from outside the Kernel.             │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
└────────────────────────────┬────────────────────────────────────────┘
                             │ kernel.spawn() → Agent created
                             │ kernel.exec()  → ExecTool runs
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                           RUNTIME                                    │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    ENGINE (oxi)                               │   │
│  │                                                              │   │
│  │  oxi-ai:     LLM provider, streaming, context window         │   │
│  │  oxi-agent:  Agent loop, tool registry, compaction            │   │
│  │  oxi-sdk:    Unified SDK wrapping oxi-ai + oxi-agent          │   │
│  │                                                              │   │
│  │  The core engine where agents "think and use tools".          │   │
│  │  Horizontal dependency — used by both Kernel and Runtime.     │   │
│  └──────────────────────────────┬───────────────────────────────┘   │
│                                  │                                   │
│       ┌──────────────────────────┼──────────────────────────┐       │
│       ▼                          ▼                          ▼       │
│  ┌──────────┐            ┌───────────┐            ┌──────────┐     │
│  │  Agent   │            │ Workspace │            │   Host   │     │
│  │          │            │           │            │          │     │
│  │  (oxi)   │            │   bash    │            │  macOS   │     │
│  │  LLM     │            │  direct   │            │  git/gh  │     │
│  │  tool    │            │  exec     │            │  osacript│     │
│  │  calls   │            │ build/test│            │          │     │
│  └──────────┘            └───────────┘            └──────────┘     │
│                                                                      │
│  Workspace: directory-based sandbox via AccessManager                │
│  Host:      ExecTool (allowlist + metacharacter blocking)            │
└─────────────────────────────────────────────────────────────────────┘
```

### Layer Summary Table

```
Layer        Implementation                  Role
───────────  ──────────────────────────────  ──────────────────────────────────
Terminal     oxios-web, oxios-cli,           User interface (entry point)
             oxios-telegram
Application  ProgramManager, .programs/      Workflow composition (System Calls)
Kernel       oxios-kernel                    Management (state, schedule, audit, VCS)
Runtime      Agent + Workspace + Host        Execution (actual work)
Engine       oxi-sdk + oxi-ai + oxi-agent    Core LLM/agent engine (horizontal dep)
```

---

## 3. Kernel Subsystems

The kernel contains 20+ subsystems, each with a single responsibility. They are private (`pub(crate)`) and only accessible through the KernelHandle facade.

### 3.1 Supervisor — Agent Lifecycle (The "init" of Oxios)

**File:** `crates/oxios-kernel/src/supervisor.rs`

The Supervisor is the process manager, analogous to Unix `init`. It handles the most fundamental agent lifecycle operations: fork, exec, wait, and kill.

```
                    ┌─────────────┐
                    │  Supervisor  │
                    │   (trait)    │
                    └──────┬──────┘
                           │ implements
                    ┌──────▼──────┐
                    │    Basic    │
                ┌──│  Supervisor  │──────────────┐
                │  └─────────────┘               │
                │                                │
      ┌─────────▼─────────┐          ┌──────────▼──────────┐
      │   Agent Registry   │          │   AgentRuntime      │
      │   HashMap<Id,Info> │          │   (oxi-agent loop)  │
      │                    │          │                     │
      │   • agents         │          │   • provider        │
      │   • handles        │          │   • model_id        │
      │     (cancellation) │          │   • kernel_handle   │
      └────────────────────┘          └─────────────────────┘
```

**Operations:**

| Operation | Unix Analog | Description |
|-----------|-------------|-------------|
| `fork(spec)` | `fork()` | Create a new agent from a Seed specification |
| `exec(id)` | `exec()` | Start executing an agent |
| `run_with_seed(id, seed)` | `fork()+exec()` | Fork and execute atomically, run to completion |
| `wait(id)` | `waitpid()` | Wait for agent to complete, return final status |
| `kill(id)` | `kill()` | Terminate an agent cooperatively (cancellation token) + abort task |
| `list()` | `ps` | List all known agents with their status |

**Key details:**
- Each agent gets a `JoinHandle` (tokio task) and an `AtomicBool` cancellation token
- Kill sets the cancellation flag and aborts the tokio task
- Delegates actual tool-calling to `AgentRuntime`

### 3.2 Orchestrator — Ouroboros Protocol Execution (The "Brain")

**File:** `crates/oxios-kernel/src/orchestrator.rs`

The Orchestrator coordinates the full Ouroboros lifecycle. It is the "brain" that takes a user message and drives it through the spec-first protocol.

```
User Message
      │
      ▼
┌─────────────┐    ambiguity too high    ┌──────────────┐
│  INTERVIEW   │─────────────────────────→│  Ask user     │
│              │◄─────────────────────────│  clarifying   │
└──────┬───────┘    user response         │  questions    │
       │                                    └──────────────┘
       │ ambiguity low enough
       ▼
┌─────────────┐
│    SEED      │   Generate formal specification
│  Generation  │   (goal, constraints, acceptance criteria)
└──────┬───────┘
       │
       ▼
┌─────────────┐     ┌──────────────────┐
│   EXECUTE    │────→│  AgentRuntime     │
│              │     │  (tool-calling)   │
└──────┬───────┘     └──────────────────┘
       │
       ▼
┌─────────────┐     failed     ┌──────────────┐
│  EVALUATE    │──────────────→│    EVOLVE     │──→ back to SEED
│              │               │  (mutate spec)│    (max 3 loops)
└──────┬───────┘               └──────────────┘
       │ passed
       ▼
    Result
```

**Orchestrator responsibilities:**
- Receives messages from Gateway (channel-agnostic)
- Manages Ouroboros sessions (interview state, seed generation, evaluation)
- Supports multi-agent execution via `AgentGroup` — splits seeds into subtasks
- Integrates with `SpaceManager` for automatic context routing
- Commits results to `GitLayer` for versioned persistence
- Publishes events to `EventBus` at each phase transition

### 3.3 AgentLifecycleManager — Full Orchestrated Lifecycle

**File:** `crates/oxios-kernel/src/agent_lifecycle.rs`

Extracted from the Orchestrator to avoid a god-object. Manages the complete lifecycle from fork to cleanup:

```
spawn_and_run(seed, priority)
       │
       ▼
  ┌─────────────┐
  │  1. Fork     │   supervisor.fork(seed)
  └──────┬──────┘
         ▼
  ┌─────────────┐
  │  2. Register │   a2a.register(card)
  │     A2A      │   Build AgentCard from seed (infer capabilities)
  └──────┬──────┘
         ▼
  ┌─────────────┐
  │  3. Deliver  │   a2a.deliver_pending_messages()
  │   pending    │   Inject any queued inter-agent messages
  └──────┬──────┘
         ▼
  ┌─────────────┐
  │  4. Ensure   │   access_manager.get_or_create_permissions()
  │  permissions │   Grant default tool access (bash, read, write, etc.)
  └──────┬──────┘
         ▼
  ┌─────────────┐
  │  5. Submit   │   scheduler.submit(task)
  │   to queue   │   scheduler.start_task(task_id)
  └──────┬──────┘
         ▼
  ┌─────────────┐
  │  6. Run      │   supervisor.run_with_seed(id, seed)
  │   (timeout)  │   Enforced by max_execution_time_secs
  └──────┬──────┘
         ▼
  ┌─────────────┐
  │  7. Cleanup  │   a2a.unregister() + scheduler.complete/fail()
  └─────────────┘
```

### 3.4 AgentRuntime — Tool-Calling Loop Wrapper

**File:** `crates/oxios-kernel/src/agent_runtime.rs`

Wraps `oxi-sdk`'s `AgentLoop` — the multi-turn LLM tool-calling engine. This is where agents actually "think" and "use tools."

```
┌───────────────────────────────────────────────┐
│                AgentRuntime                    │
│                                                │
│  ┌──────────────┐  ┌────────────────────────┐  │
│  │  oxi-agent   │  │    ToolRegistry        │  │
│  │  AgentLoop   │  │                        │  │
│  │              │  │  register_tools_from_   │  │
│  │  • provider  │  │  cspace(cspace, handle) │  │
│  │  • model     │  │                        │  │
│  │  • config    │  │  OS tools: exec, read,  │  │
│  │              │  │    write, edit, grep... │  │
│  └──────┬───────┘  │                        │  │
│         │          │  Kernel tools: agent,   │  │
│         │          │    space, persona...    │  │
│         │          │                        │  │
│         │          │  MCP tools: dynamic     │  │
│         │          │    from McpBridge       │  │
│         │          └────────────────────────┘  │
│         │                                       │
│         ▼                                       │
│  ┌──────────────────────────────────────────┐   │
│  │            CircuitBreaker                │   │
│  │  Global LLM circuit breaker              │   │
│  │  (Closed → Open → Half-Open)             │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  All tool access → KernelHandle (single path)    │
└──────────────────────────────────────────────────┘
```

**Key details:**
- Configures `AgentLoop` with `AgentLoopConfig` (max iterations, tool execution mode, space ID)
- Resolves CSpace (capability space) from persona/role/hint
- Registers tools dynamically via `register_tools_from_cspace()`
- Optionally queries `ToolRetriever` for semantic capability discovery
- Protected by global `CircuitBreaker` against LLM provider failures
- Uses `spawn_blocking` because `AgentLoop::run()` produces a `!Send` future

### 3.5 Scheduler — Priority Task Queue

**File:** `crates/oxios-kernel/src/scheduler.rs`

Priority-based task queue inspired by AIOS / AgentRM research.

```
                    ┌──────────────────────┐
                    │      Scheduler        │
                    └──────────┬───────────┘
                               │
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
  ┌─────────────┐    ┌─────────────────┐    ┌──────────────┐
  │   Priority   │    │  Rate Limiter   │    │    Zombie     │
  │    Queue     │    │                 │    │   Detector    │
  │              │    │  per-minute     │    │              │
  │  Critical(3) │    │  admission      │    │  reap tasks  │
  │  High(2)     │    │  control        │    │  stuck beyond │
  │  Normal(1)   │    │                 │    │  timeout      │
  │  Low(0)      │    │                 │    │              │
  └─────────────┘    └─────────────────┘    └──────────────┘
         │
         ▼
  ┌──────────────────────────────────────┐
  │   Max Concurrent Enforcement         │
  │   (configurable limit)               │
  └──────────────────────────────────────┘
```

**Priority levels:**

| Priority | Value | Use Case |
|----------|-------|----------|
| Critical | 3 | Must execute immediately |
| High | 2 | Important user-facing tasks |
| Normal | 1 | Default for most tasks |
| Low | 0 | Background work |

**Features:**
- BinaryHeap-based priority queue (higher priority first, LIFO within same priority)
- Rate-limit-aware admission control (configurable per-minute limit)
- Zombie task detection: tasks stuck beyond `zombie_timeout_secs` are automatically reaped
- Maximum concurrent task enforcement
- Full task lifecycle: `Queued → Running → Completed/Failed/Cancelled`

### 3.6 StateStore — Persistent State

**File:** `crates/oxios-kernel/src/state_store.rs`

JSON-on-disk persistent storage. Every piece of state — sessions, audit entries, memory, seeds — is persisted through the StateStore.

```
~/.oxios/workspace/
    ├── sessions/          ← Session data (ephemeral)
    │   └── {id}.json
    ├── seeds/             ← Ouroboros seed specs
    │   └── {id}.json
    ├── programs/          ← Installed programs
    ├── skills/            ← Skill definitions
    ├── memory/            ← Agent memory entries
    │   └── {id}.json
    ├── audit/             ← Audit trail entries
    │   └── entries.json
    └── state/             ← General-purpose state
        └── {category}/{name}.json
```

**Operations:** `save()`, `load()`, `delete()`, `list()`, `save_markdown()`, `load_audit_entries()`, `commit_all()`

### 3.7 EventBus — Broadcast Events

**File:** `crates/oxios-kernel/src/event_bus.rs`

The "pipe" of Oxios. All agents and subsystems communicate through kernel events published on the bus.

```
┌─────────────┐     publish()     ┌──────────────┐    receive()    ┌──────────────┐
│  Subsystem A │────────────────→│   EventBus    │──────────────→│  Subsystem B │
│  (publisher) │                  │              │                 │ (subscriber) │
└─────────────┘                  │  tokio::      │                 └──────────────┘
                                  │  broadcast    │
┌─────────────┐     publish()     │              │    receive()    ┌──────────────┐
│  Subsystem C │────────────────→│              │──────────────→│  Subsystem D │
│  (publisher) │                  │              │                 │ (subscriber) │
└─────────────┘                  └──────────────┘                 └──────────────┘
                                         │
                                         ▼
                                  ┌──────────────┐
                                  │  AuditTrail  │
                                  │  (attached)  │
                                  └──────────────┘
```

**Kernel Events:**

| Event | Description |
|-------|-------------|
| `AgentCreated` | New agent forked |
| `AgentStarted` | Agent begins executing |
| `AgentStopped` | Agent terminated |
| `AgentFailed` | Agent encountered error |
| `MessageReceived` | Inter-agent message |
| `SeedCreated` | New seed specification |
| `EvaluationComplete` | Evaluation result |
| `PhaseStarted` | Ouroboros phase begins |
| `PhaseCompleted` | Ouroboros phase ends |
| `AgentOutput` | Agent produced output |

### 3.8 AccessManager — RBAC & Path Sandboxing

**File:** `crates/oxios-kernel/src/access_manager/`

OWASP-inspired least-privilege security. Every agent starts with minimal permissions and must be explicitly granted access.

```
┌───────────────────────────────────────────────────────────────┐
│                      AccessManager                             │
│                                                                │
│  ┌──────────────────┐  ┌──────────────────┐  ┌─────────────┐ │
│  │  Agent Perms      │  │  RBAC Manager    │  │  Workspace  │ │
│  │                   │  │                  │  │  Sandbox    │ │
│  │  • tool access    │  │  • roles         │  │             │ │
│  │  • path allowlist │  │  • policies      │  │  • path     │ │
│  │  • network access │  │  • approvals     │  │    binding  │ │
│  │  • rate limits    │  │  • HitL          │  │  • agent    │ │
│  │                   │  │    (human-in-    │  │    assign   │ │
│  └──────────────────┘  │     the-loop)    │  │             │ │
│                         └──────────────────┘  └─────────────┘ │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                    Audit Log                              │ │
│  │  Every access decision logged with agent, action, result │ │
│  └──────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
```

**Security model:**
- **Least privilege by default:** agents get minimal permissions on creation
- **Tool access control:** which tools each agent can use
- **Path sandboxing:** agents can only access files within their assigned workspace
- **RBAC policies:** role-based access with human-in-the-loop approval for sensitive operations
- **Audit logging:** every security-relevant decision is recorded

### 3.9 AuditTrail — Merkle-Chain Tamper-Evident Log

**File:** `crates/oxios-kernel/src/audit_trail.rs`

Merkle-chain style audit log where each entry is cryptographically linked to the previous one using BLAKE3.

```
Entry₁              Entry₂              Entry₃
┌────────────┐     ┌────────────┐     ┌────────────┐
│ action     │     │ action     │     │ action     │
│ agent_id   │     │ agent_id   │     │ agent_id   │
│ timestamp  │     │ timestamp  │     │ timestamp  │
│ prev_hash ─┼────→│ prev_hash ─┼────→│ prev_hash  │
│ hash(BLAKE3)│     │ hash(BLAKE3)│     │ hash(BLAKE3)│
└────────────┘     └────────────┘     └────────────┘
```

- Each entry's hash is computed over `{prev_hash} | {action} | {agent_id} | {timestamp} | {details}`
- Chain verification: `verify_chain()` checks that every entry's `prev_hash` matches the preceding entry's hash
- Guardian daemon periodically verifies the chain and alerts on corruption
- Persisted via StateStore and restored on startup

### 3.10 BudgetManager — Token/Cost Limits

**File:** `crates/oxios-kernel/src/budget.rs`

Enforces per-agent resource budgets to prevent runaway LLM costs.

```
┌────────────────────────────────────┐
│         BudgetManager              │
│                                    │
│  BudgetKind:                       │
│  ┌────────────────────────────┐   │
│  │  TokenLimit { max_tokens } │   │
│  │  CostLimit  { max_cost }   │   │
│  │  TurnsLimit { max_turns }  │   │
│  └────────────────────────────┘   │
│                                    │
│  Per-agent tracking:               │
│  • check(agent, kind) → bool       │
│  • consume(agent, amount)          │
│  • remaining(agent) → BudgetInfo   │
│                                    │
│  Raises BudgetExceeded when over   │
└────────────────────────────────────┘
```

### 3.11 ResourceMonitor — System Resource Tracking

**File:** `crates/oxios-kernel/src/resource_monitor.rs`

Tracks CPU and memory usage for agent budget enforcement and overload detection.

```
┌────────────────────────────────────┐
│        ResourceMonitor             │
│                                    │
│  • cpu_percent: f64                │
│  • memory_used_mb: f64             │
│  • memory_total_mb: f64            │
│  • history: Vec<ResourceSnapshot>  │
│                                    │
│  is_overloaded() → bool            │
│  (checks against OverloadThreshold)│
│                                    │
│  Polls at configurable interval    │
│  Retains last N snapshots          │
└────────────────────────────────────┘
```

### 3.12 GitLayer — In-Process Version Control

**File:** `crates/oxios-kernel/src/git_layer.rs`

In-process version control using `gix` (pure Rust git implementation). No external git binary required.

```
┌────────────────────────────────────┐
│           GitLayer                 │
│                                    │
│  Workspace: ~/.oxios/workspace/    │
│                                    │
│  commit_file(path, msg)            │
│  commit_all(msg)                   │
│  remove_file(path, msg)            │
│  log(limit) → Vec<LogEntry>        │
│  tag(name, message)                │
│  restore(path, ref_spec)           │
│  verify() → bool                   │
│  is_enabled() → bool               │
│                                    │
│  Auto-commit on state changes      │
│  Guardian verifies repo integrity  │
└────────────────────────────────────┘
```

### 3.13 CronScheduler — Scheduled Jobs

**File:** `crates/oxios-kernel/src/cron.rs`

Time-based job execution with persistent state via StateStore.

```
┌────────────────────────────────────┐
│        CronScheduler               │
│                                    │
│  add_cron(job) → job_id            │
│  remove_cron(id) → Result          │
│  list_crons() → Vec<CronJob>       │
│  tick() → Vec<CronJobResult>       │
│                                    │
│  CronJob:                          │
│  • id, expression, task            │
│  • source (User/Program/System)    │
│  • last_run, next_run              │
│                                    │
│  Tick interval: configurable       │
│  Integrates with GitLayer          │
└────────────────────────────────────┘
```

### 3.14 MemoryManager — Vector Store & Semantic Search

**File:** `crates/oxios-kernel/src/memory/`

Persistent agent memory with multiple indexing strategies.

```
┌───────────────────────────────────────────────────────────┐
│                     MemoryManager                          │
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │  Vector Store │  │    TF-IDF     │  │     HNSW      │  │
│  │               │  │   Embedding   │  │    Index       │  │
│  │  • entries    │  │               │  │               │  │
│  │  • JSON disk  │  │  • tokenize   │  │  • approximate │  │
│  │               │  │  • normalize  │  │    nearest     │  │
│  │               │  │  • cosine sim │  │    neighbor    │  │
│  └──────────────┘  └──────────────┘  └───────────────┘  │
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │  Hyperbolic  │  │    Flash      │  │   Reasoning   │  │
│  │  Embeddings  │  │   Attention   │  │     Bank      │  │
│  │  (Poincaré)  │  │               │  │               │  │
│  └──────────────┘  └──────────────┘  └───────────────┘  │
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │  Sona        │  │  RVF Store   │  │  Auto Memory  │  │
│  │  Learning    │  │              │  │  Bridge       │  │
│  │  Engine      │  │              │  │               │  │
│  └──────────────┘  └──────────────┘  └───────────────┘  │
│                                                           │
│  MemoryEntry { id, content, memory_type, embedding, ... } │
│  MemoryType: Fact | Procedure | Episode | Insight | Meta  │
└───────────────────────────────────────────────────────────┘
```

**Key components:**
- **TF-IDF Embedding:** No external model needed. Works for any language including Korean.
- **HNSW Index:** Approximate nearest neighbor for fast semantic search
- **Hyperbolic Embeddings:** Poincaré ball model for hierarchical concept relationships
- **Flash Attention:** Memory-efficient attention mechanism for context ranking
- **Reasoning Bank:** Stores agent reasoning chains for future reference
- **Sona Learning Engine:** Adaptive learning from agent experiences
- **RVF Store:** Retrieval-Verification-Factuality store
- **Auto Memory Bridge:** Automatic import/export of insights with curation

### 3.15 ProgramManager — Installable Programs

**File:** `crates/oxios-kernel/src/program/`

Programs are installable capabilities that extend the agent OS. Think of them as "packages" in a package manager.

```
┌─────────────────────────────────────────────────────┐
│                  ProgramManager                      │
│                                                      │
│  install(path) → Result                             │
│  uninstall(name) → Result                           │
│  get_program(name) → Option<Program>                │
│  list_enabled() → Vec<Program>                      │
│  init() → Result                                    │
│                                                      │
│  Program structure:                                  │
│  ┌──────────────────────────────────────────────┐   │
│  │  program.toml (metadata)                      │   │
│  │  ├── name, description, version               │   │
│  │  ├── tools: [{name, command, description}]    │   │
│  │  ├── mcp_servers: [{name, command, args}]     │   │
│  │  └── host_requirements: {required, optional}  │   │
│  │                                                │   │
│  │  SKILL.md (instructions for the agent)         │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
│  Built-in programs:                                  │
│  code-review, debug, deploy, guardian,               │
│  refactor, program-creator                           │
└─────────────────────────────────────────────────────┘
```

### 3.16 SkillStore — Markdown Instruction Templates

**File:** `crates/oxios-kernel/src/skill.rs`

Skills are markdown instruction templates that teach agents how to perform specific tasks.

```
┌────────────────────────────────┐
│         SkillStore             │
│                                │
│  Workspace: skills/            │
│                                │
│  init_defaults(share_dir)      │
│  get(name) → Option<Skill>     │
│  list() → Vec<SkillMeta>       │
│                                │
│  Skill:                        │
│  • name                        │
│  • description                 │
│  • instructions (markdown)     │
│  • triggers (keywords)         │
└────────────────────────────────┘
```

### 3.17 PersonaManager — Multiple AI Characters

**File:** `crates/oxios-kernel/src/persona_manager.rs`

Manages multiple AI personas with distinct system prompts and behaviors.

```
┌────────────────────────────────┐
│      PersonaManager            │
│                                │
│  first_enabled() → Option<P>   │
│  get(name) → Option<&Persona>  │
│  list() → Vec<&Persona>        │
│  set_active(name)              │
│                                │
│  Persona:                      │
│  • name                        │
│  • system_prompt               │
│  • description                 │
│  • enabled: bool               │
│                                │
│  Active persona prompt set on  │
│  OuroborosEngine at startup    │
└────────────────────────────────┘
```

### 3.18 McpBridge — Model Context Protocol

**File:** `crates/oxios-kernel/src/mcp/`

Bridges external MCP servers into the Oxios tool ecosystem. MCP is the vertical protocol (agent → external tool).

```
┌─────────────────────────────────────────────────────────┐
│                     McpBridge                            │
│                                                          │
│  register_server(McpServer)                              │
│  initialize_all() → Result                               │
│  list_tools() → Vec<McpTool>                             │
│  call_tool(name, args) → McpToolCallResult               │
│                                                          │
│  ┌───────────────────┐  ┌───────────────────┐           │
│  │   MCP Client      │  │   MCP Protocol    │           │
│  │                   │  │                   │           │
│  │  Per-server       │  │  JSON-RPC over    │           │
│  │  stdio transport  │  │  stdin/stdout     │           │
│  └───────────────────┘  └───────────────────┘           │
│                                                          │
│  Sources:                                                │
│  • config.toml [mcp.servers]                             │
│  • Environment vars (OXIOS_MCP_*_COMMAND)                │
│  • Program MCP servers (program.meta.mcp_servers)        │
└─────────────────────────────────────────────────────────┘
```

### 3.19 A2AProtocol — Agent-to-Agent Communication

**File:** `crates/oxios-kernel/src/a2a.rs`

Google's A2A protocol for horizontal agent↔agent communication. Unlike MCP (vertical: agent→tool), A2A enables agents to discover each other, delegate tasks, and share results.

```
┌─────────────────────────────────────────────────────────┐
│                    A2AProtocol                           │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │              Agent Card Registry                    │ │
│  │                                                    │ │
│  │  register_agent(AgentCard)                         │ │
│  │  unregister_agent(id)                              │ │
│  │  find_by_capability(cap) → Vec<AgentCard>          │ │
│  │  get_agent(id) → Option<AgentCard>                 │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  Message Types:                                          │
│  ┌──────────────────┐  ┌──────────────────────┐        │
│  │ TaskDelegation   │  │ StatusUpdate          │        │
│  │ "Here, do X"    │  │ "I'm at Y%, status Z" │        │
│  └──────────────────┘  └──────────────────────┘        │
│  ┌──────────────────┐  ┌──────────────────────┐        │
│  │ ResultSharing    │  │ CapabilityQuery      │        │
│  │ "Here's result" │  │ "Who can do X?"      │        │
│  └──────────────────┘  └──────────────────────┘        │
│  ┌──────────────────┐                                  │
│  │ Handshake        │  DelegationHandler:              │
│  │ "Hello, I do Y" │  Custom handler for task          │
│  └──────────────────┘  delegation (spawns agent)        │
└─────────────────────────────────────────────────────────┘
```

### 3.20 SpaceManager — Conversation Context Partitioning

**File:** `crates/oxios-kernel/src/space.rs`

Spaces provide isolated memory and workspace for different contexts (projects, topics, domains). The OS automatically routes user messages to the appropriate Space.

```
┌───────────────────────────────────────────────────────────┐
│                     SpaceManager                           │
│                                                           │
│  3-Layer Detection:                                       │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  Layer 1: Filesystem path detection                 │ │
│  │  "Fix the bug in /projects/oxios/src/main.rs"       │ │
│  │  → Route to Space bound to /projects/oxios/         │ │
│  └─────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  Layer 2: Keyword matching                          │ │
│  │  Tags on each Space matched against message         │ │
│  └─────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  Layer 3: LLM topic classification                  │ │
│  │  Fallback when path/keyword don't match             │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                           │
│  Space:                                                   │
│  • id, name, source (Auto/Manual)                        │
│  • paths (filesystem binding)                             │
│  • workspace_dir (scratch space)                          │
│  • tags (for keyword matching)                            │
│  • ConversationBuffer (scoped message history)            │
│  • KnowledgeBridge (auto knowledge extraction)            │
└───────────────────────────────────────────────────────────┘
```

### 3.21 CircuitBreaker — LLM Provider Protection

**File:** `crates/oxios-kernel/src/circuit_breaker.rs`

3-state circuit breaker protecting against cascading LLM provider failures.

```
          success              threshold failures
    ┌──────────────┐       ┌───────────────────────┐
    │              │       │                       │
    │   ┌─────┐   │       │   ┌─────┐             │
    │   │CLOSED│   │       │   │CLOSED│────────────┼───→  ┌──────┐
    │   └──┬──┘   │       │   └─────┘             │      │ OPEN │
    │      │      │       │                       │      └──┬───┘
    │      │      │       │                       │         │
    │   success   │       │   timeout elapsed     │         │
    │      │      │       │       ┌───────┐       │         │
    │      ▼      │       │       │HALF-  │◄──────┼─────────┘
    │  (normal)   │       │       │OPEN   │       │
    │             │       │       └───┬───┘       │
    │             │       │           │           │
    │             │       │    ┌──────┴──────┐    │
    │             │       │  success    failure │  │
    │             │       │    │           │     │  │
    │             │       │    ▼           ▼     │  │
    │             │       │  CLOSED      OPEN   │  │
    │             │       │                    │  │
    └──────────────┘       └────────────────────┘

    Closed:  All requests pass through (normal)
    Open:    Requests rejected immediately (provider failing)
    HalfOpen: One probe request tests recovery
```

### 3.22 HostToolValidator — Host Tool Checking

**File:** `crates/oxios-kernel/src/host_tools.rs`

Validates that required and optional host tools are available on the system.

```
┌──────────────────────────────────────────────┐
│           HostToolValidator                   │
│                                               │
│  Required tools (MUST be present):            │
│  • git, ...                                   │
│                                               │
│  Optional tools (MAY be present):             │
│  • gh, osascript, ...                         │
│                                               │
│  validate_required() → Vec<missing>           │
│  check_optional() → HashMap<tool, bool>       │
│  full_check() → HostToolStatus               │
│                                               │
│  "Minimal container, host dependency"         │
│  philosophy — container ships only essential   │
│  tools; additional capabilities come from     │
│  the host system                              │
└──────────────────────────────────────────────┘
```

### 3.23 AuthManager — Authentication

**File:** `crates/oxios-kernel/src/auth.rs`

Manages authentication for the Oxios system. Used by `KernelHandle` for identity verification.

```
┌────────────────────────────────────┐
│         AuthManager                │
│                                    │
│  API key auth via:                 │
│  • engine.api_key (config.toml)    │
│  • ~/.oxi/auth.json (oxi CLI)      │
│                                    │
│  Key tracking:                     │
│  • key metadata (created, last_use)│
│  • validation                      │
└────────────────────────────────────┘
```

### 3.24 WasmSandbox — WASM-Based Sandboxing

**File:** `crates/oxios-kernel/src/wasm_sandbox.rs`

WebAssembly sandbox using wasmtime for safely executing untrusted tool code with resource limits.

```
┌──────────────────────────────────────────────┐
│             WasmSandbox                       │
│                                               │
│  Feature-gated: wasm-sandbox                  │
│                                               │
│  Resource limits:                             │
│  • Memory limit                               │
│  • Instruction count limit                    │
│  • Module size limit                          │
│                                               │
│  WasmConfig { memory_limit, fuel_limit, ... } │
│  execute(module, function, args) → Result     │
│                                               │
│  Errors: ModuleNotFound, FunctionNotFound,    │
│          ExecutionFailed, OutOfResources      │
└──────────────────────────────────────────────┘
```

### 3.25 CredentialStore — Multi-Source Credential Resolution

**File:** `crates/oxios-kernel/src/credential.rs`

Resolves API keys from multiple sources with clear priority:

```
  Priority Order
  ══════════════

  1. config.toml ──── [engine].api_key
         │              (explicit override)
         │
  2. ~/.oxi/auth.json  (shared with oxi CLI)
         │
         │
  3. Environment var   (CI/CD, containers)
                        oxi-ai env var fallback

  resolve(provider, config_key) → Option<(key, source)>
  store(provider, api_key) → Result  (via onboarding wizard)
```

### 3.26 ContextManager — 3-Tier Context Hierarchy

The context system provides a 3-tier hierarchy for managing agent context:

```
┌────────────────────────────────────────────────────────┐
│                Context Hierarchy                        │
│                                                         │
│  Tier 1: System Context                                 │
│  ├── Persona system prompt                              │
│  ├── Global skills and instructions                     │
│  └── OS-level configuration                             │
│                                                         │
│  Tier 2: Space Context                                  │
│  ├── Space-specific knowledge                           │
│  ├── Conversation history (ConversationBuffer)          │
│  └── KnowledgeBridge auto-extracted facts               │
│                                                         │
│  Tier 3: Agent Context                                  │
│  ├── Current task specification (Seed)                  │
│  ├── Working memory                                     │
│  └── Tool results                                       │
│                                                         │
│  Context flows top-down: System → Space → Agent         │
│  Each tier can override/extend the tier above           │
└────────────────────────────────────────────────────────┘
```

---

## 4. KernelHandle — The System Call Facade

The `KernelHandle` is the primary API for all kernel operations. It implements the **Facade pattern**, composing 11 typed domain APIs into a single object. Every tool, program, and subcommand interacts with the kernel exclusively through `KernelHandle`.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          KernelHandle                                    │
│                                                                          │
│  ┌─────────────┐ ┌────────────┐ ┌──────────────┐ ┌──────────────┐      │
│  │  StateApi   │ │  AgentApi  │ │ SecurityApi  │ │ PersonaApi   │      │
│  │             │ │            │ │              │ │              │      │
│  │ save()      │ │ fork()     │ │ authenticate │ │ list()       │      │
│  │ load()      │ │ kill()     │ │ audit()      │ │ get()        │      │
│  │ delete()    │ │ list()     │ │ verify_chain │ │ set_active() │      │
│  │ sessions()  │ │ budget()   │ │ approve()    │ │ system_prompt│      │
│  └─────────────┘ └────────────┘ └──────────────┘ └──────────────┘      │
│                                                                          │
│  ┌──────────────┐ ┌──────────┐ ┌───────────┐ ┌──────────────────┐      │
│  │ ExtensionApi │ │  McpApi  │ │ InfraApi   │ │    SpaceApi      │      │
│  │              │ │          │ │            │ │                  │      │
│  │ programs()   │ │ list_    │ │ git()      │ │ create()        │      │
│  │ skills()     │ │   tools()│ │ scheduler  │ │ detect()        │      │
│  │ host_tools() │ │ call_    │ │ cron()     │ │ route()         │      │
│  │ install()    │ │   tool() │ │ resources  │ │ knowledge()     │      │
│  └──────────────┘ └──────────┘ │ events()   │ └──────────────────┘      │
│                                 │ config()   │                           │
│                                 └───────────┘                            │
│                                                                          │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐                      │
│  │  ExecApi    │ │ BrowserApi   │ │   A2aApi     │                      │
│  │             │ │              │ │              │                      │
│  │ exec_conf   │ │ headless     │ │ send()       │                      │
│  │ access_mgr  │ │ browser      │ │ receive()    │                      │
│  │ sandboxing  │ │ backend      │ │ delegate()   │                      │
│  └─────────────┘ └──────────────┘ │ discover()   │                      │
│                                    └──────────────┘                      │
│                                                                          │
│  Convenience methods (cross-API orchestration):                          │
│  • save_and_commit()   — State + Infra (Git)                            │
│  • delete_and_commit() — State + Infra (Git)                            │
│  • flush_audit()       — Security + Infra (Git)                         │
│  • commit_all()        — State + Infra (Git)                            │
│  • schedule()          — Infra (Cron)                                   │
│  • unschedule()        — Infra (Cron)                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

### The 11 Domain APIs

| API | Subsystem Access | Responsibility |
|-----|-----------------|---------------|
| `StateApi` | StateStore | Data persistence, sessions, load/save/delete |
| `AgentApi` | Supervisor, BudgetManager, MemoryManager | Agent lifecycle, budgets, memory |
| `SecurityApi` | AuthManager, AuditTrail, AccessManager, StateStore | Auth, audit, RBAC, approvals |
| `PersonaApi` | PersonaManager | Multi-persona management |
| `ExtensionApi` | ProgramManager, SkillStore, HostToolValidator | Programs, skills, host tools |
| `McpApi` | McpBridge | MCP server bridge |
| `InfraApi` | GitLayer, AgentScheduler, CronScheduler, ResourceMonitor, EventBus, Config | Infrastructure: Git, scheduling, resources, events |
| `SpaceApi` | SpaceManager, EventBus | Context partitioning, knowledge flow |
| `ExecApi` | ExecConfig, AccessManager | Execution configuration, sandboxing |
| `BrowserApi` | Feature-gated browser backend | Headless browser automation |
| `A2aApi` | A2AProtocol | Agent-to-agent communication |

### KernelHandle Creation

The `KernelHandle` is assembled in `kernel.rs` (the binary crate, not the library) during `KernelBuilder::build()`. It's cached via `OnceLock` — created once, reused forever.

```rust
// From src/kernel.rs — Kernel::handle()
pub fn handle(&self) -> Arc<KernelHandle> {
    self.handle_cache.get_or_init(|| {
        Arc::new(KernelHandle::new(
            StateApi::new(self.state_store.clone()),
            AgentApi::new(self.supervisor.clone(), ...),
            SecurityApi::new(self.auth_manager.clone(), ...),
            PersonaApi::new(Arc::new(self.persona_manager.clone())),
            ExtensionApi::new(self.program_manager.clone(), ...),
            McpApi::new(self.mcp_bridge.clone()),
            InfraApi::new(self.git_layer.clone(), ...),
            SpaceApi::new(self.space_manager.clone(), ...),
            ExecApi::new(Arc::new(self.config.exec.clone()), ...),
            self.build_browser_api(),
            A2aApi::new(self.a2a_protocol.clone()),
        ))
    }).clone()
}
```

---

## 5. Data Flow

### How a User Message Flows Through the System

```
User: "Review the code in src/main.rs and suggest improvements"
  │
  │  1. Terminal receives message
  ▼
┌─────────────────────────────────────────────────────────────┐
│  Channel (Web/CLI/Telegram)                                  │
│  • Accepts raw user message                                  │
│  • Wraps in channel-agnostic format                          │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │  2. Gateway routes message
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Gateway                                                     │
│  • Channel-agnostic message hub                              │
│  • Routes to Orchestrator                                    │
│  • Does NOT parse or interpret the message                   │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │  3. Orchestrator begins Ouroboros
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Orchestrator                                                │
│                                                              │
│  3a. Space Detection                                         │
│      SpaceManager.detect(message)                            │
│      → Layer 1: filesystem path match                       │
│      → Layer 2: keyword match                               │
│      → Layer 3: LLM topic classification                    │
│      → Route to matching Space (or create new)              │
│                                                              │
│  3b. Interview Phase                                         │
│      OuroborosEngine conducts interview with LLM            │
│      → Assess ambiguity in user's request                   │
│      → If ambiguous: ask clarifying questions               │
│      → If clear: proceed to Seed generation                 │
│                                                              │
│  3c. Seed Generation                                         │
│      Generate formal specification:                          │
│      { goal, constraints, acceptance_criteria, ontology }    │
│                                                              │
│  3d. Agent Spawning                                          │
│      AgentLifecycleManager.spawn_and_run(seed, priority)     │
│        ├── supervisor.fork(seed)                            │
│        ├── a2a.register(card)                               │
│        ├── access_manager.ensure_permissions()              │
│        ├── scheduler.submit(task)                           │
│        └── supervisor.run_with_seed(agent_id, seed)         │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │  4. Agent executes
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  AgentRuntime                                                │
│                                                              │
│  4a. Configure AgentLoop                                     │
│      • Resolve CSpace from persona/role                     │
│      • Register tools: exec, read, write, edit, grep,       │
│        memory, kernel tools, MCP tools                      │
│      • Set context: system prompt + space context + seed    │
│                                                              │
│  4b. Multi-turn tool-calling loop                            │
│      AgentLoop.run():                                        │
│      ┌──────────────────────────────────────────┐           │
│      │  while not done:                          │           │
│      │    LLM generates response                 │           │
│      │    if tool_call:                          │           │
│      │      execute tool via KernelHandle        │           │
│      │      append result to context             │           │
│      │    else:                                  │           │
│      │      return final response                │           │
│      └──────────────────────────────────────────┘           │
│                                                              │
│  4c. Tools execute via KernelHandle                          │
│      exec tool → ExecApi → AccessManager (RBAC check)       │
│      read/write → filesystem operations (sandboxed)         │
│      memory → MemoryManager → vector store                  │
│                                                              │
│  4d. CircuitBreaker guards LLM calls                         │
│      If provider fails repeatedly → circuit opens →          │
│      immediate rejection instead of timeout                  │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │  5. Result returns
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Orchestrator (Evaluation Phase)                             │
│                                                              │
│  5a. Evaluate result against acceptance criteria             │
│  5b. If FAILED and loops remaining → Evolve seed            │
│      Mutate spec, re-execute (up to max 3 loops)            │
│  5c. If PASSED → commit result                              │
│      git_layer.commit() → versioned persistence             │
│  5d. Publish events to EventBus                              │
│      AgentStopped, EvaluationComplete, etc.                 │
│  5e. Return OrchestrationResult                              │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │  6. Response to user
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Channel                                                     │
│  • Format result for the user's channel                     │
│  • Web: JSON API response                                    │
│  • CLI: stdout with JSON output                              │
│  • Telegram: chat message                                    │
└─────────────────────────────────────────────────────────────┘
```

### OrchestrationResult

The final result returned from the orchestrator:

```json
{
  "response": "...",
  "session_id": "uuid",
  "space_id": "uuid | null",
  "space_tag": "[emoji label] | null",
  "seed_id": "uuid | null",
  "agent_id": "uuid | null",
  "phase_reached": "Execute",
  "evaluation_passed": true,
  "exit_code": 0,
  "duration_ms": 3500
}
```

---

## 6. Dependency Graph

### Crate Dependency Diagram

```
                    ┌──────────────────────────────────────────────────────┐
                    │                     oxios (binary)                    │
                    │                  src/kernel.rs (assembly)             │
                    │                                                       │
                    │   Kernel::builder().build().await                     │
                    │   Wires all components together                       │
                    └──┬──────────┬──────────┬──────────┬──────────────────┘
                       │          │          │          │
          ┌────────────┘    │     ┌─┘     ┌──┘     ┌───┘
          ▼                 ▼     ▼       ▼        ▼
  ┌──────────────┐ ┌──────────────┐ ┌──────────┐ ┌──────────────┐
  │ oxios-kernel │ │oxios-ouroboros│ │oxios-gate│ │  Channel     │
  │              │ │              │ │  way     │ │  (feature)   │
  │ All 20+      │ │ Ouroboros    │ │          │ │              │
  │ subsystems   │ │ protocol:    │ │ Message  │ │ oxios-web    │
  │ + tools      │ │ interview →  │ │ routing  │ │ oxios-cli    │
  │ + Kernel     │ │ seed → exec  │ │          │ │ oxios-telegram│
  │ Handle       │ │ → evaluate   │ │ Channel  │ │              │
  │              │ │ → evolve     │ │ agnostic │ │ Feature-     │
  │ oxi-sdk ─────┼─┤              │ │          │ │ gated        │
  │ oxi-ai ──────┼─┤ oxi-sdk ─────┤ │ Gateway  │ │              │
  │              │ │              │ │ trait    │ │ Axum/Dioxus  │
  │ gix, blake3, │ │              │ │          │ │ Clap         │
  │ wasmtime, .. │ │              │ │          │ │ teloxide     │
  └──────────────┘ └──────────────┘ └──────────┘ └──────────────┘
          │
          │ (horizontal dependency)
          ▼
  ┌──────────────────────────────────────┐
  │           oxi-sdk (crates.io)        │
  │                                      │
  │  oxi-ai:    LLM provider, streaming  │
  │  oxi-agent: Agent loop, tool reg.    │
  │                                      │
  │  Used by both Kernel and Runtime     │
  └──────────────────────────────────────┘
```

### Detailed Crate → Layer Mapping

```
Crate                      Layer        Description
─────────────────────────  ──────────   ─────────────────────────────────
oxios/ (binary)            Kernel       Assembly: KernelBuilder wires all parts
oxios-kernel/              Kernel       All subsystems, tools, KernelHandle
oxios-ouroboros/           Kernel       Ouroboros spec-first protocol engine
oxios-gateway/             Kernel       Channel-agnostic message routing
oxios-web/                 Terminal     Web dashboard (Axum + Dioxus/WASM)
oxios-cli/                 Terminal     CLI channel (Clap)
oxios-telegram/            Terminal     Telegram channel (teloxide)
oxi-sdk (crates.io)        Engine      Unified SDK for oxi-ai + oxi-agent
oxi-ai (crates.io)         Engine      LLM provider construction
```

### Directory Structure

```
oxios/                          # Main binary (src/main.rs, src/kernel.rs, src/cmd_run.rs)
├── crates/
│   ├── oxios-kernel/           # Core: supervisor, scheduler, event bus, state store, tools, memory
│   ├── oxios-ouroboros/        # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   └── oxios-gateway/          # Channel-agnostic message hub
├── channels/
│   ├── oxios-web/              # Web dashboard (Axum backend + Dioxus/WASM frontend)
│   ├── oxios-cli/              # CLI channel
│   └── oxios-telegram/         # Telegram channel
├── .programs/                  # OS-level programs (code-review, debug, deploy, guardian, refactor, ...)
├── share/                      # Default skills, programs, config
└── docs/                       # Architecture docs, RFCs, design docs
```

---

## 7. Security Model

Oxios follows an OWASP-inspired security model built around least privilege, defense in depth, and auditability.

### Security Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        SECURITY LAYERS                               │
│                                                                      │
│  Layer 1: Authentication                                            │
│  ┌────────────────────────────────────────────────────────────┐     │
│  │  AuthManager                                                │     │
│  │  • API key validation                                       │     │
│  │  • Credential resolution (config → oxi auth → env)         │     │
│  │  • Key metadata tracking                                    │     │
│  └────────────────────────────────────────────────────────────┘     │
│                                                                      │
│  Layer 2: Authorization (RBAC)                                       │
│  ┌────────────────────────────────────────────────────────────┐     │
│  │  AccessManager                                              │     │
│  │  • Role-Based Access Control                                │     │
│  │  • Per-agent tool permissions                               │     │
│  │  • Human-in-the-Loop (HitL) approvals for sensitive ops     │     │
│  │  • Workspace sandboxing (path restrictions)                 │     │
│  └────────────────────────────────────────────────────────────┘     │
│                                                                      │
│  Layer 3: Execution Sandboxing                                       │
│  ┌────────────────────────────────────────────────────────────┐     │
│  │  ExecTool + AccessManager                                   │     │
│  │  • Shell mode: bash -c, RBAC-enforced                      │     │
│  │  • Structured mode: binary allowlist + metachar blocking    │     │
│  │  • Directory-based workspace sandbox                        │     │
│  │  • WASM sandbox for untrusted code (feature-gated)          │     │
│  └────────────────────────────────────────────────────────────┘     │
│                                                                      │
│  Layer 4: Audit & Integrity                                          │
│  ┌────────────────────────────────────────────────────────────┐     │
│  │  AuditTrail + GitLayer                                      │     │
│  │  • Merkle-chain tamper-evident audit log (BLAKE3)           │     │
│  │  • Every access decision logged                             │     │
│  │  • Guardian daemon verifies chain integrity                 │     │
│  │  • All state changes version-controlled via Git             │     │
│  └────────────────────────────────────────────────────────────┘     │
│                                                                      │
│  Layer 5: Resilience                                                 │
│  ┌────────────────────────────────────────────────────────────┐     │
│  │  CircuitBreaker + BudgetManager                             │     │
│  │  • 3-state circuit breaker for LLM provider failures        │     │
│  │  • Token/cost budgets per agent                             │     │
│  │  • Max execution time enforcement                           │     │
│  │  • Resource monitoring and overload detection               │     │
│  └────────────────────────────────────────────────────────────┘     │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Path Sandboxing

Agents are confined to their assigned workspace directory. The AccessManager enforces:

```
Agent "code-review"
    ├── Allowed paths:  /workspace/project-a/
    ├── Allowed tools:  [bash, read, write, edit, grep, find]
    ├── Denied paths:   Everything outside assigned workspace
    └── Network access: None by default

File access check:
  requested_path: /workspace/project-a/src/main.rs
  workspace_root: /workspace/project-a/
  → WITHIN workspace → ALLOWED

File access check:
  requested_path: /etc/passwd
  workspace_root: /workspace/project-a/
  → OUTSIDE workspace → DENIED
```

### RBAC Flow

```
Agent requests tool use
         │
         ▼
  ┌──────────────────┐
  │ Does agent have  │──No──→ DENY + audit log
  │ tool permission? │
  └────────┬─────────┘
           │ Yes
           ▼
  ┌──────────────────┐
  │ Does agent have  │──No──→ DENY + audit log
  │ path access?     │
  └────────┬─────────┘
           │ Yes
           ▼
  ┌──────────────────┐
  │ Is operation     │──Yes─→ Queue for human approval
  │ high-risk?       │         (HitL)
  └────────┬─────────┘
           │ No
           ▼
  ┌──────────────────┐
  │ Execute + audit  │
  │ log the action   │
  └──────────────────┘
```

---

## 8. Unix Philosophy Mapping

Oxios maps classic Unix concepts to the agent OS paradigm:

```
┌────────────────────────────────┬──────────────────────────────────────────────┐
│        Unix Concept            │           Oxios Equivalent                   │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ init (PID 1)                   │ Supervisor                                   │
│                                │ Manages agent lifecycle (fork/exec/wait/kill)│
├────────────────────────────────┼──────────────────────────────────────────────┤
│ fork()                         │ supervisor.fork(seed)                        │
│                                │ Create new agent from specification          │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ exec()                         │ supervisor.exec(id) / AgentRuntime.run()     │
│                                │ Start executing an agent                     │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ wait() / waitpid()             │ supervisor.wait(id)                          │
│                                │ Block until agent completes                  │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ kill()                         │ supervisor.kill(id)                          │
│                                │ Terminate agent (cooperative + abort)        │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ ps                             │ supervisor.list()                            │
│                                │ List all agents with status                  │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Filesystem                     │ StateStore                                   │
│                                │ Everything is version-controlled state       │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Pipes                          │ EventBus                                     │
│                                │ Inter-agent communication via broadcast      │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Signals                        │ A2AProtocol messages                         │
│                                │ Agent-to-agent (delegation, status, result)  │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ cron / at                      │ CronScheduler                                │
│                                │ Time-based scheduled job execution            │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ git                            │ GitLayer                                     │
│                                │ In-process version control via gix            │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ /proc                          │ ResourceMonitor                              │
│                                │ CPU/memory tracking and overload detection   │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ cgroups                        │ BudgetManager                                │
│                                │ Per-agent resource limits (tokens, cost)     │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ seccomp / chroot               │ WasmSandbox + AccessManager                  │
│                                │ Sandboxed execution with path restrictions   │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ /var/log                       │ AuditTrail                                   │
│                                │ Merkle-chain tamper-evident audit log        │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ System calls                   │ KernelHandle (11 domain APIs)                │
│                                │ The ONLY interface for kernel operations     │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Shell / Binaries               │ Programs + Skills                            │
│                                │ Installable capabilities + instructions      │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Terminal / TTY                 │ Channels (Web/CLI/Telegram)                  │
│                                │ User entry points to the system              │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Everything is a file           │ Everything is StateStore with versioning     │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Small programs do big things   │ Small tools composed by agents               │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Text streams as interface      │ Message streams as interface                 │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Do one thing well              │ Each subsystem has single responsibility     │
├────────────────────────────────┼──────────────────────────────────────────────┤
│ Kernel provides interface only │ Kernel exposes System Calls only             │
└────────────────────────────────┴──────────────────────────────────────────────┘
```

---

## 9. Dependency Rules

### Layer Dependency Direction

```
    Terminal         →         Application         →         Kernel         →         Runtime
                                                                        ↕
                                                                  Engine (oxi)
                                                                  (horizontal)
```

**Rules:**

1. **Each layer depends ONLY on the layer directly below it.**
2. **Dependencies flow top-down only. No reverse dependencies.**
3. **Engine (oxi) is a horizontal dependency** — used by both Kernel and Runtime, but depends on neither.

```
    ┌─────────────┐        ┌──────────────┐       ┌─────────────┐       ┌──────────┐
    │  Terminal   │──────→│ Application  │──────→│   Kernel    │──────→│ Runtime  │
    │             │        │              │       │             │       │          │
    │  Depends on │        │  Depends on  │       │  Depends on │       │ Depends  │
    │  App only   │        │  Kernel only │       │  Engine     │       │ on Engine│
    └─────────────┘        └──────────────┘       │             │       └──────────┘
                                                   │  Engine is  │
                                                   │  horizontal │
                                                   └─────────────┘
```

### Subsystem Isolation

Within the Kernel, subsystems are `pub(crate)` — they can see each other but are invisible to external code. The ONLY public interface is `KernelHandle`.

```
┌─────────────────────────────────────────────────────────┐
│                    oxios-kernel (crate)                   │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │            Public API (pub)                       │   │
│  │                                                   │   │
│  │  KernelHandle, StateApi, AgentApi, ...            │   │
│  │  Orchestrator, Supervisor (trait), ...            │   │
│  │  Public types: AgentId, Seed, Space, etc.         │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │          Internal (pub(crate))                    │   │
│  │                                                   │   │
│  │  All subsystem implementations                    │   │
│  │  Inter-subsystem wiring                           │   │
│  │  Tool registration logic                          │   │
│  │  Event handlers                                   │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
└─────────────────────────────────────────────────────────┘
         │                            │
         ▼                            ▼
  External crates                 External crates
  (oxios binary,                  (nothing — internal
   channels)                       details hidden)
```

### Key Architectural Invariants

> **INVARIANT 1:** The Kernel exposes System Calls ONLY. Applications and channels never touch subsystems directly.
>
> **INVARIANT 2:** The KernelHandle is the single path for all kernel operations. No shortcuts.
>
> **INVARIANT 3:** Kernel internals (subsystems) are invisible outside the `oxios-kernel` crate.
>
> **INVARIANT 4:** Engine (oxi) is a horizontal dependency — Kernel and Runtime both use it, but it depends on neither.
>
> **INVARIANT 5:** No reverse dependencies. Terminal → Application → Kernel → Runtime. Never upward.
>
> **INVARIANT 6:** All tool access goes through KernelHandle. AgentRuntime registers tools that delegate to KernelHandle APIs.
>
> **INVARIANT 7:** The binary crate (`src/kernel.rs`) assembles components. The library crate (`oxios-kernel`) provides them. Assembly ≠ provision.

---

*This document reflects the architecture as of v0.1.2. For the latest implementation details, refer to the source code and inline documentation.*
