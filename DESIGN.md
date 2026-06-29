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
3. **Crystallize** — Generate a directive before acting. Once set, it doesn't change
4. **Evolve** — Each loop knows more than the last

### Convergence

Both philosophies reject uncertainty. Unix fails fast on bad input. Ouroboros clarifies before acting.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Gateway                                 │
│            (channel-agnostic message hub)                  │
│                                                             │
│   ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐                      │
│   │ Web  │ │ CLI  │ │Telegram│ │Discord│ ...                 │
│   │      │ │      │ │       │ │       │                      │
│   └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘                      │
│      └────────┴────────┴────────┴─────────┘                │
│                     │                                       │
│           message in → route → dispatch                     │
└─────────────────────┼───────────────────────────────────────┘
                      │
                      ▼
┌───────────────────────────────────────────────────────────────┐
│                  Kernel (oxios-kernel)                         │
│                                                                │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐         │
│  │ Supervisor  │  │ Event Bus    │  │ State Store   │         │
│  │ (lifecycle) │  │ (broadcast) │  │ (markdown)    │         │
│  └─────────────┘  └──────────────┘  └───────────────┘         │
│                                                                │
│  │          Ouroboros Protocol                              │   │
│  │  assess → crystallize → execute → review                 │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          Agent Runtime (oxi-agent)                      │   │
│  │  tools: read, write, edit, bash, grep, find, ls         │   │
│  │  LLM: oxi-ai (multi-provider)                           │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          AIOS-Inspired Kernel Extensions                │   │
│  │  ┌──────────────┐ ┌───────────────┐ ┌───────────────┐  │   │
│  │  │   Scheduler  │ │ContextManager │ │   Access      │  │   │
│  │  │ (priority/   │ │  (3-tier:    │ │   Manager     │  │   │
│  │  │  rate-limit) │ │  active/cache/│ │   (OWASP)     │  │   │
│  │  │              │ │  archive)    │ │               │  │   │
│  │  └──────────────┘ └───────────────┘ └───────────────┘  │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          Programs & MCP                                  │   │
│  │  ┌──────────────────┐ ┌───────────────────────────────┐ │   │
│  │  │   ProgramManager │ │        McpBridge              │ │   │
│  │  │  (OS-level apps) │ │  (MCP protocol awareness)    │ │   │
│  │  └──────────────────┘ └───────────────────────────────┘ │   │
│  │  ┌──────────────────────────────────────────────────┐  │   │
│  │  │        ExecTool / HostToolValidator               │  │   │
│  │  │  (secure host command execution & validation)    │  │   │
│  │  └──────────────────────────────────────────────────┘  │   │
│  └────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
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
| Shell script | Program | OS-level installable capability |
| Daemon | Background agent | Background service |
| /bin, /usr/bin | Tool registry | Built-in tools |
| Filesystem | Workspace | Persistent storage |
| ExecTool | ExecTool | Secure host command execution |
| Device driver | MCP server | Protocol-aware tool extension |
| Package dependency | host_tools | Host command availability |

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
│   │       ├── exec_tool.rs    ExecTool (secure host command execution)
│   │       ├── program.rs     ProgramManager (OS-level programs)
│   │       ├── skill.rs       SkillStore (markdown instruction templates)
│   │       ├── mcp.rs         McpBridge (MCP protocol awareness)
│   │       ├── host_tools.rs  HostToolValidator (host dependency check)
│   │       ├── scheduler.rs   AgentScheduler (AIOS-inspired priority queue)
│   │       ├── context_manager.rs ContextManager (3-tier RAM-like)
│   │       ├── access_manager.rs AccessManager (OWASP-inspired security)
│   │       └── types.rs       AgentId, AgentInfo, AgentStatus
│   │
│   ├── oxios-ouroboros/    Spec-first protocol implementation
│   │   └── src/
│   │       ├── lib.rs          Public exports
│   │       ├── protocol.rs    OuroborosProtocol trait, Phase, ExecutionResult
│   │       ├── interview.rs    InterviewResult, questions/answers
│   │       ├── directive.rs   Directive (crystallize output)
│   │       ├── types.rs       AmbiguityScore, Entity, shared types
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
│   │   │   ├── routes.rs  API route handlers
│   │   │   └── channel.rs WebChannel implements Channel trait
│   │   └── static/
│   │       ├── index.html      Dashboard UI (SPA)
│   │       └── default-config.toml
│
└── docs/
```

### Dependencies (no reimplementation)

```
oxi/oxi-ai        → LLM provider layer for oxios-kernel and ouroboros
oxi/oxi-agent     → Tool runtime for oxios-kernel (AgentRuntime)
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
- **Program Manager** — OS-level installable applications
- **MCP Bridge** — Model Context Protocol awareness
- **Host Tool Validator** — Host dependency validation
- **Persona Manager** — Multi-persona support for future agent customization

