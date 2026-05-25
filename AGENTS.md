# Oxios AGENTS.md

> Onboarding document for AI agents working on this codebase.
> Hand-written. Every sentence is intentional. Do not auto-regenerate.

## What

Oxios is an **Agent Operating System** in Rust. It's an OS where AI agents execute real work on behalf of users — fork, exec, wait, kill, just like Unix processes.

**Stack:** Rust 2021, tokio async, serde (JSON+TOML), oxi-sdk + oxi-ai (crates.io). ~66.7K lines across ~300 source files (196 Rust + 106 TypeScript/TSX).

```
User → Channel (Web/CLI/Telegram) → Gateway → Kernel (supervisor + scheduler + ouroboros + agent_runtime)
```

```
oxios/                     # Main binary (src/main.rs, src/kernel.rs, src/cmd_run.rs)
├── crates/
│   ├── oxios-kernel/      # Core: supervisor, scheduler, event bus, state store, tools, memory
│   ├── oxios-markdown/    # Markdown knowledge base: VirtualFs, BacklinkIndex, link graph
│   ├── oxios-ouroboros/   # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   ├── oxios-gateway/     # Channel-agnostic message hub
│   └── oxios-mcp/         # MCP client library (JSON-RPC 2.0 over stdio)
├── benchmarks/
│   └── oxios-bench/       # Performance benchmarking suite
├── channels/
│   ├── oxios-web/         # Web dashboard (Axum backend + React frontend)
│   ├── oxios-cli/         # CLI channel
│   └── oxios-telegram/    # Telegram channel
├── share/                 # Default skills (share/default-skills/), config
└── docs/                  # Architecture docs, RFCs, design documents
```

**Dependency graph:**
```
oxios → oxios-kernel → oxios-ouroboros
      → oxios-markdown (knowledge base)
      → oxios-mcp (MCP client)
      → oxi-sdk (crates.io, NOT path dep)
      → oxi-ai (provider construction)
    → oxios-gateway
    → oxios-web/oxios-cli/oxios-telegram (channel plugins, feature-gated)
```


## Quick Facts

| Fact | Value |
|------|-------|
| **Language** | Rust 2021 + TypeScript 5 (frontend) |
| **License** | MIT |
| **CI** | GitHub Actions (macOS + Linux, fmt+clippy+test+audit+frontend) |
| **Build** | `cargo build && cd channels/oxios-web/web && bun run build` |
| **Test** | `cargo test --workspace` |

## Why

| Principle | Meaning |
|-----------|---------|
| **Unix philosophy** | Every component does one thing. Compose small pieces. |
| **Ouroboros first** | Never execute without a spec. Interview → seed → execute → evaluate → evolve. |
| **No reimplementation** | Reuse oxi-sdk. Never reimplement what oxi already provides. |
| **Channel agnostic** | Gateway doesn't care where messages come from. |
| **User invisible** | Users don't know how many agents are running. They talk, the OS handles the rest. |
| **No containers** | Direct host execution. Security via AccessManager (RBAC + path sandboxing). |

## How

```bash
# Frontend build (required for CI/web channel)
cd channels/oxios-web/web && bun install && bun run build

# Build everything (Rust only)
cargo build

# Run all tests (must pass at every commit)
cargo test --workspace

# Run oxios daemon (background by default)
cargo run

# Run in foreground (for debugging)
cargo run -- --foreground

# Single-shot execution with JSON output
cargo run -- run --json "prompt"

# Frontend dev server (requires backend on port 3000)
cd channels/oxios-web/web && bun dev
```

## Daemon & CLI

`oxios` starts as a **daemon** by default (launchd on macOS, systemd on Linux). First run triggers an interactive setup wizard if credentials are missing.

The `oxios run` subcommand is designed for **programmatic consumption** — agents can call it via `exec` tool:

