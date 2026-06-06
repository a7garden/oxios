# Oxios AGENTS.md

> Onboarding document for AI agents working on this codebase.
> Hand-written. Every sentence is intentional. Do not auto-regenerate.

## What

Oxios is an **Agent Operating System** in Rust. AI agents fork, exec, wait, kill — just like Unix processes.

**Stack:** Rust 2021, tokio async, serde, oxi-sdk (crates.io).

```
User → Channel (Web/CLI/Telegram) → Gateway → Kernel
```

```
oxios/
├── crates/
│   ├── oxios-kernel/      # Supervisor, scheduler, memory, security, tools
│   ├── oxios-markdown/    # Knowledge base (VirtualFs, BacklinkIndex)
│   ├── oxios-ouroboros/   # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   ├── oxios-gateway/     # Channel-agnostic message hub
│   ├── oxios-mcp/         # MCP client (JSON-RPC 2.0 over stdio)
│   ├── oxios-memory/      # Tiered agent memory (Hot/Warm/Cold, Dream, HNSW)
│   └── oxios-calendar/    # .ics-based calendar event management
├── surface/oxios-web/     # Web dashboard (Axum + React)
├── channels/              # CLI, Telegram channels (feature-gated)
├── share/                 # Default skills, config
└── docs/                  # Architecture, RFCs, design documents
```

**Dependencies:** `oxios → oxios-kernel → {oxios-memory, oxios-ouroboros, oxios-markdown, oxios-calendar, oxios-mcp, oxi-sdk}`. `oxi-sdk` is a crates.io dependency — never reimplement what it provides.

## Quick Facts

| Fact | Value |
|------|-------|
| **Language** | Rust 2021 + TypeScript 5 (frontend) |
| **License** | MIT |
| **CI** | `cargo fmt && clippy -D warnings && cargo test --workspace` |
| **Build** | `cargo build && cd surface/oxios-web/web && bun run build` |
| **Test** | `cargo test --workspace` |

## Principles

- **Unix philosophy** — fork/exec/wait/kill for agents. Compose small pieces.
- **Ouroboros first** — never execute without a spec. Interview → seed → execute → evaluate → evolve.
- **No reimplementation** — reuse oxi-sdk from crates.io.
- **Channel agnostic** — gateway doesn't care where messages come from.
- **No containers** — direct host execution. Security via AccessManager (RBAC + path sandboxing).

## Commands

```bash
cargo build                                          # Build
cargo test --workspace                               # Test
cargo run                                            # Daemon (background)
cargo run -- --foreground                            # Daemon (foreground)
cargo run -- run --json "prompt"                     # Single-shot JSON execution
cd surface/oxios-web/web && bun install && bun dev   # Frontend dev server
```

## Conventions

- **Language:** Code, comments, docs, commits — English. User-facing messages — Korean.
- **Rust:** `anyhow` for apps, `thiserror` for libs. `#![warn(missing_docs)]` on public crates.
- **Naming:** Crates `oxios-<component>`, public API `verb_noun`.
- **Testing:** Unit tests in `#[cfg(test)] mod tests`. Integration tests in `tests/`.
- **Commits:** `<type>(<scope>): <description>` — scopes: kernel, ouroboros, gateway, web, cli, docs.

### Document Rules

**No analysis or progress files in the project root.** AI agents generate intermediate files during sessions. These belong in `docs/` with proper naming, or are deleted after use.

| Type | Location | Example |
|------|----------|---------|
| RFC / design proposal | `docs/rfc-NNN-<topic>.md` | `rfc-014-agent-sandbox.md` |
| Architecture decision | `docs/ARCHITECTURE.md` | Append § section. No standalone files. |
| Design doc (UI, flow) | `docs/designs/` | `YYYY-MM-DD-<topic>-design.md` |
| Implementation result | `docs/archive/` | `<topic>-result.md` |
| Audit / review | `docs/production-audit/` | `YYYY-MM-DD-<topic>.md` |
| Temporary analysis | **Delete after use.** | Never create `*-analysis.md`, `fix-*.md`, `*-output.md`, `PROGRESS.md` in root. |

**Allowed root files:** `AGENTS.md`, `README.md`, `CHANGELOG.md`, `DESIGN.md`, `CONTRIBUTING.md`, `LICENSE`, `THIRD-PARTY-NOTICES.md`. Nothing else.

