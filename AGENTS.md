# Oxios AGENTS.md

> Onboarding document for AI agents working on this codebase.
> Hand-written. Every sentence is intentional. Do not auto-regenerate.

## What

Oxios is an **Agent Operating System** in Rust. AI agents fork, exec, wait, kill — just like Unix processes.

**Stack:** Rust 2024 (edition 2024, MSRV 1.96), tokio async, serde, oxi-sdk (crates.io).

```
User → Channel (Web/CLI/Telegram) → Gateway → Kernel
```

```
oxios/
├── crates/
│   ├── oxios-kernel/      # Supervisor, scheduler, memory, security, tools
│   ├── oxios-markdown/    # Knowledge base (VirtualFs, BacklinkIndex)
│   ├── oxios-ouroboros/   # Unified intent handling (assess → crystallize → execute → review)
│   ├── oxios-gateway/     # Channel-agnostic message hub
│   ├── oxios-mcp/         # MCP client (JSON-RPC 2.0 over stdio)
│   ├── oxios-memory/      # Tiered agent memory (Hot/Warm/Cold, Dream, HNSW)
│   └── oxios-calendar/    # .ics-based calendar event management
├── src/                   # Binary: HTTP API server, CLI/Telegram channels, main()
│   ├── api/               # REST/WebSocket/SSE (was surface/oxios-web)
│   └── channels/          # In-process channels (was channels/oxios-{cli,telegram})
├── web/                   # React frontend (was surface/oxios-web/web)
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
| **Build** | `cargo build && cd web && bun run build` |
| **Test** | `cargo test --workspace` |

## Principles

- **Unix philosophy** — fork/exec/wait/kill for agents. Compose small pieces.
- **Intent-first** — assess every message; depth adapts to the task. assess → crystallize → execute → review.
- **No reimplementation** — reuse oxi-sdk from crates.io.
- **Channel agnostic** — gateway doesn't care where messages come from.
- **No containers** — direct host execution. Security via AccessManager (RBAC + path sandboxing).

## Conventions

- **Language:** Code, comments, docs, commits — English. **Structural/tool output is English** (global product): CLI `--help`, status panels, banners, error messages, permission-denial reasons, and Telegram bot guidance/commands. **Agent conversational replies follow the user's language** (Korean for Korean users). **Web UI is bilingual** (Korean/English). The string *sources* in `oxios-gateway`/`oxios-kernel` (e.g. `error_classify.rs`, `gate.rs`) count as tool output → English.
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
| `~/.oxios/workspace/` | Agent working directory (sessions, skills) |
| `~/.oxios/knowledge/` | User markdown knowledge base |
| `~/.oxi/auth.json` | oxi-cli credentials (separate from Oxios) |

## Architecture (summary)

See `docs/ARCHITECTURE.md` for the full reference (subsystems, data flow, dependency graph).

- **Kernel** (`oxios-kernel`) — intentionally monolithic single crate. Star topology around `AgentId`, `EventBus`, `StateStore`. No circular deps. Internal boundaries via `pub(crate)` + directory mod files. See ARCHITECTURE.md §10 for rationale.
- **KernelHandle** — Facade with 13 typed APIs (Agent, Space, Security, Persona, Exec, Browser, MCP, Extension, Infra, A2A, State, KnowledgeBase, KnowledgeLens).
- **Supervisor** — Agent lifecycle: fork/exec/wait/kill.
- **Orchestrator** — Ouroboros protocol end-to-end. The "brain".
- **AgentRuntime** — Wraps oxi-sdk tool-calling loop.
- **OxiosEngine** — Wraps oxi-sdk's `Oxi`. Provider/model resolution goes through `OxiBuilder`. The **catalog port** (`oxi-sdk` `ModelCatalog`) is initialized once at boot (`OxiosEngine::init_file_catalog`, self-hosted under `~/.oxios/cache/`) and attached to every engine — including across hot-swaps — so `resolve_model` and Web UI introspection (`EngineApi`) consult dynamic models.dev metadata (live price/limit refresh, user overrides) before falling back to the static registry. Static free-fns (`get_provider_models`, etc.) remain as fallbacks (the static `model_db` is itself backed by the embedded models.dev snapshot, so it carries real prices).
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
| `docs/rfc-024-web-daemon-reliability.md` | Modifying web↔daemon delivery, SSE/WS, static asset serving, readiness |
| `docs/design-knowledge-ui.md` | Knowledge UI (frontend components, shortcuts, architecture) |
| `docs/channel-plugin-guide.md` | Adding a new channel |
| `docs/USER-GUIDE.md` | Changing user-facing features or CLI behavior |
| `share/default-config.toml` | Changing configuration options |

## Release

Two separate pipelines, two different triggers.

### Web UI — GitHub Actions (automatic)

Tag push (`v*`) triggers `.github/workflows/release.yml`. Builds `web/` with Bun, zips `dist/`, uploads as GitHub Release asset. No local action needed.

```bash
git tag v1.2.0 && git push --tags   # → CI builds & publishes web-dist.zip
```

### crates.io — CI (automatic)

GitHub Actions `publish.yml` publishes all 8 crates in topological order.
`publish.yml` is dispatched by `release.yml` after a GitHub Release is created
(a Release made with GITHUB_TOKEN doesn't emit `release: published`, so
`release.yml` triggers it via `gh workflow run`).

Topological order:

① oxios-markdown, oxios-mcp, oxios-ouroboros, oxios-memory   (no oxios deps)
② oxios-calendar    → oxios-markdown
③ oxios-kernel      → {ouroboros, markdown, calendar, mcp, memory}
④ oxios-gateway     → oxios-kernel
⑤ oxios (binary)    → {kernel, gateway, markdown, ouroboros, calendar}
```

