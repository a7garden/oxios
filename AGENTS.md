# Oxios AGENTS.md

> Onboarding document for AI agents working on this codebase.
> Hand-written. Every sentence is intentional. Do not auto-regenerate.

## What

Oxios is an **Agent Operating System** in Rust. It's an OS where AI agents execute real work on behalf of users — fork, exec, wait, kill, just like Unix processes.

**Stack:** Rust 2021, tokio async, serde (JSON+TOML), oxi-sdk + oxi-ai (crates.io). ~56K lines across 184 source files.

```
User → Channel (Web/CLI/Telegram) → Gateway → Kernel (supervisor + scheduler + ouroboros + agent_runtime)
```

```
oxios/                     # Main binary (src/main.rs, src/kernel.rs, src/cmd_run.rs)
├── crates/
│   ├── oxios-kernel/      # Core: supervisor, scheduler, event bus, state store, tools, memory
│   ├── oxios-ouroboros/   # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   └── oxios-gateway/     # Channel-agnostic message hub
├── channels/
│   ├── oxios-web/         # Web dashboard (Axum backend + React frontend)
│   ├── oxios-cli/         # CLI channel
│   └── oxios-telegram/    # Telegram channel
├── .programs/             # OS-level programs (code-review, debug, deploy, guardian, refactor, program-creator)
├── share/                 # Default skills, programs, config
└── docs/                  # Architecture docs, RFCs, design docs
```

**Dependency graph:**
```
oxios → oxios-kernel → oxi-sdk (crates.io, NOT path dep)
                    → oxi-ai (provider construction)
                    → oxios-ouroboros
      → oxios-gateway
      → oxios-web/oxios-cli/oxios-telegram (channel plugins, feature-gated)
```


## Quick Facts

| Fact | Value |
|------|-------|
| **Language** | Rust 2021 |
| **Version** | 0.1.2 |
| **License** | MIT |
| **CI** | GitHub Actions (macOS-latest, fmt+clippy+test+audit) |
| **Build** | `cargo build` |
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
cargo build                # Build everything
cargo test --workspace     # Run all tests (must pass at every commit)
cargo run                  # Run oxios daemon (background by default)
cargo run -- --foreground  # Run in foreground (for debugging)
cargo run -- run --json "prompt"   # Single-shot execution with JSON output
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
| `~/.oxios/workspace/programs/` | Installed programs |
| `~/.oxios/workspace/skills/` | Skill definitions |
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
- **Kernel tools** (`tools/kernel/`) — Space, Agent, Persona, Cron, Security, Budget, Resource tools. Each wraps a KernelHandle API domain.
- **KernelHandle** (`kernel_handle/`) — Facade exposing 12 typed APIs: AgentApi, SpaceApi, SecurityApi, PersonaApi, ExecApi, BrowserApi, McpApi, ExtensionApi, InfraApi, A2aApi, StateApi (+ CredentialApi via `credential.rs`).
- **AccessManager** (`access_manager/`) — OWASP-inspired least-privilege. RBAC, path sandboxing, audit logging.
- **AuditTrail** (`audit_trail.rs`) — Merkle-chain style tamper-evident audit log. Each entry cryptographically linked.
- **Memory** (`memory/`) — Vector store with hyperbolic embeddings, HNSW indexing, flash attention, reasoning bank, Sona learning engine, RVF store.
- **MCP** (`mcp/`) — Model Context Protocol client, protocol handler, and server integration. Wrapped by `McpApi` in kernel_handle.
- **Auth** (`auth.rs`) — Authentication manager. Used by KernelHandle for identity verification.
- **Workers** (`workers/`) — Background worker pool for async task processing.
- **WasmSandbox** (`wasm_sandbox.rs`) — WASM-based sandbox for executing untrusted code.
- **Onboarding** (`onboarding.rs`) — Interactive setup wizard triggered on first run.
- **Space** (`space/`) — Directory: `manager.rs` (CRUD), `conversation_buffer.rs`, `knowledge_bridge.rs` (auto-knowledge extraction), `detection.rs` (intent classification).
- **Telemetry** (`telemetry_otel.rs` / `telemetry_stub.rs`) — OpenTelemetry integration with compile-time feature toggle to stub.
- **ResourceMonitor** (`resource_monitor.rs`) — System resource tracking for agent budget enforcement.
- **Kernel** (`src/kernel.rs`) — `Kernel::builder().build().await` assembles all components. `execute_prompt_with_session()` for CLI execution.
- **Program** (`program/`) — OS-level installable capabilities. See `.programs/` for examples.
- **Capability** (`capability/`) — Template-based capability resolution for agent tool discovery.
- **A2A** (`a2a.rs`) — Google's agent-to-agent protocol. Horizontal agent communication.
- **CircuitBreaker** (`circuit_breaker.rs`) — 3-state (Closed→Open→Half-Open) protection against cascading LLM provider failures.
- **CredentialStore** (`credential.rs`) — Multi-source credential resolution: config.toml → oxi auth.json → env var.
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
| `docs/channel-plugin-guide.md` | Before adding a new channel (Web, Telegram, etc.) |
| `docs/channel-registry.md` | Before registering a new channel |
| `docs/rfc-001-kernel-facade.md` | Before modifying KernelHandle or tool APIs |
| `docs/rfc-002-kernel-module-organization.md` | Before reorganizing kernel modules |
| `docs/refactoring-design.md` | Before large-scale refactoring |
| `docs/program-development.md` | Before creating or modifying programs |
| `docs/USER-GUIDE.md` | Before changing user-facing features or CLI behavior |
| `share/default-config.toml` | Before changing configuration options |

## Pitfalls

- **Workspace deps**: If `cargo build` fails with missing `oxi-sdk`, ensure it's in `[workspace.dependencies]` in root `Cargo.toml` AND `[dependencies]` in the crate using it. The project depends on `oxi-sdk` (not separate `oxi-ai`/`oxi-agent`).
- **Stdin blocking**: `oxios run --context-file -` reads stdin to EOF. Don't use with interactive input — it blocks.
- **Session IDs**: Sessions live in orchestrator memory. Process restart loses them. Use `--session` only within a single CLI session chain.
- **Kernel binary vs library**: `src/kernel.rs` (the assembler/builder) lives in the **binary crate**, not `oxios-kernel`. The library provides components; the binary wires them together.
- **Agent lifecycle split**: `Supervisor` handles low-level process management. `AgentLifecycleManager` handles the full orchestrated lifecycle (A2A registration, scheduling, permissions). Don't add lifecycle logic to Orchestrator directly — use `AgentLifecycleManager`.
- **CI oxi checkout**: CI checks out `a7garden/oxi` at `v0.4.4` alongside oxios. If tests fail with oxi-related errors, check that the oxi ref matches.
- **Feature gates**: Web, CLI, Telegram are feature-gated. Browser is enabled by default (default feature). If a channel doesn't compile, check `cargo build -p oxios --features <feature>`.
- **Tool registration**: All kernel tools must be registered in `tools/kernel_bridge.rs::register_all_kernel_tools()`. Don't add tools directly to `registration.rs` for kernel operations.