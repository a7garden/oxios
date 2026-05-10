# Oxios — Agent Operating System

> *"Do one thing well. Specify before you build. Evolve, don't repeat."*

Oxios is an Agent Operating System built in Rust. It combines Unix philosophy (minimal composable tools) with Ouroboros methodology (specification-first workflows) to create an OS where AI agents execute real work on behalf of users.

**Engine:** `oxi-ai` + `oxi-agent` from `pi2oxi` are consumed as path dependencies. Never reimplement what oxi already provides.

**Runtime:** Direct host execution via `ExecTool` with workspace-based sandboxing.

---

## Quick Start

### 1. Install

```bash
cargo install oxios
# Or build from source:
git clone https://github.com/your-repo/oxios
cd oxios && cargo build --release
```

### 2. Configure

Create `~/.oxios/config.toml`:

```toml
[gateway]
host = "127.0.0.1"
port = 4200

[security]
auth_enabled = true
default_api_key = "sk-your-key-here"  # or set OXIOS_API_KEY env var
```

Or use environment variables:
```bash
export OXIOS_API_KEY=sk-your-key-here
export ANTHROPIC_API_KEY=sk-ant-...
```

### 3. Run

```bash
oxios
# Or with a custom config:
oxios --config /path/to/config.toml
```

### 4. Use

```bash
# Via CLI (interactive)
oxios chat

# Via REST API
curl -X POST http://127.0.0.1:4200/api/chat \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OXIOS_API_KEY" \
  -d '{"content": "Build a TODO app", "user_id": "user1"}'

# Via Web Dashboard
open http://127.0.0.1:4200
```

### 5. Cron Jobs (autonomous agents)

Schedule agents to run on a schedule:

```toml
[cron]
enabled = true
tick_interval_secs = 60

[cron.jobs.morning_report]
schedule = "0 9 * * *"
goal = "Summarize latest tech news"
priority = "low"
```

Manage via API:
```bash
# List jobs
curl http://127.0.0.1:4200/api/cron-jobs \
  -H "Authorization: Bearer $OXIOS_API_KEY"

# Create a job
curl -X POST http://127.0.0.1:4200/api/cron-jobs \
  -H "Authorization: Bearer $OXIOS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name":"news","schedule":"0 * * * *","goal":"Fetch top HN stories"}'
```

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
│  ┌────────────────────────────────────────────────────────┐   │
│  │          Ouroboros Protocol                              │   │
│  │  interview → seed → execute → evaluate → evolve         │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          Agent Runtime (oxi-agent)                      │   │
│  │  tools: read, write, edit, exec, grep, find, ls         │   │
│  │  LLM: oxi-ai (multi-provider)                         │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          AIOS-Inspired Extensions                       │   │
│  │  Scheduler (priority queue) | Context (3-tier) | Access │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          Programs & MCP                                 │   │
│  │  ProgramManager | McpBridge | HostToolValidator         │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          ExecTool (host execution)                      │   │
│  │  workspace exec | structured commands | access control  │   │
│  └────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### 1. Build

```bash
cargo build --workspace
```

### 2. Configure

Oxios creates its config on first run at `~/.oxios/config.toml`. Set your API key:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
# or
export OPENAI_API_KEY=sk-...
```

Or edit `~/.oxios/config.toml`:

```toml
[kernel]
workspace = "~/.oxios/workspace"
max_agents = 16

[gateway]
host = "127.0.0.1"
port = 4200

[exec]
allowed_commands = ["git", "gh"]
default_timeout_secs = 120

[scheduler]
max_concurrent = 8
rate_limit_per_minute = 60
```

### 3. Run

```bash
cargo run
# → Oxios starts on http://127.0.0.1:4200
# → Open the URL in your browser to chat
```

---

## CLI Commands

```bash
# Interactive mode (default — starts web server)
oxios

# Run a single prompt
oxios run "do something"

# Program management
oxios program install <path>  # Install a program from directory
oxios program list           # List installed programs
oxios program uninstall <name> # Uninstall a program
oxios program enable <name>   # Enable a program
oxios program disable <name> # Disable a program

# Skill management
oxios skill list            # List available skills
oxios skill create <name> --desc "..." --content "..." # Create a skill

# System
oxios status                 # Show system status
oxios config show            # Show current configuration
oxios config get <key>       # Get a config value
```

### Options

```

---

## Configuration Reference

Oxios uses TOML configuration at `~/.oxios/config.toml`.

### `[kernel]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `workspace` | String | `~/.oxios/workspace` | Base directory for state |
| `event_bus_capacity` | usize | `256` | Broadcast channel capacity |
| `max_agents` | usize | `16` | Max concurrent agents |

### `[gateway]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | String | `"127.0.0.1"` | Web server bind host |
| `port` | u16 | `4200` | Web server port |

