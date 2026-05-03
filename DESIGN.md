# Oxios — Agent Operating System

> *"Do one thing well. Specify before you build. Evolve, don't repeat."*

Oxios is an Agent Operating System where Unix philosophy meets Ouroboros spec-first methodology. Humans describe intent poorly; Oxios clarifies, specifies, and executes.

---

## Philosophy

### Unix Principles

1. **Minimal unit** — Every component is small and does one thing well
2. **Composition** — Small pieces connect via pipes to build larger things
3. **Text is universal** — Markdown is the universal interface
4. **Extensibility** — Build what you need, omit what you don't (YAGNI)

### Ouroboros Principles

1. **Wonder** — "What IS this, really?" — question the essence first
2. **Ambiguity Gate** — Don't execute until ambiguity ≤ 0.2
3. **Seed** — Immutable specification. Once set, it doesn't change
4. **Evolve** — Each loop knows more than the last

### Convergence

Both philosophies reject uncertainty. Unix fails fast on bad input. Ouroboros clarifies before acting.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Gateway                            │
│            (channel-agnostic message hub)              │
│                                                         │
│   ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐        │
│   │ Web  │ │ CLI  │ │Tele- │ │Disc- │ │ API  │  ...    │
│   │      │ │      │ │gram  │ │ord   │ │      │        │
│   └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘        │
│      └────────┴────────┴────────┴────────┘              │
│                     │                                    │
│           message in → route → dispatch                  │
└─────────────────────┼───────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────┐
│                  Kernel (oxios-kernel)                   │
│                                                          │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐   │
│  │ Supervisor  │  │ Event Bus    │  │ State Store   │   │
│  │ (lifecycle) │  │ (broadcast)  │  │ (markdown)    │   │
│  └─────────────┘  └──────────────┘  └───────────────┘   │
│                                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │          Ouroboros Protocol                      │   │
│  │  interview → seed → execute → evaluate → evolve │   │
│  └─────────────────────────────────────────────────┘   │
│                                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │          Agent Runtime (oxi-agent)               │   │
│  │  tools: read, write, edit, bash, grep, find, ls  │   │
│  │  LLM: oxi-ai (multi-provider)                    │   │
│  └─────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
                      │
                      ▼
┌──────────────────────────────────────────────────────────┐
│              Container Garden (Apple Container)           │
│              macOS Silicon only                           │
└──────────────────────────────────────────────────────────┘
```

---

## Unix ↔ Oxios Mapping

| Unix | Oxios | Role |
|------|-------|------|
| Kernel (syscalls) | oxi-agent (tool calls) | Minimal execution unit: read, write, edit, bash |
| Process | Agent instance | Running AI instance |
| Shell | Gateway | Human↔OS interface |
| Pipe (|) | Event Bus | Inter-process communication |
| init (PID 1) | Supervisor | Agent lifecycle management |
| Shell script | Skill | Composition of tools into larger capability |
| Daemon | Background agent | Background service |
| /bin, /usr/bin | Tool registry | Built-in tools |
| Filesystem | Workspace | Persistent storage |
| Container | Garden | Isolated execution environment |

---

## Crate Structure

```
oxios/
├── Cargo.toml              (workspace root)
├── DESIGN.md
├── AGENTS.md
├── README.md
│
├── crates/
│   ├── oxios-kernel/       Core: supervisor, lifecycle, event bus, state store
│   │   └── src/
│   │       ├── lib.rs          Public exports
│   │       ├── config.rs       TOML configuration (OxiosConfig)
│   │       ├── event_bus.rs    KernelEvent, EventBus (broadcast)
│   │       ├── state_store.rs  Markdown-based state persistence
│   │       ├── supervisor.rs   Supervisor trait, BasicSupervisor
│   │       ├── agent_runtime.rs oxi-agent wrapper (AgentRuntime)
│   │       ├── orchestrator.rs  Ouroboros lifecycle coordinator
│   │       ├── container.rs    AppleBackend implements ContainerBackend
│   │       ├── garden.rs       GardenManager (container lifecycle)
│   │       ├── host_exec.rs   HostExecBridge (secure relay)
│   │       └── types.rs       AgentId, AgentInfo, AgentStatus
│   │
│   ├── oxios-ouroboros/    Spec-first protocol implementation
│   │   └── src/
│   │       ├── lib.rs          Public exports
│   │       ├── protocol.rs    OuroborosProtocol trait, Phase, ExecutionResult
│   │       ├── interview.rs    InterviewResult, questions/answers
│   │       ├── seed.rs         Seed, AmbiguityScore, Entity
│   │       ├── evaluation.rs   EvaluationResult (mechanical/semantic/consensus)
│   │       └── ouroboros_engine.rs LLM-backed OuroborosEngine
│   │
│   ├── oxios-gateway/      Channel-agnostic message hub
│   │   └── src/
│   │       ├── lib.rs          Public exports
│   │       ├── gateway.rs      Gateway struct, route(), run()
│   │       ├── channel.rs     Channel trait (name, receive, send)
│   │       └── message.rs     IncomingMessage, OutgoingMessage
│   │
│   └── oxios/              Main binary
│       └── src/main.rs      CLI, kernel init, server startup
│
├── channels/
│   ├── oxios-web/         Web dashboard (first channel)
│   │   ├── src/
│   │   │   ├── lib.rs      Public exports
│   │   │   ├── server.rs  Axum HTTP server with graceful shutdown
│   │   │   ├── routes.rs  API route handlers (chat, status, gardens, etc.)
│   │   │   └── channel.rs WebChannel implements Channel trait
│   │   └── static/
│   │       ├── index.html      Dashboard UI (SPA)
│   │       ├── default-config.toml
│   │       └── Containerfile
│   └── oxios-web/          (placeholder for future channels)
│
└── docs/
```

### Dependencies (no reimplementation)

```
pi2oxi/oxi-ai        → LLM provider layer for oxios-kernel and ouroboros
pi2oxi/oxi-agent     → Tool runtime for oxios-kernel (AgentRuntime)
```

Oxios is a layer on top of oxi. oxi is consumed as a path dependency, never reimplemented.

---

## Core Components

### 1. oxios-kernel

The OS kernel. Everything passes through here.

**Responsibilities:**
- **Supervisor** — Agent instance creation (fork), execution, monitoring, termination (reap)
- **Event Bus** — Inter-agent communication (evolved Unix pipe, broadcast)
- **State Store** — Markdown-based persistent state (sessions, memory, workspace)
- **Agent Runtime** — Wraps oxi-agent for tool-calling loop execution
- **Orchestrator** — Coordinates full Ouroboros lifecycle per message
- **Garden Manager** — Container lifecycle management

**Agent Lifecycle:**

```
User request → Gateway → Kernel
                        │
                        ├── 1. Ouroboros: interview (clarify intent)
                        ├── 2. Ouroboros: seed (generate spec)
                        ├── 3. fork: create agent instance
                        ├── 4. exec: tool-calling loop per spec
                        ├── 5. evaluate: verify result
                        └── 6. reap: cleanup after completion
                              │
                              └── Result → Gateway → User
