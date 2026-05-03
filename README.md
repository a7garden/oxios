# Oxios — Agent Operating System

> *"Do one thing well. Specify before you build. Evolve, don't repeat."*

Oxios is an Agent Operating System built in Rust. It combines Unix philosophy (minimal composable tools) with Ouroboros methodology (specification-first workflows) to create an OS where AI agents execute real work on behalf of users.

**Engine:** `oxi-ai` + `oxi-agent` from `pi2oxi` are consumed as path dependencies. Never reimplement what oxi already provides.

**Runtime:** Apple Container on macOS Silicon. Linux support is deferred.

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
│  │  tools: read, write, edit, bash, grep, find, ls         │   │
│  │  LLM: oxi-ai (multi-provider)                           │   │
│  └────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
                      │
                      ▼
┌────────────────────────────────────────────────────────────────┐
│              Container Garden (Apple Container)                │
│              macOS Silicon only                                │
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

[container]
garden_path = "~/.oxios/gardens"
allowed_host_commands = ["git", "gh"]
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

# Garden management
oxios garden new <name>       # Create a new garden workspace
oxios garden up <name>       # Start a garden container
oxios garden down <name>      # Stop a garden container
oxios garden remove <name>   # Remove a garden
oxios garden list            # List all gardens
oxios garden exec <name> -- cmd args...  # Execute command in garden

# System
oxios status                 # Show system status
oxios config show            # Show current configuration
oxios config get <key>       # Get a config value
```

### Options

```bash
oxios --help                 # Show help
oxios -c ~/.oxios.toml       # Use custom config path
oxios -v                     # Verbose logging (debug level)
oxios garden new myapp       # Create garden "myapp"
oxios garden up myapp        # Start the garden
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

### `[container]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `garden_path` | String | `~/.oxios/gardens` | Base directory for gardens |
| `image_tag` | String | `"oxios:latest"` | Default container image |
| `allowed_host_commands` | Vec\<String\> | `[]` (all allowed) | Whitelist for host exec |
| `memory_limit` | String | `"4g"` | Default memory limit |
| `cpu_limit` | u64 | `4` | Default CPU limit |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
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
```

```
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
GET  /api/seeds              → [{ id, goal, constraints_count, created_at }]
GET  /api/seeds/:id          → seed JSON
```

### Memory

```
GET  /api/memory             → [{ name, category }]
GET  /api/memory/:name       → { name, category, content }
```

### Gardens

```
GET    /api/gardens                 → [{ name, image_tag, running, created_at }]
POST   /api/gardens                 → { name } → garden summary
POST   /api/gardens/:name/start    → { status, name }
POST   /api/gardens/:name/stop     → { status, name }
DELETE /api/gardens/:name          → { status, name }
POST   /api/gardens/:name/exec     → { command, workdir? } → { stdout, stderr, exit_code, duration_ms }
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
├── crates/
│   ├── oxios-kernel/          # Core: supervisor, event bus, state store, container
│   │   └── src/
│   │       ├── lib.rs         # Public exports
│   │       ├── supervisor.rs  # Agent lifecycle (fork/exec/wait/kill)
│   │       ├── event_bus.rs   # Broadcast event bus (KernelEvent)
│   │       ├── state_store.rs # Markdown-based persistent state
│   │       ├── config.rs      # TOML configuration
│   │       ├── container.rs   # Apple Container backend
│   │       ├── garden.rs      # Garden lifecycle manager
│   │       ├── host_exec.rs   # Secure host command execution bridge
│   │       ├── orchestrator.rs # Ouroboros lifecycle coordinator
│   │       └── agent_runtime.rs # oxi-agent wrapper for seed execution
│   ├── oxios-ouroboros/       # Ouroboros spec-first protocol
│   │   └── src/
│   │       ├── lib.rs         # Public exports
│   │       ├── protocol.rs   # OuroborosProtocol trait, Phase enum
│   │       ├── interview.rs  # Interview result types
│   │       ├── seed.rs       # Seed struct, AmbiguityScore
│   │       ├── evaluation.rs # Evaluation result types
│   │       └── ouroboros_engine.rs # LLM-backed protocol implementation
│   ├── oxios-gateway/         # Channel-agnostic message router
│   │   └── src/
│   │       ├── lib.rs         # Public exports
│   │       ├── gateway.rs    # Gateway struct, route(), run()
│   │       ├── channel.rs    # Channel trait definition
│   │       └── message.rs   # IncomingMessage, OutgoingMessage
│   └── oxios/                 # Main binary
│       └── src/main.rs        # CLI, kernel init, server startup
├── channels/
│   ├── oxios-web/             # Web dashboard (first channel)
│   │   ├── src/
│   │   │   ├── lib.rs        # Public exports
│   │   │   ├── server.rs     # Axum HTTP server
│   │   │   ├── routes.rs    # API route handlers
│   │   │   └── channel.rs   # WebChannel impl of Channel trait
│   │   └── static/
│   │       ├── index.html    # Dashboard UI
│   │       ├── default-config.toml
│   │       └── Containerfile
├── AGENTS.md                  # AI agent conventions
├── DESIGN.md                  # Architecture and design decisions
└── README.md                  # This file
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
                                                  Tools (read/write/edit/bash)
                                                         ↓
                                                  Result ← Gateway ← WebChannel ← User
```

**Garden lifecycle:**
```
oxios garden new myapp     → Create directory structure + Containerfile
oxios garden up myapp      → Apple Container run → garden running
oxios garden exec myapp -- cmd   → container exec → result
oxios garden down myapp    → container stop/delete
oxios garden remove myapp  → Delete directory + metadata
```

---

## License

MIT
