# Oxios AGENTS.md

> Convention guide for AI agents working on this codebase.

## Project Overview

Oxios is an Agent Operating System built in Rust. It combines Unix philosophy (minimal composable tools) with Ouroboros methodology (specification-first workflows) to create an OS where AI agents execute real work on behalf of users.

**Engine:** oxi-ai + oxi-agent (from `../pi2oxi/`) are consumed as path dependencies. Never reimplement what oxi already provides.

**Runtime:** Apple Container on macOS Silicon. Linux support is deferred.

## Architecture

```
Gateway (channel-agnostic) → Kernel (supervisor + ouroboros + oxi-agent) → Container Garden
```

- Gateway: message hub, channels plug in as plugins (Web first, Telegram/CLI later)
- Kernel: agent lifecycle (fork/exec/wait/kill), event bus, state store, tool registry
- Ouroboros: spec-first protocol (interview → seed → execute → evaluate → evolve)
- Container: Apple Container isolation (garden per project)

## Kernel Modules

The `oxios-kernel` crate exposes these public modules:

| Module | Purpose |
|--------|---------|
| `supervisor` | Agent lifecycle management (fork/exec/wait/kill) |
| `event_bus` | KernelEvent broadcast channel |
| `state_store` | Markdown-based persistent state |
| `config` | TOML configuration (OxiosConfig) |
| `orchestrator` | Ouroboros lifecycle coordinator |
| `agent_runtime` | oxi-agent wrapper (tool-calling loop) |
| `container` | Apple Container backend |
| `garden` | Garden lifecycle manager |
| `host_exec` | Secure host command execution bridge |
| `program` | OS-level programs (installable capabilities) |
| `skill` | Markdown-based agent instruction templates |
| `mcp` | MCP (Model Context Protocol) awareness |
| `host_tools` | Host dependency validator |
| `scheduler` | AIOS-inspired task scheduler |
| `context_manager` | AIOS-inspired 3-tier context management |
| `access_manager` | OWASP-inspired security enforcement |

## Directory Structure

```
oxios/
├── crates/
│   ├── oxios-kernel/       Core: supervisor, event bus, state store
│   ├── oxios-ouroboros/    Spec-first protocol
│   ├── oxios-gateway/      Channel-agnostic message hub
│   └── oxios/              Main binary
├── channels/
│   └── oxios-web/          Web dashboard (first channel)
└── docs/
```

## Code Conventions

### Language

- All code, comments, docs, commit messages: **English**
- User-facing explanations to the operator: Korean

### Rust

- Edition 2021, MSRV: whatever stable Rust is current
- `#![allow(dead_code)]` only during scaffolding, remove before commit
- `#![warn(missing_docs)]` on public crates
- Error handling: `anyhow` for applications, `thiserror` for library crates
- Async: tokio runtime throughout
- Serialization: serde + serde_json for wire, toml for config

### Naming

- Crates: `oxios-<component>` (kebab-case)
- Modules: snake_case
- Types/traits: PascalCase
- Public API: verb_noun pattern (`fork_agent`, `send_message`, `create_seed`)

### Testing

- Unit tests in `#[cfg(test)] mod tests` within each file
- Integration tests in `tests/` directory per crate
- `cargo test --workspace` must pass at every commit

### Commits

```
<type>(<scope>): <description>

Types: feat, fix, refactor, test, docs, chore
Scopes: kernel, ouroboros, gateway, web, cli, docs
```

## Key Principles

1. **Unix philosophy:** Every component does one thing well. Compose small pieces.
2. **Ouroboros first:** Never execute without a spec. Interview → seed → execute → evaluate → evolve.
3. **No reimplementation:** Reuse oxi-ai and oxi-agent from pi2oxi. Reuse clawgarden code where applicable.
4. **Channel agnostic:** Gateway doesn't care if the message comes from Web, CLI, or Telegram.
5. **User invisible:** Users don't know how many agents are running. They talk, the OS handles the rest.
6. **Container minimalism:** Ship essential tools only; rich functionality comes from host dependencies.

## Dependency Map

```
oxios (binary)
├── oxios-kernel
│   ├── oxi-agent (from ../pi2oxi/oxi-agent)
│   │   └── oxi-ai (from ../pi2oxi/oxi-ai)
│   └── oxios-ouroboros
├── oxios-gateway
└── oxios-web (channel plugin)
```

## Reusable Code from ClawGarden

When implementing, check `../clawgarden/crates/` for reusable code:

| ClawGarden crate | Oxios equivalent | Reuse strategy |
|------------------|------------------|----------------|
| `clawgarden-proto/envelope.rs` | `oxios-kernel` event types | Adapt envelope/EventType |
| `clawgarden-proto/config.rs` | `oxios-kernel` config | Adapt layered config |
| `clawgarden-bus/supervisor.rs` | `oxios-kernel` supervisor | Direct adaptation |
| `clawgarden-bus/uds_server.rs` | `oxios-kernel` event bus | Adapt UDS protocol |
| `clawgarden-relay` | `oxios-kernel` host exec | Direct reuse |
| `clawgarden-cli/backend/apple.rs` | `oxios-kernel` container | Direct reuse |
| `clawgarden-memory` | `oxios-kernel` state store | Adapt knowledge/memory |