```

**Core traits:**

```rust
// Supervisor manages agent lifecycle
trait Supervisor: Send + Sync {
    async fn fork(&self, spec: &Seed) -> Result<AgentId>;
    async fn run_with_seed(&self, id: AgentId, seed: &Seed) -> Result<ExecutionResult>;
    async fn wait(&self, id: AgentId) -> Result<AgentStatus>;
    async fn kill(&self, id: AgentId) -> Result<()>;
    async fn list(&self) -> Result<Vec<AgentInfo>>;
}

// Event bus for kernel events
struct EventBus { /* broadcast channel */ }
enum KernelEvent {
    AgentCreated { id: AgentId, name: String },
    AgentStarted { id: AgentId },
    AgentStopped { id: AgentId },
    AgentFailed { id: AgentId, error: String },
    SeedCreated { seed_id: SeedId },
    EvaluationComplete { seed_id: SeedId, passed: bool },
    PhaseStarted { session_id: String, phase: Phase },
    PhaseCompleted { session_id: String, phase: Phase, result_summary: String },
    // ...
}
```

### 2. oxios-ouroboros

Rust implementation of the Ouroboros methodology. The protocol that governs every agent's lifecycle.

**The protocol never skips steps:**

```
interview  → Ask until ambiguity ≤ 0.2
seed       → Generate immutable spec (JSON, stored in seeds/)
execute    → Run tool-calling loop per spec (delegated to AgentRuntime)
evaluate   → 3-stage verification (mechanical → semantic → consensus)
evolve     → Feed evaluation back as input for next loop
```

**Ambiguity Score:**

```rust
struct AmbiguityScore {
    goal_clarity: f64,        // weight 40%
    constraint_clarity: f64,  // weight 30%
    success_criteria: f64,   // weight 30%
}

impl AmbiguityScore {
    fn ambiguity(&self) -> f64 {
        1.0 - (self.goal_clarity * 0.4
             + self.constraint_clarity * 0.3
             + self.success_criteria * 0.3)
    }