### `[exec]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `allowed_commands` | Vec\<String\> | `[]` (all allowed) | Whitelist for host exec |
| `default_timeout_secs` | u64 | `120` | Default timeout per exec call |
| `max_timeout_secs` | u64 | `600` | Maximum allowed timeout |
| `required_host_tools` | Vec\<String\> | `[]` | Host tools that MUST be present |
| `optional_host_tools` | Vec\<String\> | `[]` | Host tools checked lazily |

### `[scheduler]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_concurrent` | usize | `8` | Max simultaneous agent tasks |
| `rate_limit_per_minute` | u32 | `60` | LLM API rate limit |
| `zombie_timeout_secs` | u64 | `300` | Timeout before reaping tasks |

### `[context]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `active_max_tokens` | usize | `100000` | Active tier capacity |
| `cache_max_entries` | usize | `50` | Cache tier capacity |
| `archive_enabled` | bool | `true` | Enable archive tier |

### `[access]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `audit_max_entries` | usize | `10000` | Max audit log entries |
| `default_tool_allowlist` | Vec\<String\> | `["read","write"]` | Default allowed tools |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `OXIOS_API_KEY` | Primary API key (takes precedence over config file) |
| `ANTHROPIC_API_KEY` | Anthropic API key (LLM backend) |
| `OPENAI_API_KEY` | OpenAI API key (LLM backend) |
| `API_KEY` | Fallback API key |
| `RUST_LOG` | Tracing filter (e.g., `info`, `debug`) |

---

## API Reference

The web server exposes a REST API at `http://localhost:4200/api/`.

### Chat

```
POST /api/chat
  Body: { "content": "...", "user_id": "user", "session_id": "" }
  Response: { "id": "...", "echo": "...", "reply": "...", "session_id": "...", "phase": "..." }

GET /api/chat/stream
  WebSocket endpoint for real-time streaming
```

### Status & Control

```
GET  /api/status             → { service, status, version, channels, uptime }
GET  /api/agents             → [{ id, name, status, created_at, seed_id }]
POST /api/agents/:id/kill    → 200 OK
```

### Config

```
GET  /api/config             → current config as JSON
PUT  /api/config             → update config
```

### Workspace

```
GET  /api/workspace/tree?dir=   → [{ name, is_dir, size }]
GET  /api/workspace/file/*path   → file content
PUT  /api/workspace/file/*path   → write file
```

### Seeds

```
GET  /api/seeds                    → [{ id, goal, constraints_count, created_at }]
GET  /api/seeds/:id                → seed JSON
GET  /api/seeds/:id/evolution      → [{ id, generation, goal, parent_id, score, passed }]
```

### Skills

```
GET  /api/skills                   → [{ name, description }]
GET  /api/skills/:name             → { name, description, content, path }
POST /api/skills                   → create skill
DELETE /api/skills/:name           → delete skill
```

### Memory

```
GET  /api/memory             → [{ name, category }]
GET  /api/memory/:name       → { name, category, content }
```

### Exec

```
POST /api/exec     → { command, args?, workdir?, timeout? } → { stdout, stderr, exit_code, duration_ms }
```

### Programs (OS-level applications)

```
GET    /api/programs                      → [{ name, version, description, author, enabled, tools_count }]
POST   /api/programs                      → { path } → install program
GET    /api/programs/:name                → { name, version, tools, skill_content, path }
DELETE /api/programs/:name                → uninstall program
POST   /api/programs/:name/enable         → enable program
POST   /api/programs/:name/disable        → disable program
GET    /api/programs/:name/host-requirements → { all_required_present, missing_required, ... }
```

### Scheduler (AIOS-inspired task scheduling)

```
GET  /api/scheduler/stats   → { queued, running, max_concurrent, rate_limit_per_minute, rate_remaining }
GET  /api/scheduler/tasks  → { queued: [...], running: [...] }
```

### Audit & Permissions

```
GET  /api/audit                         → [{ timestamp, agent_name, action, resource, allowed, reason }]
GET  /api/permissions/:agent            → { agent_name, allowed_tools, allowed_paths, denied_paths, ... }
PUT  /api/permissions/:agent            → update permissions
```

### Host Tools

```
GET  /api/host-tools   → { all_required_present, missing_required, optional_available }
```

### MCP (Model Context Protocol)

```
GET  /api/mcp/servers                    → [{ name, command, args, enabled }]
POST /api/mcp/servers                    → register MCP server (stub)
```

### Events

```
GET /api/events              → SSE stream of KernelEvent
```

---

## Development

### Build

```bash
cargo build --workspace          # Debug build
cargo build --workspace --release # Release build
```

### Test

```bash
cargo test --workspace           # Run all tests
cargo test --workspace -q        # Quiet output
```

### Lint

```bash
cargo clippy --workspace -- -D warnings  # Strict linting
```

### Project Structure