```bash
# JSON output — parse response, session_id, evaluation_passed, exit_code
oxios run --json "review this code"

# Pass file as context (use `-` for stdin)
cat file.rs | oxios run --json --context-file - "describe this"

# Exit code for script integration: 0=passed, 1=failed
oxios run --exit-code --json "run tests"
echo $?  # 0 or 1

# Multi-turn session (pass session_id from response)
SID=$(oxios run --json "initial prompt" | jq -r '.session_id')
oxios run --json --session "$SID" "follow-up"
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


## File Locations

| Path | Purpose |
|------|---------|
| `~/.oxios/` | Oxios home directory |
| `~/.oxios/config.toml` | Main configuration |
| `~/.oxios/workspace/` | Agent working directory |
| `~/.oxios/workspace/sessions/` | Session data (ephemeral) |
| `~/.oxios/workspace/seeds/` | Ouroboros seed specs |
| `~/.oxios/workspace/skills/` | Unified skill definitions (replaces Programs + Skills) |
| `~/.oxi/auth.json` | oxi-cli credentials (separate from Oxios) |

## Conventions

- **Language:** Code, comments, docs, commits — English. User-facing messages — Korean.
- **Rust:** `#![warn(missing_docs)]` on public crates. `anyhow` for apps, `thiserror` for libs.
- **Naming:** Crates `oxios-<component>`, public API `verb_noun` (`fork_agent`, `create_seed`).
- **Testing:** Unit tests in `#[cfg(test)] mod tests`. Integration tests in `tests/` per crate.
- **Commits:** `<type>(<scope>): <description>` — scopes: kernel, ouroboros, gateway, web, cli, docs.

## Key Architecture Points

- **Supervisor** (`supervisor.rs`) — Agent lifecycle: fork/exec/wait/kill. The "init" of Oxios.
- **AgentLifecycleManager** (`agent_lifecycle.rs`) — Extracted from Orchestrator: fork → register A2A → check permissions → schedule → run → cleanup.
- **Scheduler** (`scheduler.rs`) — Priority-based task queue (AIOS/AgentRM-inspired). Rate-limit-aware admission, zombie detection, max concurrent enforcement.
- **Orchestrator** (`orchestrator.rs`) — Runs the Ouroboros protocol end-to-end. The "brain".
- **AgentRuntime** (`agent_runtime.rs`) — Wraps oxi-agent's tool-calling loop.
- **OxiosEngine** (`engine.rs`) — Thin wrapper around `oxi_sdk::Oxi`. Provider/model resolution via `OxiBuilder`. Uses `oxi_ai` for provider construction.
- **KernelBridge** (`tools/kernel_bridge.rs`) — Registers all kernel domain tools into an agent's `ToolRegistry` during agent build.
- **ExecTool** (`tools/exec_tool.rs`) — Two modes: `shell` (bash -c, RBAC-enforced) and `structured` (binary allowlist + metacharacter blocking).
- **Kernel tools** (`tools/kernel/`) — Space, Agent, Persona, Cron, Security, Budget, Resource, Knowledge tools. Each wraps a KernelHandle API domain.
- **KernelHandle** (`kernel_handle/`) — Facade exposing 13 typed APIs: AgentApi, SpaceApi, SecurityApi, PersonaApi, ExecApi, BrowserApi, McpApi, ExtensionApi, InfraApi, A2aApi, StateApi, KnowledgeBase, KnowledgeLens. Internally uses `CredentialStore` (`credential.rs`) for multi-source key resolution. Each API lives in its own file (`*_api.rs`).
- **AccessManager** (`access_manager/`) — OWASP-inspired least-privilege. RBAC, path sandboxing, audit logging.
- **AuditTrail** (`audit_trail.rs`) — Merkle-chain style tamper-evident audit log. Each entry cryptographically linked.
- **Memory** (`memory/`) — Tiered memory system (Hot/Warm/Cold) with Dream-time consolidation (RFC-008). Includes: auto-classification, auto-protection, Ebbinghaus-inspired decay, compaction tree (Raw→Daily→Weekly→Monthly→Root), ROOT index (O(1) topic lookup), proactive recall, HNSW vector search, hyperbolic embeddings, flash attention, reasoning bank, Sona learning engine, RVF store.
- **MCP** (`mcp/`) — Model Context Protocol client. Web uses `oxios-mcp` (crates.io); kernel uses `crates/oxios-mcp` via `mcp/mod.rs` integration layer.
- **Auth** (`auth.rs`) — Authentication manager. Used by KernelHandle for identity verification.
- **Workers** (`workers/`) — Background worker pool for async task processing.
- **WasmSandbox** (`wasm_sandbox.rs`) — WASM-based sandbox for executing untrusted code.
- **Onboarding** (`onboarding.rs`) — Interactive setup wizard triggered on first run.
- **Space** (`space/`) — Directory: `manager.rs` (CRUD), `conversation_buffer.rs`, `space_bridge.rs` (cross-Space memory transfer), `detection.rs` (intent classification). Note: `knowledge_bridge.rs` was renamed to `space_bridge.rs` (RFC-003).
- **Telemetry** (`telemetry_otel.rs` / `telemetry_stub.rs`) — OpenTelemetry integration with compile-time feature toggle to stub.
- **ResourceMonitor** (`resource_monitor.rs`) — System resource tracking for agent budget enforcement.
- **Kernel** (`src/kernel.rs`) — `Kernel::builder().build().await` assembles all components. `execute_prompt_with_session()` for CLI execution.
- **Skill** (`skill.rs`) — Unified skill system (RFC-009). `SkillManager` replaces former `SkillStore` + `ProgramManager` + `HostToolValidator`. Each skill is a `SKILL.md` with YAML frontmatter carrying all metadata (4-dimensional requirements, install specs, invocation policy). No separate `program.toml` files. Skill source hierarchy: agent-specific > workspace > global user > bundled.
- **Capability** (`capability/`) — Template-based capability resolution for agent tool discovery.
- **A2A** (`a2a.rs`) — Google's agent-to-agent protocol. Horizontal agent communication.
- **CircuitBreaker** (`circuit_breaker.rs`) — 3-state (Closed→Open→Half-Open) protection against cascading LLM provider failures.
- **CredentialStore** (`credential.rs`) — Multi-source credential resolution: env var → config.toml → oxi auth.json.
- **Daemon** (`daemon.rs`) — PID file management, start/stop, system service install (launchd/systemd).
- **GitLayer** (`git_layer.rs`) — In-process version control via `gix`. Commits, logs, tags, restore.
- **CronScheduler** (`cron.rs`) — Scheduled job execution with persistent state.
- **BudgetManager** (`budget.rs`) — Token/cost budget enforcement per agent.
- **AgentGroup** (`agent_group.rs`) — Oxios-level group management for Seed-split multi-agent execution.

