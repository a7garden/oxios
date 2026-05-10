# Oxios — User Guide

> AI Agent Operating System. Unix philosophy + Ouroboros methodology.

---

## Overview

Oxios is an AI Agent Operating System that runs as a **24/7 daemon**.

```
User (Web/CLI) → Gateway → Kernel → Agent → Response
                     ↑
              Cron scheduling (time-based triggers)
```

**Key Concepts:**
- **Ouroboros Protocol**: interview → seed → execute → evaluate → evolve
- **ExecTool**: direct host command execution (allowlist + metachar blocking)
- **Workspace Sandbox**: directory-based isolation, RBAC + audit logging

---

## Installation

### 1. Build

```bash
# From source
git clone https://github.com/your-repo/oxios
cd oxios
cargo build --release

# Binary will be at: target/release/oxios
```

### 2. Environment Variables

```bash
export ANTHROPIC_API_KEY=sk-ant-...
# or
export OPENAI_API_KEY=sk-...
```

### 3. Run

```bash
oxios
# → Web UI opens at http://127.0.0.1:4200
# → Runs 24/7 in background
```

### 4. Daemon Management

```bash
oxios daemon status    # Check status
oxios daemon restart   # Restart
```

---

## Web UI

Open `http://127.0.0.1:4200` in your browser.

```
┌─────────────────────────────────────────────────────────┐
│  🌿 Oxios                              [Persona ▼]       │
├──────────┬──────────────────────────────────────────────┤
│ 💬 Chat  │                                              │
│ 👥 Agents│  ┌──────────────────────────────────────┐   │
│ 📅 Cron  │  │ Welcome to Oxios. How can I help you? │   │
│ 📁 Programs│ │                                      │   │
│ 🎯 Memory │  └──────────────────────────────────────┘   │
│ ⚙️ Config │                                              │
└──────────┴──────────────────────────────────────────────┘
```

**Usage:**
1. Type a message in Chat panel
2. Agent works using Ouroboros protocol
3. View results

---

## API

### Chat

```bash
curl -X POST http://127.0.0.1:4200/api/chat \
  -H "Content-Type: application/json" \
  -d '{"content": "Build a TODO app in Rust", "user_id": "user1"}'
```

### Agent Management

```bash
# List running agents
curl http://127.0.0.1:4200/api/agents

# Kill agent
curl -X POST http://127.0.0.1:4200/api/agents/<id>/kill
```

### Status

```bash
curl http://127.0.0.1:4200/api/status
```

---

## CLI

### Interactive Mode

```bash
oxios chat
```

### Single Prompt

```bash
oxios run "Build a TODO app in Rust"
```

### Config

```bash
oxios config show
oxios config get exec.allowed_commands
```

---

## Cron Jobs

Schedule agents to run on a schedule.

### Via Config

`~/.oxios/config.toml`:

```toml
[cron]
enabled = true
tick_interval_secs = 60

[cron.jobs.morning_news]
schedule = "0 9 * * *"
goal = "Summarize top tech news"
priority = "low"
```

### Via API

```bash
curl -X POST http://127.0.0.1:4200/api/cron-jobs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "news-summary",
    "schedule": "0 9 * * *",
    "goal": "Summarize top tech news"
  }'
```

---

## Programs

Programs are installable apps for agents.

### Install

```bash
oxios program install ./my-program
```

### List

```bash
oxios program list
```

### Enable/Disable

```bash
oxios program enable my-program
oxios program disable my-program
```

### Structure

```
my-program/
├── program.toml     # Metadata
├── SKILL.md        # Agent instructions
├── bin/            # Executables (optional)
└── config/         # Config (optional)
```

---

## Configuration

Config file: `~/.oxios/config.toml`

### Full Example

```toml
[kernel]
workspace = "~/.oxios/workspace"
max_agents = 10

[gateway]
host = "127.0.0.1"
port = 4200

[exec]
# Allowed host commands (empty = allow all, dev mode)
allowed_commands = ["git", "gh", "open", "osascript"]
# Default timeout (seconds)
default_timeout_secs = 120
# Max timeout (seconds)
max_timeout_secs = 600
# Required host tools (checked at startup)
required_host_tools = ["git"]
# Optional host tools (checked when needed)
optional_host_tools = ["gh", "osascript", "shortcuts", "remindctl"]

[scheduler]
max_concurrent = 5
rate_limit_per_minute = 60
zombie_timeout_secs = 300

[security]
auth_enabled = false
max_execution_time_secs = 300
```

---

## Tools

Tools available to agents:

| Tool | Description |
|------|-------------|
| `exec` (shell) | Run bash commands in workspace |
| `exec` (structured) | Run allowlisted host commands |
| `read` | Read files |
| `write` | Write files |
| `edit` | Edit files |
| `grep` | Search text |
| `find` | Find files |
| `ls` | List directories |

---

## Memory

Agents can remember across sessions:

```bash
# Store
curl -X PUT http://127.0.0.1:4200/api/memory/my-note \
  -d '{"category": "notes", "content": "Important info"}'

# Search
curl http://127.0.0.1:4200/api/memory
```

---

## Audit Log

All tool usage is audited:

```bash
curl http://127.0.0.1:4200/api/audit
```

Response:
```json
[
  {
    "timestamp": "2026-05-10T18:00:00Z",
    "agent_name": "agent-123",
    "action": "exec",
    "resource": "bash -c 'git status'",
    "allowed": true,
    "reason": null
  }
]
```

---

## Troubleshooting

### Daemon won't start

```bash
# Check logs
RUST_LOG=debug oxios 2>&1

# Check port
lsof -i :4200
```

### Agent stuck

```bash
# List agents
oxios agent list

# Kill agent
oxios agent kill <id>
```

### Config errors

```bash
# Validate config
oxios config show

# Reset to defaults
rm ~/.oxios/config.toml
oxios  # Auto-regenerates
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic (Claude) API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `OXIOS_API_KEY` | Oxios API key (optional) |
| `RUST_LOG` | Log level (`info`, `debug`) |

---

## File Structure

```
~/.oxios/
├── config.toml              # Config
├── workspace/               # Workspace
│   ├── memory/             # Agent memory
│   │   ├── knowledge/     # Knowledge base
│   │   └── conversations/ # Session logs
│   ├── sessions/          # Sessions
│   ├── seeds/             # Ouroboros seeds
│   ├── skills/            # Skill templates
│   └── programs/         # Installed programs
└── api-keys.json         # API keys (production)
```

---

## See Also

- `README.md` — Installation and development guide
- `DESIGN.md` — Architecture and design decisions
- `AGENTS.md` — AI agent development conventions