```
oxios/
├── Cargo.toml                 # Workspace root
├── DESIGN.md                 # Architecture and design decisions
├── AGENTS.md                 # AI agent conventions
├── CHANGELOG.md              # Release notes
│
├── crates/
│   ├── oxios-kernel/          # Core: supervisor, event bus, state store, exec
│   │   └── src/
│   │       ├── lib.rs              # Public exports
│   │       ├── supervisor.rs       # Agent lifecycle (fork/exec/wait/kill)
│   │       ├── event_bus.rs        # Broadcast event bus (KernelEvent)
│   │       ├── state_store.rs      # Markdown-based persistent state
│   │       ├── config.rs           # TOML configuration (OxiosConfig)
│   │       ├── orchestrator.rs      # Ouroboros lifecycle coordinator
│   │       ├── agent_runtime.rs     # oxi-agent wrapper for seed execution
│   │       ├── host_tools.rs         # HostToolValidator (required host tools)
│   │       ├── program.rs            # ProgramManager (OS-level programs)
│   │       ├── skill.rs              # SkillStore (markdown instruction templates)
│   │       ├── mcp.rs                # McpBridge (MCP protocol awareness)
│   │       ├── scheduler.rs           # AgentScheduler (AIOS-inspired)
│   │       ├── context_manager.rs      # ContextManager (3-tier hierarchy)
│   │       ├── access_manager.rs      # AccessManager (OWASP-inspired)
│   │       └── types.rs               # AgentId, AgentInfo, AgentStatus
│   │       └── tools/
│   │           ├── exec_tool.rs       # ExecTool (host command execution)
│   │           ├── program_tool.rs    # ProgramTool (routes through ExecTool)
│   │           ├── mcp_tool.rs        # MCP tool bridge
│   │           └── memory_tools.rs    # Memory recall/store tools
│   │
│   ├── oxios-ouroboros/       # Ouroboros spec-first protocol
│   │   └── src/
│   │       ├── lib.rs             # Public exports
│   │       ├── protocol.rs        # OuroborosProtocol trait, Phase enum
│   │       ├── interview.rs       # Interview result types
│   │       ├── seed.rs            # Seed struct, AmbiguityScore
│   │       ├── evaluation.rs       # Evaluation result types
│   │       └── ouroboros_engine.rs # LLM-backed protocol implementation
│   │
│   ├── oxios-gateway/         # Channel-agnostic message router
│   │   └── src/
│   │       ├── lib.rs           # Public exports
│   │       ├── gateway.rs       # Gateway struct, route(), run()
│   │       ├── channel.rs       # Channel trait definition
│   │       └── message.rs      # IncomingMessage, OutgoingMessage
│   │
│   └── oxios/                 # Main binary
│       └── src/main.rs         # CLI, kernel init, server startup
│
├── channels/
│   └── oxios-web/             # Web dashboard (first channel)
│       ├── src/
│       │   ├── lib.rs         # Public exports
│       │   ├── server.rs      # Axum HTTP server
│       │   ├── routes.rs      # API route handlers (all endpoints)
│       │   └── channel.rs     # WebChannel impl of Channel trait
│       └── static/
│           ├── index.html     # Dashboard UI
│           └── default-config.toml
│
└── docs/
```

### Dependencies

Oxios depends on the oxi engine from the sibling `pi2oxi` repository:

| Dependency | Role |
|-----------|------|
| `pi2oxi/oxi-ai` | Multi-provider LLM interface |
| `pi2oxi/oxi-agent` | Tool-calling agent runtime |

These are consumed as path dependencies — never reimplemented.

### Architecture Notes

**Message flow:**
```
User → WebChannel → Gateway → Orchestrator → OuroborosEngine
                                                         ↓
                                                  Supervisor
                                                         ↓
                                               AgentRuntime (oxi-agent)
                                                         ↓
                                               ProgramManager (capabilities)
                                                         ↓
                                               Tools (read/write/edit/bash)
                                                         ↓
                                               Result ← Gateway ← WebChannel ← User
```

**Execution model:**
```
Agent needs to run a command
        ↓
ExecTool::call() invoked
        ↓
  ├── workspace mode → shell exec in workspace directory
  └── structured mode → binary must be in allowed_commands allowlist
        ↓
AccessManager checks permissions → allow / deny
        ↓
Result returned to agent
```

**Program lifecycle:**
```
oxios program install ./my-program  → Parse program.toml, copy to ~/.oxios/workspace/programs/
POST /api/programs                   → Same via API
Agent queries program               → Load SKILL.md, use tool definitions
```

---

## Programs (OS-level applications)

Programs are installable application packages for Oxios agents. They provide
structured capabilities with metadata for discovery.

### Structure

```
program-name/
├── program.toml     # Metadata (name, version, tools, dependencies)
├── SKILL.md        # Instruction file (agent-facing docs)
├── bin/            # Optional executables
└── config/        # Optional configs
```

### Usage

```bash
# Install
oxios program install ./code-review

# List
oxios program list

# Enable/Disable
oxios program enable code-review
oxios program disable code-review

# Uninstall
oxios program uninstall code-review
```

---

## License

MIT
