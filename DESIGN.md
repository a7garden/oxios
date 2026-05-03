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
│            (channel-agnostic message hub)                │
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
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ Supervisor  │  │ Event Bus    │  │ State Store   │  │
│  │ (lifecycle) │  │ (IPC/pipe)   │  │ (markdown)    │  │
│  └─────────────┘  └──────────────┘  └───────────────┘  │
│                                                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │          Ouroboros Protocol                      │    │
│  │  interview → seed → execute → evaluate → evolve │    │
│  └─────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │          Agent Runtime (oxi-agent)               │    │
│  │  tools: read, write, edit, bash                  │    │
│  │  LLM: oxi-ai (multi-provider)                   │    │
│  └─────────────────────────────────────────────────┘    │
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
| Pipe (\|) | Event Bus | Inter-process communication |
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
│   ├── oxios-ouroboros/    Spec-first protocol: interview → seed → execute → evaluate
│   ├── oxios-gateway/      Channel-agnostic message hub
│   └── oxios/              Main binary
│
├── channels/
│   ├── oxios-web/          Web dashboard (MVP first channel)
│   ├── oxios-cli/          CLI interface (later)
│   └── oxios-telegram/     Telegram bot (later)
│
└── docs/
    └── ...
```

### Dependencies (no reimplementation)

```
pi2oxi/oxi-ai        → LLM provider layer for oxios-kernel
pi2oxi/oxi-agent     → Tool runtime for oxios-kernel
pi2oxi/oxi-tui       → Not used (Web replaces TUI)
```

Oxios is a layer on top of oxi. oxi is consumed as a path dependency, never reimplemented.

---

## Core Components

### 1. oxios-kernel

The OS kernel. Everything passes through here.

**Responsibilities:**
- **Supervisor** — Agent instance creation (fork), execution, monitoring, termination (reap)
- **Event Bus** — Inter-agent communication (evolved Unix pipe)
- **State Store** — Markdown-based persistent state (sessions, memory, workspace)
- **Tool Registry** — read, write, edit, bash + extension tool registration

**Agent Lifecycle:**

```
User request → Gateway → Kernel
                        │
                        ├── 1. Ouroboros: interview (clarify intent)
                        ├── 2. Ouroboros: seed (generate spec)
                        ├── 3. fork: create agent instance
                        ├── 4. exec: tool-calling loop
                        ├── 5. evaluate: verify result
                        └── 6. reap: cleanup after completion
                              │
                              └── Result → Gateway → User
```

Users don't know how many agents ran. They speak, the OS handles the rest. Like `make -j4`.

**Core traits:**

```rust
trait Kernel {
    async fn fork(&self, spec: &Seed) -> Result<AgentId>;
    async fn exec(&self, id: AgentId) -> Result<()>;
    async fn wait(&self, id: AgentId) -> Result<AgentStatus>;
    async fn kill(&self, id: AgentId) -> Result<()>;
    fn bus(&self) -> &EventBus;
    fn store(&self) -> &StateStore;
}
```

### 2. oxios-ouroboros

Rust implementation of the Ouroboros methodology. The protocol that governs every agent's lifecycle.

**The protocol never skips steps:**

```
interview  → Ask until ambiguity ≤ 0.2
seed       → Generate immutable spec (YAML/TOML)
execute    → Run tool-calling loop per spec
evaluate   → 3-stage verification (mechanical → semantic → consensus)
evolve     → Feed evaluation back as input for next loop
```

**Ambiguity Score:**

```rust
struct AmbiguityScore {
    goal_clarity: f64,        // weight 40%
    constraint_clarity: f64,  // weight 30%
    success_criteria: f64,    // weight 30%
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
    acceptance: Vec<String>,
    ontology: Vec<Entity>,
    created_at: DateTime<Utc>,
    // Immutable after creation. To change, create a new Seed.
}
```

**Nine Minds (thinking modes, loaded on demand):**

| Mode | Question | When active |
|------|----------|-------------|
| Interviewer | "What are you assuming?" | interview phase |
| Ontologist | "What IS this, really?" | ontology definition |
| Seed Architect | "Is this complete?" | seed generation |
| Evaluator | "Did we build the right thing?" | evaluate phase |
| Contrarian | "What if the opposite were true?" | stagnation detected |
| Hacker | "What constraints are real?" | stuck |
| Simplifier | "What's the simplest thing?" | complexity rising |
| Researcher | "What evidence do we have?" | insufficient info |
| Architect | "Would we build it this way?" | structural issues |

### 3. oxios-gateway

Channel-agnostic message router. The Gateway doesn't care what channel a message comes from.

**Core traits:**

```rust
trait Channel {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<IncomingMessage>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}

trait Gateway {
    fn register(&mut self, channel: Box<dyn Channel>);
    async fn route(&self, msg: IncomingMessage) -> Result<()>;
}
```

**Message flow:**

```
User → Channel(Web) → Gateway → Kernel → Ouroboros → Agent → Result
                                                                │
User ← Channel(Web) ← Gateway ← Kernel ← Result ◄─────────────┘
```

Channels are plugins. Build Web first, then Telegram, Discord, CLI plug in later.

### 4. oxios-web

First channel. Web dashboard.

**Capabilities:**
- Chat (converse with agents)
- Control (agent status, system resources, configuration)
- Browse (memory, documents, markdown, knowledge base)

**Tech:** Rust embedded HTTP server (axum) + static frontend

---

## State Store

All state is markdown.

```
~/.oxios/
├── config.toml              System configuration
├── workspace/
│   ├── memory/              Agent memory
│   │   ├── 2026-05-03.md    Daily conversation summaries
│   │   └── knowledge/       Knowledge base
│   │       └── project-a.md
│   ├── seeds/               Ouroboros Seed specs
│   │   └── task-cli.yaml
│   ├── sessions/            Conversation sessions
│   │   └── abc123.jsonl
│   └── skills/              Skill definitions
│       └── code-review/
│           └── SKILL.md
└── gardens/                 Container isolation environments
    └── project-a/
```

---

## Container Isolation

Apple Container based. Each Garden is an isolated execution environment.

```
oxios garden new project-a     ← Create garden
oxios garden up project-a      ← Start container
oxios garden down project-a    ← Stop
oxios garden list              ← List gardens
```

Agents execute tools inside the garden. Host Exec Bridge enables macOS commands (remindctl, shortcuts, etc.).

---

## Command Interface

```
oxios                          Interactive mode (default)
oxios "refactor this code"    Single prompt

oxios garden new <name>        Create garden
oxios garden up <name>         Start
oxios garden down <name>       Stop
oxios garden list              List

oxios seed show                View current seed
oxios seed history             Seed history

oxios status                   System status
oxios logs                     Logs

oxios config set <key> <val>   Set config
oxios config get <key>         Get config
```

---

## Build Order (MVP)

```
Phase 1: Kernel skeleton
  ├── oxios-kernel (supervisor, event bus, state store)
  ├── oxi-agent dependency wiring
  └── Basic agent execution test

Phase 2: Ouroboros Protocol
  ├── oxios-ouroboros (interview, seed, evaluate)
  ├── Ambiguity score calculation
  └── Seed generation/validation

Phase 3: Gateway + Web
  ├── oxios-gateway (channel trait, routing)
  ├── oxios-web (HTTP server, dashboard)
  └── Chat + Control + Browse

Phase 4: Container
  ├── Apple Container integration
  ├── Garden lifecycle
  └── Host Exec Bridge

Phase 5: Channel expansion
  ├── oxios-telegram
  ├── oxios-cli
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