## Adding a New Tool

1. **Define** the tool in `crates/oxios-kernel/src/tools/<name>_tool.rs` (or `tools/kernel/` for kernel facade tools)
   - Implement `AgentTool` from `oxi_sdk`
   - Return `AgentToolResult::success()` or `AgentToolResult::error()`
2. **Register** in `tools/kernel_bridge.rs` via `register_all_kernel_tools()` — this is the canonical registration point
3. **Test** with `oxios run --json "<command that triggers tool>"`
4. **Audit** the execution path in `access_manager/` if sensitive
5. If the tool wraps a KernelHandle API, add a corresponding `*_api.rs` in `kernel_handle/`

**Tool signature pattern:**
```rust
#[async_trait]
impl AgentTool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &'static str { "..." }
    fn parameters_schema(&self) -> Value { json!({...}) }
    async fn execute(&self, tool_call_id: &str, params: Value, ...) -> Result<AgentToolResult, String>
}
```

## Detailed Docs (read when relevant)

| File | When to read |
|------|-------------|
| `docs/ARCHITECTURE.md` | Before modifying kernel structure or adding modules |
| `docs/rfc-008-memory-consolidation.md` | Before modifying memory system (tiered memory, Dream, decay, compaction) |
| `docs/rfc-009-skill-unification.md` | Before modifying skill system (unified SKILL.md frontmatter, requirements) |
| `docs/rfc-010-clawhub-marketplace.md` | Before working on marketplace feature |
| `docs/channel-plugin-guide.md` | Before adding a new channel (Web, Telegram, etc.) |
| `docs/channel-registry.md` | Before registering a new channel |
| `docs/rfc-003-knowledge-separation.md` | Before modifying knowledge/memory architecture |
| `docs/rfc-004-knowledge-system.md` | Before modifying knowledge system |
| `docs/rfc-005-knowledge-integration.md` | Before integrating knowledge with AI engine |
| `docs/rfc-006-js-space-integration.md` | Before modifying JS/Space integration |
| `docs/rfc-007-remaining-port.md` | Before porting remaining features |
| `docs/refactoring-design.md` | Before large-scale refactoring |
| `docs/programs-and-skills.md` | Reference for skill frontmatter format (note: Programs terminology deprecated, now unified as Skills) |
| `docs/USER-GUIDE.md` | Before changing user-facing features or CLI behavior |
| `share/default-config.toml` | Before changing configuration options |