    fn is_ready(&self) -> bool {
        self.ambiguity() <= 0.2
    }
}
```

**Seed (immutable spec):**

```rust
struct Seed {
    id: Uuid,
    goal: String,
    constraints: Vec<String>,
    acceptance_criteria: Vec<String>,
    ontology: Vec<Entity>,
    created_at: DateTime<Utc>,
    // Immutable after creation. To change, create a new Seed via evolve().
}
```

**Nine Minds (thinking modes, loaded on demand):**

Implemented via LLM system prompts in OuroborosEngine (not loaded as separate types):

| Mode | System Prompt | When active |
|------|--------------|-------------|
| Interviewer | Socratic questioning | interview phase |
| Seed Architect | Crystallize to spec | seed generation |
| Evaluator | 3-stage verification | evaluate phase |
| Contrarian | Opposite hypothesis | stagnation detected |
| Hacker | Constraint reality check | stuck |
| Simplifier | Simplest path | complexity rising |
| Researcher | Evidence gathering | insufficient info |
| Architect | Structure analysis | structural issues |
| Evolver | Improvement loop | evolve phase |

### 3. oxios-gateway

Channel-agnostic message router. The Gateway doesn't care what channel a message comes from.

**Core traits:**

```rust
trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}

struct Gateway {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
    orchestrator: Arc<Orchestrator>,
}

impl Gateway {
    pub async fn register(&self, channel: Box<dyn Channel>);
    pub async fn route(&self, msg: IncomingMessage) -> Result<()>;
    pub async fn run(&self) -> Result<()>; // polls channels in loop
}
```

**Message flow:**

```
User → Channel(Web) → Gateway → Orchestrator → Kernel → Ouroboros → Agent → Result
                                                                │
User ← Channel(Web) ← Gateway ← Result ◄────────────────────────────────┘
```

Channels are plugins. Web is first, Telegram, Discord, CLI plug in later.

### 4. oxios-web

First channel. Web dashboard with Axum HTTP server.

**Capabilities:**
- Chat (converse with agents via POST /api/chat)
- Control (agent status, system resources)
- Browse (memory, documents, seeds)
- Gardens (container lifecycle management)
- Events (SSE stream of KernelEvent)

**Tech:** Axum + tower-http + static HTML/CSS/JS frontend

---

## State Store

All state is markdown or JSON files.

```
~/.oxios/
├── config.toml              System configuration
├── workspace/
│   ├── memory/              Agent memory
│   │   ├── 2026-05-03.md    Daily conversation summaries
│   │   └── knowledge/       Knowledge base
│   │       └── project-a.md
│   ├── seeds/               Ouroboros Seed specs (JSON)
│   │   └── <uuid>.json
│   ├── sessions/            Conversation sessions
│   │   └── abc123.jsonl
│   └── skills/              Skill definitions
│       └── code-review/
│           └── SKILL.md
└── gardens/                 Container isolation environments
    └── project-a/
        ├── workspace/       Mounted to container
        ├── Containerfile
        └── .env
```

---

## Container Isolation

Apple Container based. Each Garden is an isolated execution environment.

**CLI:**

```bash
oxios garden new project-a     ← Create garden
oxios garden up project-a      ← Start container
oxios garden exec project-a -- ls /workspace
oxios garden down project-a    ← Stop container
oxios garden remove project-a  ← Delete everything
oxios garden list              ← List all gardens
```

---

## Command Interface (CLI)

```bash
oxios                          Interactive mode (default — starts web server on port 4200)
oxios run "do something"       Run single prompt through Ouroboros
oxios garden new <name>        Create garden
oxios garden up <name>         Start
oxios garden down <name>       Stop
oxios garden remove <name>    Remove
oxios garden list              List
oxios garden exec <name> -- cmd args...  Execute in garden
oxios status                   System status
oxios config show              Show config
oxios config get <key>         Get config value
oxios -c path.toml            Custom config path
oxios -v                       Verbose logging
```

---

## Build Order (MVP)

```
Phase 1: Kernel skeleton ✓
  ├── oxios-kernel (supervisor, event bus, state store) ✓
  ├── oxi-agent dependency wiring ✓
  └── Basic agent execution test ✓

Phase 2: Ouroboros Protocol ✓
  ├── oxios-ouroboros (interview, seed, evaluate) ✓
  ├── Ambiguity score calculation ✓
  └── Seed generation/validation ✓

Phase 3: Gateway + Web ✓
  ├── oxios-gateway (channel trait, routing) ✓
  ├── oxios-web (HTTP server, dashboard) ✓
  └── Chat + Control + Browse + Gardens ✓

Phase 4: Container ✓
  ├── Apple Container integration ✓
  ├── Garden lifecycle ✓
  └── Host Exec Bridge ✓

Phase 5: Channel expansion
  ├── oxios-telegram (later)
  ├── oxios-cli (later)
  └── ...
```

---

## Project Info

| Item | Value |
|------|-------|
| Language | Rust (edition 2021) |
| Target | macOS Silicon (Apple Container) |
| Engine | oxi-ai + oxi-agent (pi2oxi path dependency) |
| License | MIT |
| Default port | 4200 |