**Agent Lifecycle:**

```
User request → Gateway → Kernel
                        │
                        ├── 1. assess: classify message (conversation / clarify / task)
                        ├── 2. crystallize: generate directive (goal, constraints, criteria)
                        ├── 3. fork: create agent instance
                        ├── 4. execute: tool-calling loop per directive
                        ├── 5. review: verify result against criteria
                        └── 6. reap: cleanup after completion
                              │
                              └── Result → Gateway → User
```

**Core traits:**

```rust
// Supervisor manages agent lifecycle
trait Supervisor: Send + Sync {
    async fn fork_directive(&self, directive: &Directive, env: &ExecEnv) -> Result<AgentId>;
    async fn run(&self, id: AgentId) -> Result<ExecutionResult>;
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
    DirectiveCreated { directive_id: DirectiveId },
    ReviewComplete { directive_id: DirectiveId, passed: bool },
    PhaseStarted { session_id: String, phase: Phase },
    PhaseCompleted { session_id: String, phase: Phase, result_summary: String },
    // ...
}
```

### 1a. Agent Scheduler (AIOS-inspired)

Inspired by the AIOS paper (Rutgers) and AgentRM systems, the Scheduler manages
agent task execution with OS-like scheduling discipline:

**Key features:**
- **Priority queue** — Tasks ranked by priority (Critical > High > Normal > Low),
  with FIFO ordering within the same priority level
- **Rate-limit-aware admission** — Checks LLM API rate limits before starting tasks
- **Max concurrent enforcement** — Configurable limit on simultaneous tasks
- **Zombie detection & reaping** — Tasks exceeding timeout are automatically reaped

```rust
pub enum Priority { Critical, High, Normal, Low }

pub struct ScheduledTask {
    pub id: Uuid,
    pub description: String,
    pub priority: Priority,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub error: Option<String>,
}

impl AgentScheduler {
    pub fn submit(&self, task: ScheduledTask) -> Result<Uuid>;
    pub fn next_task(&self) -> Option<ScheduledTask>;
    pub fn complete_task(&self, task_id: Uuid) -> Result<()>;
    pub fn rate_limit_remaining(&self) -> u32;
    pub fn stats(&self) -> SchedulerStats;
}
```

### 1b. Context Manager (AIOS-inspired)

Like an OS managing RAM pages, the Context Manager manages LLM context windows:

| Tier | Storage | Capacity | Use case |
|------|---------|----------|----------|
| **Active** | In-memory, in-context | Configurable (default 100k tokens) | Current conversation |
| **Cache** | In-memory, not in-context | Configurable (default 50 entries) | Recent sessions |
| **Archive** | Compressed on disk | Unlimited | Long-term storage |

```rust
pub enum ContextTier { Active, Cache, Archive }

pub struct ContextEntry {
    pub session_id: String,
    pub tier: ContextTier,
    pub content: String,
    pub token_count: usize,
    pub created_at: DateTime<Utc>,
}

impl ContextManager {
    pub fn store_active(&self, session_id: &str, content: &str) -> Result<()>;
    pub fn get_active(&self, session_id: &str) -> Option<ContextEntry>;
    pub fn demote_to_cache(&self, session_id: &str) -> Result<()>;
    pub fn stats(&self) -> ContextStats;
}
```

### 1c. Access Manager (OWASP-inspired)

Enforces least-privilege security for all agents:

