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