`oxios-web`/`oxios-cli`/`oxios-telegram` were merged into the binary as
in-process modules per RFC-026 — no separate crates to publish.
- **Kernel is intentionally monolithic.** See ARCHITECTURE.md §10. Do not propose splitting.
- **oxi-sdk is crates.io only.** Never add as path dep. Never reimplement what it provides.
- **Kernel binary vs library.** `src/kernel.rs` (assembler) is in the binary crate, not `oxios-kernel`.
- **Agent lifecycle split.** `Supervisor` = low-level process. `AgentLifecycleManager` = full lifecycle (A2A, scheduling, permissions). Don't add lifecycle logic to Orchestrator.
- **Tool registration.** All kernel tools → `tools/kernel_bridge.rs::register_all_kernel_tools()`. Not `registration.rs`.
- **Two knowledge systems.** Agent memory = MemoryManager (JSON per Space). User notes = KnowledgeBase (`.md` files, `~/.oxios/knowledge/`). See `docs/rfc-003-knowledge-separation.md`.
- **Unified skill model.** No separate `program/` module or `program.toml`. `SkillManager` handles everything. Each skill = `SKILL.md` + YAML frontmatter.
- **Feature gates.** Web, CLI, Telegram, browser, telemetry are feature-gated. Check `cargo build -p oxios --features <feature>`.
- **`--all-features` works.** oxi-sdk 0.45.1 + wasmtime 24 migration — `cargo build/clippy --workspace --all-features` compiles. As of oxi-sdk 0.45.x, `AgentConfig` gained `ttsr_engine`/`memory`/`todo`/`agent_pool` fields (all `#[serde(skip, default)]`); fill with `..Default::default()` to keep sites working. CI still uses per-crate features (`.github/workflows/ci.yml`) for precision. The prior wasm-sandbox `ResourceLimiter` regression (missing `table_growing` on wasmtime 24) is fixed in `crates/oxios-kernel/src/wasm_sandbox.rs`.
- **Workspace deps.** `oxi-sdk` must be in both `[workspace.dependencies]` (root `Cargo.toml`) AND `[dependencies]` in the crate using it.
- **Stdin blocking.** `oxios run --context-file -` reads stdin to EOF. Don't use with interactive input.
- **CI oxi checkout.** CI checks out `a7garden/oxi` alongside oxios. Check ref if oxi-related tests fail.