## Knowledge UI

The Knowledge UI is a full-screen app-within-app (`fixed inset-0 z-30`) built into the oxios-web dashboard. It provides a files.md-style markdown note-taking experience backed by `oxios-markdown`.

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  channels/oxios-web/web/src/                                 │
│                                                              │
│  routes/knowledge/index.tsx     → /knowledge/                  │
│    └── components/knowledge/                                  │
│        ├── knowledge-layout.tsx  ← Zustand store + shortcuts  │
│        ├── knowledge-sidebar.tsx  ← File tree + chat/journal   │
│        ├── file-tree.tsx         ← Recursive tree             │
│        ├── editor-panel.tsx      ← Editor + toolbar           │
│        ├── markdown-editor.tsx   ← HyperMD (CM5) editor       │
│        ├── split-editor.tsx      ← Second pane               │
│        ├── editor-toolbar.tsx     ← Back/forward/split/close  │
│        ├── knowledge-chat.tsx     ← Quick notes (Chat.md)      │
│        ├── search-modal.tsx       ← ⌘K global search           │
│        ├── move-modal.tsx         ← ⌘M file mover              │
│        ├── info-panel.tsx         ← Backlinks/Copilot/Graph   │
│        ├── copilot.tsx            ← AI copilot panel           │
│        ├── link-graph.tsx         ← SVG graph viz             │
│        ├── habits.tsx             ← Year grid tracker         │
│        ├── today-stats.tsx        ← Daily completion card     │
│        └── knowledge-settings.tsx  ← Config editor             │
│                                                              │
│  hooks/use-knowledge.ts       ← 29 TanStack Query API hooks   │
│  hooks/use-knowledge-shortcuts.ts  ← Global ⌘N/D/Enter/⌘W   │
│  stores/knowledge.ts         ← Zustand (mode, path, history)  │
│  lib/hypermd-setup.ts        ← CM5 + HyperMD side-effect init │
│  lib/autocomplete-link.ts    ← `[` link hint function          │
│  types/knowledge.ts          ← Full TypeScript type surface   │
│  types/hypermd.d.ts          ← CM5/HyperMD type declarations  │
└──────────────────────────────────────────────────────────────┘

