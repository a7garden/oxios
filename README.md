<div align="center">

# ⬡ Oxios

**Agent Operating System**

*Where AI agents don't just talk — they work.*

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/Version-0.4.0-6E40C9?logo=rust&logoColor=white)](https://crates.io/crates/oxios)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Lines of Code](https://img.shields.io/badge/LOC-67K%2B-00A86B?logo=rust&logoColor=white)]()
[![GitHub](https://img.shields.io/badge/GitHub-a7garden%2Foxios-181717?logo=github)](https://github.com/a7garden/oxios)

**Built with**

[![oxi](https://img.shields.io/badge/oxi-agent_runtime-6E40C9?logo=rust&logoColor=white)](https://github.com/a7garden/oxi)
&nbsp;
[![oxibrowser](https://img.shields.io/badge/oxibrowser-headless_browser-00A86B?logo=rust&logoColor=white)](https://github.com/a7garden/oxibrowser)
&nbsp;
[![ouroboros](https://img.shields.io/badge/ouroboros-specification_framework-E95420?logo=rust&logoColor=white)](https://github.com/Q00/ouroboros)

[Getting Started](#getting-started) · [Architecture](#architecture) · [Core Concepts](#core-concepts) · [CLI Reference](#cli-reference) · [REST API](#rest-api) · [Ecosystem](#ecosystem)

</div>

---

## Table of Contents

- [Why Oxios?](#why-oxios)
- [Getting Started](#getting-started)
- [Architecture](#architecture)
- [Core Concepts](#core-concepts)
  - [Ouroboros Protocol](#-ouroboros-protocol)
  - [Supervisor](#-supervisor)
  - [Scheduler](#-scheduler)
  - [Built-in Browser](#-built-in-browser)
  - [Skills](#-skills)
  - [Vector Memory](#-vector-memory)
  - [Spaces](#-spaces)
  - [Security Model](#-security-model)
  - [MCP & A2A](#-mcp--a2a)
  - [Persona System](#-persona-system)
  - [Circuit Breaker](#-circuit-breaker)
  - [Git Integration](#-git-integration)
  - [Cron Scheduler](#-cron-scheduler)
  - [Budget Manager](#-budget-manager)
  - [Resource Monitor](#-resource-monitor)
- [CLI Reference](#cli-reference)
- [REST API](#rest-api)
- [Project Structure](#project-structure)
- [Ecosystem](#ecosystem)
- [Contributing](#contributing)
- [License](#license)

---

## Why Oxios?

Large language models are powerful, but they're stuck in chat boxes. Oxios gives them an **operating system** — lifecycle management, tool execution, state persistence, security boundaries, and an orchestration protocol — so agents can autonomously complete real tasks.

| The Problem | What Oxios Does |
|---|---|
| Agents die when the chat closes | **Supervisor** manages agent lifecycle: fork, exec, wait, kill |
| No specification → unreliable output | **Ouroboros**: interview → seed → execute → evaluate → evolve |
| Every app reinvents browser/execution | **Built-in engine**: headless browser, host exec, MCP bridge, skills |
| Agents have no memory between sessions | **Vector memory**: persistent, searchable knowledge with semantic recall |
| No security boundary between agents | **Access Manager**: RBAC, path sandboxing, Merkle-chain audit trail |
| LLM provider outages cascade | **Circuit Breaker**: 3-state protection against cascading failures |
| No protocol for agent-to-agent work | **A2A**: Google's agent-to-agent protocol for horizontal communication |

**~67,000 lines of Rust. 196+ source files. Zero containers. Zero subprocess browsers.** Everything runs in-process.

---

## Getting Started

### Install

```bash
cargo install oxios
```

### Configure

Set your LLM provider key:

```bash
# Anthropic (Claude)
export ANTHROPIC_API_KEY=sk-ant-...

# or OpenAI (GPT)
export OPENAI_API_KEY=sk-...
```

On first run, Oxios launches an interactive setup wizard to configure your workspace, credentials, and preferences.

### Run

```bash
oxios                    # Start the daemon (background by default)
oxios --foreground       # Run in foreground (for debugging)
```

Open **http://127.0.0.1:4200** — start talking to your agent.

### Quick Commands

```bash
oxios run --json "review this code"        # Single-shot with JSON output
oxios chat                                 # Interactive chat session
oxios status                               # Check daemon status
oxios doctor                               # Diagnose configuration issues
oxios models                               # List available LLM models
```

That's it. The OS handles the rest.

---

## Architecture

```
┌───────────────────── Channels ─────────────────────┐
│                                                     │
│   Web (Axum)  ·  CLI  ·  Telegram  ·  Discord  …   │
│              (plugin-based, feature-gated)           │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│                    Gateway                           │
│          Channel-agnostic message hub                │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│                    Kernel                            │
│                                                      │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │
│  │  Supervisor  │ │  Ouroboros   │ │  Scheduler   │  │
│  │ fork/exec/   │ │  Orchestrator│ │ Priority queue│  │
│  │ wait/kill    │ │  (protocol)  │ │ (AIOS-style) │  │
│  └─────────────┘ └──────────────┘ └──────────────┘  │
│                                                      │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │
│  │   Memory     │ │ Access Mgr   │ │  AuditTrail  │  │
│  │ Vector store │ │ RBAC + paths │ │ Merkle-chain │  │
│  │ HNSW + TF-IDF│ │ + sandboxing │ │ (blake3)     │  │
│  └─────────────┘ └──────────────┘ └──────────────┘  │
│                                                      │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │
│  │    Budget    │ │    Cron      │ │  Resource    │  │
│  │ Token/cost   │ │  Scheduler   │ │   Monitor    │  │
│  │ enforcement  │ │  (jobs)      │ │  (system)    │  │
│  └─────────────┘ └──────────────┘ └──────────────┘  │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │              Agent Runtime                      │  │
│  │  oxi-agent + oxi-ai (multi-provider)            │  │
│  │  read · write · edit · bash · grep · browser   │  │
│  │  skills · MCP · memory · A2A · git             │  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │
│  │OxiBrowser   │ │  GitLayer    │ │ CircuitBreaker│  │
│  │In-process   │ │  (gix)       │ │ 3-state LLM  │  │
│  │~10MB        │ │  version ctrl│ │  protection  │  │
│  └─────────────┘ └──────────────┘ └──────────────┘  │
└──────────────────────────────────────────────────────┘
         │
    ┌────▼────┐
    │  Host   │
    │  Exec   │
    └─────────┘
```

**No containers. No subprocess browser.** Everything runs in-process, sandboxed by workspace rules and RBAC. The kernel exposes all functionality through `KernelHandle` — a facade with typed APIs (Agent, Space, Security, Persona, Exec, Browser, MCP, Extension, Infra, A2A, State, KnowledgeBase, KnowledgeLens).

---

## Core Concepts

### 🔄 Ouroboros Protocol

Powered by the [Ouroboros specification framework](https://github.com/Q00/ouroboros). Agents never execute blindly — every task starts with a specification that evolves through cycles.

```
┌─────────────────────────────────────────────┐
│                                             │
│  Interview ──► Seed ──► Execute ──► Evaluate│
│       ▲                            │        │
│       │                            ▼        │
│       └──────── Evolve ◄────────────────────┘
│                                             │
└─────────────────────────────────────────────┘
```

| Phase | What Happens |
|-------|-------------|
| **Interview** | Agent asks clarifying questions to understand the task |
| **Seed** | Generates a formal specification (the "seed") |
| **Execute** | Implements the spec using available tools |
| **Evaluate** | Validates the output against the specification |
| **Evolve** | Refines the spec based on results, then loops |

The Ouroboros protocol is the heart of Oxios. It ensures agents produce **reliable, spec-driven output** rather than improvising solutions.

### 🧭 Supervisor

Agent lifecycle as Unix-style process management. The Supervisor is the "init" of Oxios — it manages the full lifecycle of every agent process.

```
fork() → register(A2A) → check_permissions() → schedule() → run() → cleanup()
```

Operations: `fork`, `exec`, `wait`, `kill`. The `AgentLifecycleManager` orchestrates the complete flow from agent creation through A2A registration, permission checks, scheduling, execution, and cleanup.

### 📊 Scheduler

Priority-based task queue inspired by [AIOS](https://arxiv.org/abs/2403.16971) and [AgentRM](https://arxiv.org/abs/2408.01567). Features:

- Rate-limit-aware admission control
- Zombie agent detection and cleanup
- Maximum concurrent agent enforcement
- Priority-based scheduling for multi-agent workloads

### 🌐 Built-in Browser

[OxiBrowser](https://github.com/a7garden/oxibrowser) — a **pure Rust headless browser** running in-process. ~10MB memory footprint. No Chromium. No CDP overhead.

```
"Read this URL"    →  browse(url)              →  Markdown (one-shot)
"Fill this form"   →  goto → click → type      →  Interactive tab session
"Run this JS"      →  evaluate(code)            →  JSON result
"Extract data"     →  extract(selector)         →  Structured output
```

Agents can browse the web, fill forms, extract data, and execute JavaScript — all without leaving the process.

### 🛠️ Skills

Unified skill system — every capability is a SKILL.md with YAML frontmatter. Skills unify the former Programs and Skills concepts, providing a single model for agent instructions with requirements, install specs, and invocation policy.

```yaml
---
name: code-review
description: Deep code review with quality domain analysis
requires:
  bins: ["git"]
  env: ["GITHUB_TOKEN"]
install:
  - kind: brew
    formula: git
---
```

Skill sources (highest to lowest priority): agent-specific → workspace → global user → bundled.
Built-in bundled skills include: `code-review`, `debug`, and `refactor`.

### 🧠 Tiered Memory

Agents remember across sessions with a 3-tier memory system (Hot/Warm/Cold) and automatic Dream-time consolidation:

| Component | Purpose |
|-----------|---------|
| **Memory Tiers** | Hot (always loaded, ~3K tokens) → Warm (on-demand) → Cold (compressed archive) |
| **Dream Process** | 4-phase background consolidation: Orient → Gather Signal → Consolidate → Prune & Index |
| **Auto-Classification** | Infers memory type (Fact, Decision, Episode, etc.) from content patterns |
| **Auto-Protection** | Automatically promotes importance based on access frequency and session appearances |
| **Decay Engine** | Ebbinghaus-inspired forgetting curve with protection-aware rate adjustment |
| **Compaction Tree** | Raw → Daily → Weekly → Monthly → Root progressive compression |
| **ROOT Index** | O(1) topic lookup — agents know what they know without scanning |
| **Proactive Recall** | Automatically injects relevant memories at session start and topic transitions |
| **HNSW + TF-IDF** | Semantic vector search with term-frequency embeddings |
| **Reasoning Bank** | Stores and retrieves agent reasoning chains |

### 🗂️ Spaces

Conversation context management with intelligent auto-detection:

- **Space Manager** — CRUD for conversation contexts
- **Conversation Buffer** — Manages context window and history
- **Knowledge Bridge** — Auto-extracts knowledge from conversations
- **Detection** — Intent classification for automatic space routing

Spaces let agents maintain focused, topic-specific conversations without cross-contamination.

### 🔒 Security Model

Defense in depth — multiple security layers working together:

| Layer | Mechanism | Details |
|-------|-----------|---------|
| **Tool Access** | RBAC per agent | Capability-based permissions |
| **File System** | Workspace path sandboxing | Agents can't escape their workspace |
| **Network** | SSRF protection | Private IP blocking, robots.txt obedience |
| **Execution** | Command allowlist | `shell` mode (RBAC) and `structured` mode (binary allowlist + metacharacter blocking) |
| **Audit** | Merkle-chain audit trail | Tamper-evident, blake3-hashed, cryptographically linked entries |
| **Identity** | Authentication manager | Token-based identity verification for all API calls |
| **Sandbox** | WASM sandbox | Execute untrusted code in isolated WebAssembly environment |

The `AccessManager` follows OWASP-inspired least-privilege principles. Every tool call passes through permission checks.

### 🔌 MCP & A2A

**MCP (Model Context Protocol)** — Connect to external tool servers using Anthropic's open protocol. Oxios includes a full MCP client, protocol handler, and server integration.

**A2A (Agent-to-Agent)** — Google's protocol for inter-agent communication. Agents can discover, negotiate, and collaborate with each other horizontally — no central orchestrator required.

### 🎭 Persona System

Multiple AI characters, each with their own personality and expertise:

| Persona | Role |
|---------|------|
| **Dev** | Software development, coding, implementation |
| **Review** | Code review, quality analysis, best practices |
| **Research** | Investigation, analysis, information gathering |

Personas are fully customizable — create your own via the API or CLI.

### ⚡ Circuit Breaker

3-state protection against LLM provider failures:

```
Closed ──(errors exceed threshold)──► Open ──(timeout)──► Half-Open
   ▲                                    │                    │
   └──────(success)─────────────────────┘◄──(probe)─────────┘
```

Prevents cascading failures when an LLM provider goes down. Automatically recovers via probing.

### 🔧 Git Integration

In-process version control powered by [gix](https://github.com/Byron/gitoxide):

- Commits, logs, tags, restore
- No external `git` binary required
- All operations run in-process
- Workspace changes are tracked automatically

### ⏰ Cron Scheduler

Scheduled job execution with persistent state:

```bash
# Cron jobs are managed via config.toml and API
oxios config show                         # View cron config section
curl http://127.0.0.1:4200/api/cron-jobs   # List scheduled jobs
```

### 💰 Budget Manager

Token and cost budget enforcement per agent:

- Set spending limits per agent
- Reserve budget before expensive operations
- Automatic enforcement and reset
- Prevent runaway API costs

### 📈 Resource Monitor

System resource tracking for agent budget enforcement:

- CPU and memory snapshots
- Historical resource usage
- Overload detection

---

## CLI Reference

```text
oxios                    Start the daemon (background by default)
oxios start              Start the daemon
oxios stop               Stop the daemon
oxios restart            Restart the daemon
oxios status             Show daemon status
oxios doctor             Diagnose configuration issues
oxios run <prompt>       Single-shot execution
oxios chat <prompt>      Interactive chat session
oxios config             View/edit configuration
oxios pkg                Package management
oxios agent              Agent management
oxios audit              Audit trail inspection
oxios git                Git operations (log, tags, restore, verify)
oxios budget             Budget management
oxios daemon             Daemon management (install as system service)
oxios log                View logs
oxios skill <name>       View skill details & SKILL.md
oxios skills             List all skills with eligibility status
oxios models             List available LLM models
oxios backup             Backup workspace
oxios restore            Restore from backup
oxios onboard            Re-run setup wizard
oxios reset              Reset all Oxios data (config, workspace, credentials, services)
                            Prompts for confirmation and shows what will be deleted before proceeding
oxios completion         Generate shell completions
```

### Programmatic Usage

`oxios run` is designed for scripts and agents:

```bash
# JSON output — parse response, session_id, evaluation status
oxios run --json "review this code"

# Pass file as context (stdin)
cat file.rs | oxios run --json --context-file - "describe this"

# Exit codes for CI: 0=passed, 1=failed
oxios run --exit-code --json "run all tests"
echo $?

# Multi-turn sessions
SID=$(oxios run --json "initial prompt" | jq -r '.session_id')
oxios run --json --session "$SID" "follow-up question"
```

**JSON output shape:**

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

## REST API

Full REST API on **port 4200** with 76 endpoints. Auth middleware on all `/api/*` routes.

### Chat & Streaming

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/chat` | Send a message |
| `GET` | `/api/chat/stream` | WebSocket streaming |

### Agents

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/agents` | List running agents |
| `POST` | `/api/agents/{id}/kill` | Kill an agent |
| `GET` | `/api/agent-groups` | List agent groups |
| `GET` | `/api/agent-groups/{id}` | Get group details |

### System

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check (no auth) |
| `GET` | `/api/status` | System status |
| `GET` | `/api/config` | Get configuration |
| `PUT` | `/api/config` | Update configuration |
| `GET` | `/api/metrics` | Prometheus metrics |

### Workspace

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/workspace/tree` | File tree |
| `GET` | `/api/workspace/file/{path}` | Read file |
| `PUT` | `/api/workspace/file/{path}` | Write file |

### Seeds & Skills

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/seeds` | List seeds |
| `GET` | `/api/seeds/{id}` | Get seed details |
| `GET` | `/api/seeds/{id}/evolution` | Seed evolution history |
| `GET` | `/api/skills` | List skills |
| `GET` | `/api/skills/{name}` | Get skill details |
| `POST` | `/api/skills` | Create skill |
| `DELETE` | `/api/skills/{name}` | Delete skill |

### Memory

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/memory` | List memories |
| `POST` | `/api/memory` | Create memory |
| `GET` | `/api/memory/{id}` | Get memory |
| `POST` | `/api/memory/search` | Keyword search |
| `POST` | `/api/memory/semantic` | Semantic search |
| `GET` | `/api/memory/tiers` | List memories by tier |

### Skills

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/skills` | List all skills with eligibility status |
| `GET` | `/api/skills/{name}` | Get skill details & requirements check |
| `POST` | `/api/skills` | Create skill |
| `DELETE` | `/api/skills/{name}` | Delete skill |
| `POST` | `/api/skills/{name}/enable` | Enable skill |
| `POST` | `/api/skills/{name}/disable` | Disable skill |
| `GET` | `/api/skills/{name}/content` | Get skill SKILL.md content |

### Scheduler & Audit

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/scheduler/stats` | Scheduler statistics |
| `GET` | `/api/scheduler/tasks` | List scheduled tasks |
| `GET` | `/api/audit/entries` | Audit log entries |
| `GET` | `/api/audit/verify` | Verify audit chain integrity |
| `GET` | `/api/audit/agent/{id}` | Audit entries by agent |
| `POST` | `/api/audit/export` | Export audit log |
| `POST` | `/api/audit/flush` | Flush audit log |

### Permissions

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/permissions/{agent}` | Get agent permissions |
| `PUT` | `/api/permissions/{agent}` | Update agent permissions |

### Sessions & Events

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/sessions` | List sessions |
| `GET` | `/api/sessions/{id}` | Get session details |
| `DELETE` | `/api/sessions/{id}` | Delete session |
| `GET` | `/api/events` | SSE event stream |

### Personas

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/personas` | List personas |
| `POST` | `/api/personas` | Create persona |
| `GET` | `/api/personas/{id}` | Get persona |
| `PUT` | `/api/personas/{id}` | Update persona |
| `DELETE` | `/api/personas/{id}` | Delete persona |
| `GET` | `/api/personas/active` | Get active persona |
| `PUT` | `/api/personas/active` | Set active persona |

### Cron Jobs

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/cron-jobs` | List cron jobs |
| `POST` | `/api/cron-jobs` | Create cron job |
| `GET` | `/api/cron-jobs/{id}` | Get cron job |
| `DELETE` | `/api/cron-jobs/{id}` | Delete cron job |
| `POST` | `/api/cron-jobs/{id}/edit` | Edit cron job |
| `POST` | `/api/cron-jobs/{id}/trigger` | Trigger cron job |

### Approvals (Human-in-the-Loop)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/approvals` | List pending approvals |
| `POST` | `/api/approvals/{id}/approve` | Approve request |
| `POST` | `/api/approvals/{id}/reject` | Reject request |

### Git

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/git/log` | Commit log |
| `GET` | `/api/git/tags` | List tags |
| `POST` | `/api/git/verify` | Verify repository integrity |
| `POST` | `/api/git/restore` | Restore from commit |

### Budget

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/budget/{agent_id}` | Get agent budget |
| `POST` | `/api/budget/{agent_id}` | Set agent budget |
| `DELETE` | `/api/budget/{agent_id}` | Remove agent budget |
| `POST` | `/api/budget/{agent_id}/reserve` | Reserve budget |
| `POST` | `/api/budget/{agent_id}/reset` | Reset budget |

### Resources

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/resources` | Current resource snapshot |
| `GET` | `/api/resources/history` | Historical resource usage |
| `GET` | `/api/resources/overload` | Check overload status |

---

## Project Structure

```
oxios/                          # Main binary (src/main.rs)
├── crates/
│   ├── oxios-kernel/           # Core: supervisor, scheduler, event bus, state store, tools, tiered memory
│   ├── oxios-ouroboros/        # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   └── oxios-gateway/          # Channel-agnostic message hub
├── channels/
│   ├── oxios-web/              # Web dashboard (Axum backend + React frontend)
│   ├── oxios-cli/              # CLI channel
│   └── oxios-telegram/         # Telegram channel
├── share/                      # Default skills, config, migration scripts
└── docs/                       # Architecture docs, RFCs, design documents
```

**Dependency graph:**

```
oxios ──► oxios-kernel ──► oxi-sdk (crates.io)
                       ──► oxi-ai (provider construction)
                       ──► oxios-ouroboros
      ──► oxios-gateway
      ──► oxios-web / oxios-cli / oxios-telegram (feature-gated channels)
```

**File locations:**

| Path | Purpose |
|------|---------|
| `~/.oxios/` | Oxios home directory |
| `~/.oxios/config.toml` | Main configuration |
| `~/.oxios/workspace/` | Agent working directory |
| `~/.oxios/workspace/sessions/` | Session data |
| `~/.oxios/workspace/seeds/` | Ouroboros seed specifications |
| `~/.oxios/workspace/skills/` | Unified skill definitions |

---

## Ecosystem

Oxios is part of the **a7garden** Rust AI stack — a collection of focused crates that compose into a complete agent platform:

| Project | Purpose |
|---------|---------|
| [**oxi**](https://github.com/a7garden/oxi) | LLM engine + agent runtime (multi-provider, tool-calling loop) |
| [**oxibrowser**](https://github.com/a7garden/oxibrowser) | Pure Rust headless browser (~10MB, no Chromium) |
| [**ouroboros**](https://github.com/Q00/ouroboros) | Specification-first agent framework |
| **oxios** | Agent Operating System *(you are here)* |

**Layered architecture:**

```
oxi-ai ──── LLM abstraction (multi-provider: Anthropic, OpenAI, ...)
oxi-agent ── Tool-calling agent loop
  │
ouroboros ── Specification-first protocol
  │
oxios-kernel ── Supervisor, scheduler, tools, state, security, memory
  │
oxios ── Binary + channels (Web, CLI, Telegram, ...)
```

---

## Contributing

Contributions are welcome! The project follows these conventions:

- **Language:** Code, comments, docs, commits — English
- **Rust:** `#![warn(missing_docs)]` on public crates. `anyhow` for apps, `thiserror` for libs
- **Testing:** `cargo test --workspace` must pass at every commit
- **Commits:** `<type>(<scope>): <description>` — scopes: kernel, ouroboros, gateway, web, cli, docs
- **CI:** GitHub Actions (macOS-latest, fmt + clippy + test + audit)

See [AGENTS.md](AGENTS.md) for detailed onboarding documentation.

---

## License

[MIT](LICENSE) · [Third-Party Notices](THIRD-PARTY-NOTICES.md)

---

<div align="center">

*Built by [a7garden](https://github.com/a7garden)*

</div>