```rust
pub struct AgentPermissions {
    pub agent_name: String,
    pub allowed_tools: HashSet<String>,
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub network_access: bool,
    pub max_execution_time_secs: u64,
    pub max_memory_mb: u64,
    pub can_fork: bool,
}

impl AccessManager {
    pub fn can_use_tool(&self, agent_name: &str, tool: &str) -> bool;
    pub fn can_access_path(&self, agent_name: &str, path: &str) -> bool;
    pub fn get_or_create_permissions(&self, agent_name: &str) -> Arc<Mutex<AgentPermissions>>;
    pub fn audit_log(&self) -> &[AuditEntry];
}
```

### 1d. Program Manager (OS-level applications)

Programs are the OS-level installable applications. Like Unix has `/bin` programs,
Oxios has programs that agents can "execute" to gain capabilities:

```rust
pub struct Program {
    pub meta: ProgramMeta,
    pub skill_content: String,
    pub enabled: bool,
    pub path: PathBuf,
}

pub struct ProgramMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub tools: HashMap<String, ToolDef>,
    pub host_requirements: HostRequirements,
}

impl ProgramManager {
    pub async fn install(&self, path: &Path) -> Result<Program>;
    pub async fn uninstall(&self, name: &str) -> Result<()>;
    pub async fn list_programs(&self) -> Vec<Program>;
    pub async fn get_program(&self, name: &str) -> Option<Program>;
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()>;
    pub async fn check_host_requirements(&self, name: &str) -> Result<HostRequirementsCheck>;
}
```

**Program structure:**
```
program/
├── program.toml     # Metadata
├── SKILL.md        # Instruction file
├── bin/            # Optional executables
└── config/         # Optional configs
```

### 1e. MCP Bridge (Model Context Protocol)

MCP awareness for connecting to external tool providers:

```rust
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl McpBridge {
    pub fn list_servers(&self) -> Vec<McpServer>;
    pub fn register_server(&mut self, server: McpServer) -> Result<()>;
    pub fn get_capabilities(&self) -> McpCapabilities;
}
```

### 1f. Host Tool Validator

Validates host command dependencies for the execution environment:

```rust
pub struct HostRequirements {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

pub struct HostToolStatus {
    pub tool: String,
    pub present: bool,
    pub path: Option<String>,
}

pub struct FullCheckResult {
    pub all_required_present: bool,
    pub missing_required: Vec<String>,
    pub optional_available: HashMap<String, bool>,
}

impl HostToolValidator {
    pub fn validate(&self, required: &[String]) -> HostToolStatus;
    pub fn full_check(&self) -> FullCheckResult;
    pub fn check_tool(&self, tool: &str) -> HostToolStatus;
}
```

**Philosophy:** Agents use host tools directly. Rich functionality comes from the host system.

### 1g. Session Management

Sessions track user conversations for persistence and history:

```rust
pub struct Session {
    pub id: SessionId,
    pub user_id: String,
    pub user_messages: Vec<UserMessage>,
    pub agent_responses: Vec<AgentResponse>,
    pub active_persona_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: SessionMetadata,
}

pub struct ExecutionMetadata {
    pub execution_count: u32,
    pub last_executed: Option<DateTime<Utc>>,
    pub success_count: u32,
    pub average_score: f64,
    pub tags: Vec<String>,
}
```

**Directive execution metadata:** Each directive tracks its execution history:
- `execution_count` — total number of times executed
- `last_executed` — timestamp of most recent execution
- `success_count` — how many evaluations passed
- `average_score` — rolling average of evaluation scores
- `tags` — user-defined categorization labels

Sessions are created per user conversation and persisted for later retrieval.
The Orchestrator creates/updates Sessions automatically during message handling.

**Session API:**
- `GET /api/sessions` — List recent sessions
- `GET /api/sessions/:id` — Get session with full message history
- `DELETE /api/sessions/:id` — Delete a session

### 1h. Persona Manager (Multi-Agent Characters)

Personas are AI characters with distinct voices, roles, and system prompts.
Multiple personas can be active simultaneously, laying the foundation for future
multi-agent chat scenarios (e.g., group chat with Dev, Review, and Research together).