```

### Backend

- **Route registration**: `channels/oxios-web/src/routes/knowledge_routes.rs` — all Axum handlers
- **KnowledgeBase** (direct): `crates/oxios-markdown/src/knowledge.rs` — web uses `state.knowledge` (KnowledgeBase) directly, bypassing the kernel. No AI engine dependency at the web layer.
- **KnowledgeBase** (kernel-free, via oxios-markdown): markdown note management, backlinks, AI copilot. Access via `kernel.knowledge` (Arc). See RFC-003 for architecture.
- **KnowledgeLens** (`kernel_handle/knowledge_lens.rs`) — Semantic HNSW overlay. Subscribes to KnowledgeBase `on_file_change` callbacks to keep agent memory index in sync.

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Fixed overlay** (`z-30`) | Escapes AppLayout sidebar/header/padding for full-screen experience |
| **State-based SPA** | File navigation via Zustand, not URL routes (graph/habits/settings get their own routes) |
| **HyperMD (CM5)** | Matches files.md; heavier than CM6 but simpler migration path |
| **Bundle splitting** | TanStack Router autoCodeSplitting puts HyperMD (447KB) in its own chunk, loaded only on `/knowledge/` |
| **No iframes** | All components written in React; no static HTML embedding |
| **Web bypasses kernel for knowledge** | `state.knowledge` (KnowledgeBase) is created directly in plugin.rs. Web channel doesn't need AI engine for basic CRUD.


### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `⌘K` / `⌘P` | Open search modal |
| `⌘M` | Open move file modal |
| `⌘N` | New file |
| `⌘⇧N` | New folder |
| `⌘D` | Delete current file |
| `⌘S` | Manual save |
| `⌘W` | Close split editor |
| `⌘Enter` | Open chat |
| `⌘B` / `⌘I` | Bold / Italic |
| `⌘Y` | Insert checkmark |
| `⌘~` | Toggle sidebar |
| `Escape` | Close split |
| `jj` (in chat) | Route to journal |
| `Enter` (in chat) | Send message |

### Adding a New Component

1. Create in `components/knowledge/`
2. Import store actions from `@/stores/knowledge`
3. Use API hooks from `@/hooks/use-knowledge`
4. For a new API route: add it to `knowledge_routes.rs`, then add a hook in `use-knowledge.ts`, then add types in `types/knowledge.ts`
5. If the component needs a page route (like `/knowledge/habits`), add a file in `routes/knowledge/`


### Testing

```bash
# Backend must be running (port 3000)
cargo run --bin oxios -- --foreground

# Frontend dev server
cd channels/oxios-web/web && bun dev

# Open http://localhost:5173/knowledge/
# Or directly http://localhost:3000/knowledge/ (proxied)
```

## Pitfalls

- **Workspace deps**: If `cargo build` fails with missing `oxi-sdk`, ensure it's in `[workspace.dependencies]` in root `Cargo.toml` AND `[dependencies]` in the crate using it. The project depends on `oxi-sdk` (not separate `oxi-ai`/`oxi-agent`).
- **Stdin blocking**: `oxios run --context-file -` reads stdin to EOF. Don't use with interactive input — it blocks.
- **Session IDs**: Sessions live in orchestrator memory. Process restart loses them. Use `--session` only within a single CLI session chain.
- **Kernel binary vs library**: `src/kernel.rs` (the assembler/builder) lives in the **binary crate**, not `oxios-kernel`. The library provides components; the binary wires them together.
- **Agent lifecycle split**: `Supervisor` handles low-level process management. `AgentLifecycleManager` handles the full orchestrated lifecycle (A2A registration, scheduling, permissions). Don't add lifecycle logic to Orchestrator directly — use `AgentLifecycleManager`.
- **CI oxi checkout**: CI checks out `a7garden/oxi` at `v0.4.4` alongside oxios. If tests fail with oxi-related errors, check that the oxi ref matches.
- **Feature gates**: Web, CLI, Telegram are feature-gated. Browser is enabled by default (default feature). If a channel doesn't compile, check `cargo build -p oxios --features <feature>`.
- **Tool registration**: All kernel tools must be registered in `tools/kernel_bridge.rs::register_all_kernel_tools()`. Don't add tools directly to `registration.rs` for kernel operations.
- **Two knowledge storage systems**: Agent session memory is in MemoryManager (JSON per Space). User's markdown knowledge is in KnowledgeBase (`.md` files, global `~/.oxios/knowledge/`). Don't confuse the two — see `docs/rfc-003-knowledge-separation.md`.
- **Unified skill model (RFC-009)**: Programs and skills were merged into a single Skill concept. There is no separate `program/` module or `program.toml`. `SkillManager` in `skill.rs` handles everything. Each skill is a SKILL.md with YAML frontmatter.
- **Memory tiers (RFC-008)**: Memories are automatically tiered (Hot → Warm → Cold) based on type and access patterns. The Dream process runs in the background for consolidation. Protection levels are auto-calculated from access frequency and session appearances — users never need to manually manage memory.