## File Locations

| Path | Purpose |
|------|---------|
| `~/.oxios/` | Oxios home |
| `~/.oxios/config.toml` | Configuration |
| `~/.oxios/workspace/` | Agent working directory (seeds, sessions, skills) |
| `~/.oxios/knowledge/` | User markdown knowledge base |
| `~/.oxi/auth.json` | oxi-cli credentials (separate from Oxios) |

## Architecture (summary)

See `docs/ARCHITECTURE.md` for the full reference (subsystems, data flow, dependency graph).

- **Kernel** (`oxios-kernel`) — intentionally monolithic single crate. Star topology around `AgentId`, `EventBus`, `StateStore`. No circular deps. Internal boundaries via `pub(crate)` + directory mod files. See ARCHITECTURE.md §10 for rationale.
- **KernelHandle** — Facade with 13 typed APIs (Agent, Space, Security, Persona, Exec, Browser, MCP, Extension, Infra, A2A, State, KnowledgeBase, KnowledgeLens).
- **Supervisor** — Agent lifecycle: fork/exec/wait/kill.
- **Orchestrator** — Ouroboros protocol end-to-end. The "brain".
- **AgentRuntime** — Wraps oxi-sdk tool-calling loop.
- **Memory** — Tiered (Hot/Warm/Cold), Dream consolidation, HNSW, hyperbolic embeddings.
- **AccessManager** — OWASP RBAC + path sandboxing + Merkle audit trail.
- **Skill** — Unified system (RFC-009). Each skill = `SKILL.md` with YAML frontmatter.

## Adding a New Tool

1. Define in `crates/oxios-kernel/src/tools/<name>_tool.rs` — implement `AgentTool` from `oxi_sdk`
2. Register in `tools/kernel_bridge.rs::register_all_kernel_tools()`
3. If it wraps a KernelHandle API, add `*_api.rs` in `kernel_handle/`
4. Test: `oxios run --json "<command that triggers tool>"`

## Key Docs

| File | Read when |
|------|-----------|
| `docs/ARCHITECTURE.md` | Modifying kernel structure, adding modules, understanding subsystems |
| `docs/DESIGN.md` | Understanding design philosophy and Unix↔Oxios mapping |
| `docs/rfc-008-memory-consolidation.md` | Modifying memory system |
| `docs/rfc-009-skill-unification.md` | Modifying skill system |
| `docs/rfc-010-clawhub-marketplace.md` | Marketplace feature |
| `docs/design-knowledge-ui.md` | Knowledge UI (frontend components, shortcuts, architecture) |
| `docs/channel-plugin-guide.md` | Adding a new channel |
| `docs/USER-GUIDE.md` | Changing user-facing features or CLI behavior |
| `share/default-config.toml` | Changing configuration options |

## Pitfalls

- **Kernel is intentionally monolithic.** See ARCHITECTURE.md §10. Do not propose splitting.
- **oxi-sdk is crates.io only.** Never add as path dep. Never reimplement what it provides.
- **Kernel binary vs library.** `src/kernel.rs` (assembler) is in the binary crate, not `oxios-kernel`.
- **Agent lifecycle split.** `Supervisor` = low-level process. `AgentLifecycleManager` = full lifecycle (A2A, scheduling, permissions). Don't add lifecycle logic to Orchestrator.
- **Tool registration.** All kernel tools → `tools/kernel_bridge.rs::register_all_kernel_tools()`. Not `registration.rs`.
- **Two knowledge systems.** Agent memory = MemoryManager (JSON per Space). User notes = KnowledgeBase (`.md` files, `~/.oxios/knowledge/`). See `docs/rfc-003-knowledge-separation.md`.
- **Unified skill model.** No separate `program/` module or `program.toml`. `SkillManager` handles everything. Each skill = `SKILL.md` + YAML frontmatter.
- **Feature gates.** Web, CLI, Telegram, browser, telemetry are feature-gated. Check `cargo build -p oxios --features <feature>`.
- **Workspace deps.** `oxi-sdk` must be in both `[workspace.dependencies]` (root `Cargo.toml`) AND `[dependencies]` in the crate using it.
- **Stdin blocking.** `oxios run --context-file -` reads stdin to EOF. Don't use with interactive input.
- **CI oxi checkout.** CI checks out `a7garden/oxi` alongside oxios. Check ref if oxi-related tests fail.