```rust
pub struct Persona {
    pub id: String,
    pub name: String,
    pub role: String,          // developer, qa, architect, researcher...
    pub description: String,
    pub system_prompt: String,  // The persona's character definition
    pub enabled: bool,
    pub model: Option<String>,  // Optional model override
    pub personality_traits: Vec<String>, // curious, skeptical, creative...
}

/// Default personas created on first run:
/// - **Dev** — Pragmatic developer focused on implementation
/// - **Review** — Skeptical QA/architect focused on quality
/// - **Research** — Curious researcher focused on understanding and evidence
```

```rust
impl PersonaManager {
    pub fn get_active_persona(&self) -> Option<Persona>;
    pub fn set_active_persona(&self, id: &str) -> Result<()>;
    pub fn active_system_prompt(&self) -> String;
    pub fn create_default_personas(&self);  // Dev, Review, Research
}
```

**API routes:**
- `GET /api/personas` — List all personas
- `GET /api/personas/:id` — Get persona details
- `POST /api/personas` — Create persona
- `PUT /api/personas/:id` — Update persona
- `DELETE /api/personas/:id` — Delete persona
- `GET /api/personas/active` — Get active persona
- `PUT /api/personas/active` — Set active persona

---

## Programs (OS-level installable applications)

Programs provide structured capabilities that agents can leverage. They embody the
Unix philosophy: small, composable, installable units of functionality.

### Structure

```
my-program/
├── program.toml     # Metadata (name, version, tools, dependencies)
├── SKILL.md        # Instruction file (agent-facing documentation)
├── bin/            # Optional: executable scripts
└── config/         # Optional: configuration files
```

### program.toml Format

```toml
[program]
name = "code-review"
version = "1.0.0"
description = "Comprehensive code review guidelines for agents"
author = "oxios"

# Tools this program exposes
[tools]
check_security = { description = "Run security checks on code" }

# Host tool requirements
[host_requirements]
required = ["git"]
optional = ["gh"]
```

### Philosophy

Programs are **READ-ONLY** instruction sets. They don't execute themselves;
they provide guidelines and tool definitions that agents consume.
Think of them as man pages that come with metadata for discovery.

### Host Dependency Validation

Programs declare which host tools they need. The Host Tool Validator checks availability:

```toml
[host_requirements]
required = ["git"]
optional = ["gh"]
```

This embodies Unix philosophy: declare what you need, fail clearly if missing.

---

## Command Interface (CLI)

```bash
oxios                          Interactive mode (default — starts web server on port 4200)
oxios run "do something"       Run single prompt through Ouroboros
oxios program install <path>   Install a program
oxios program list             List installed programs
oxios program uninstall <name> Uninstall a program
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
  ├── oxios-ouroboros (assess, crystallize, review) ✓
  ├── Ambiguity score calculation ✓
  └── Directive generation/validation ✓

Phase 3: Gateway + Web ✓
  ├── oxios-gateway (channel trait, routing) ✓
  ├── oxios-web (HTTP server, dashboard) ✓
  └── Chat + Control + Browse ✓

Phase 4: Exec Tool ✓
  ├── ExecTool (secure host command execution) ✓
  └── Host Tool Validator ✓

Phase 5: AIOS Extensions ✓
  ├── AgentScheduler (priority/rate-limit) ✓
  ├── ContextManager (3-tier hierarchy) ✓
  └── AccessManager (OWASP security) ✓

Phase 6: Programs + MCP + Host Tools ✓
  ├── ProgramManager (OS-level programs) ✓
  ├── SkillStore (markdown instruction templates) ✓
  ├── HostToolValidator (host dependency validation) ✓
  ├── McpBridge (MCP protocol awareness) ✓
  └── Default programs installation ✓

Phase 7: Channel expansion
  ├── oxios-telegram (later)
  ├── oxios-cli (later)
  └── ...
```

---

## Project Info

| Item | Value |
|------|-------|
| Language | Rust (edition 2021) |
| Target | macOS Silicon |
| Engine | oxi-ai + oxi-agent (oxi path dependency) |
| License | MIT |
| Default port | 4200 |