## Build & Run

```bash
cargo build                    # Build everything
cargo test --workspace         # Run all tests
cargo run                      # Run oxios
cargo build --release          # Release build
```

---

## Program Development Guide

Programs are the OS-level installable applications for Oxios. They provide structured capabilities that agents can leverage.

### Program Structure

A program is a directory containing:

```
my-program/
├── program.toml     # Metadata (required)
├── SKILL.md        # Instruction file (required)
├── bin/            # Optional: executable scripts
├── config/         # Optional: configuration files
└── README.md       # Optional: documentation
```

### program.toml Format

```toml
[program]
name = "my-program"
version = "1.0.0"
description = "What this program does"
author = "oxios"

# Tools this program exposes
[tools]
my_tool = { description = "What the tool does" }

# Host tool requirements (critical for container minimalism)
[host_requirements]
required = ["git", "curl"]
optional = ["gh", "osascript"]

# Container minimalism: tools that must be in the container
[container]
minimal_tools = ["bash", "jq", "sqlite3"]
```

### SKILL.md Format

```markdown
# My Program

## Purpose
Brief description of what this program does.

## Usage
How agents should use this program.

## Tools
- `my_tool`: Description of the tool

## Examples
```bash
example command
```
```

### Installation

Programs are installed via:

```bash
# CLI
oxios program install ./my-program

# API
POST /api/programs
{ "path": "./my-program" }
```

### Best Practices

1. **Keep programs focused** — One program, one responsibility (Unix philosophy)
2. **Document host dependencies** — Always specify required vs optional host tools
3. **Make SKILL.md comprehensive** — Agents use this to understand capability
4. **Version semantically** — Follow SemVer for program versions
5. **Test in garden** — Always test programs inside a garden container

---

## Container Minimalism Philosophy

Oxios containers ship **only essential tools**. Rich functionality comes from host macOS integration.

### Minimal Container Toolset

```toml
[container]
minimal_tools = [
    "bash",      # Shell scripting
    "curl",      # HTTP requests
    "git",       # Version control (via host mount)
    "ripgrep",   # Search
    "jq",        # JSON processing
    "sqlite3",   # Database
    "python3",   # Scripting (optional)
]
```

### Why Minimalism?

1. **Security** — Smaller attack surface
2. **Performance** — Faster container startup
3. **Reliability** — Predictable environment
4. **Unix philosophy** — Small pieces, composed

### Host Integration Layer

Rich capabilities come from the host via `host_exec`:

| Host Capability | Access Method |
|-----------------|----------------|
| Git operations | Via `git` (host-mounted) |
| GitHub CLI | Via `gh` command |
| Notifications | Via `osascript` or `remindctl` |
| Browser | Via `open` command |
| macOS automation | Via `shortcuts` or `osascript` |

### Host Tool Validation

The `HostToolValidator` ensures host dependencies are available:

```rust
pub struct HostToolValidator {
    required: Vec<String>,
    optional: Vec<String>,
}

impl HostToolValidator {
    /// Check all required tools are present on the host
    pub fn validate(&self) -> HostToolStatus;
    
    /// Get all available tools (required + optional)
    pub fn full_check(&self) -> FullCheckResult;
}
```

Access via API:

```
GET /api/host-tools
→ { all_required_present, missing_required, optional_available }
```

---

## Host Dependency Documentation

When developing features that depend on host tools, document them properly.

### Declaring Host Dependencies

In `program.toml`:

```toml
[host_requirements]
required = ["git"]           # Tools that MUST be available
optional = ["gh", "remindctl"] # Tools that enhance functionality
```

### Checking at Runtime

```rust
let check = host_tool_validator.check_host_requirements(&program_name).await;
if !check.all_required_present {
    // Warn user about missing required tools
    for tool in &check.missing_required {
        eprintln!("Warning: required tool '{}' not found on host", tool);
    }
}
```

### Common Host Tools

| Tool | Purpose | Required for |
|------|---------|--------------|
| `git` | Version control | Any git operations |
| `gh` | GitHub CLI | GitHub integration |
| `osascript` | AppleScript | macOS automation |
| `open` | Open files/URLs | Browser integration |
| `remindctl` | Reminders API | Notification features |
| `shortcuts` | Shortcuts app | Automation workflows |
| `sqlite3` | Database CLI | Database operations |

### Adding New Host Dependencies

1. **Document** in the feature's program.toml
2. **Validate** using HostToolValidator before use
3. **Gracefully degrade** if optional tools are missing
4. **Fail fast** if required tools are missing
5. **Log** access decisions to audit log via AccessManager
