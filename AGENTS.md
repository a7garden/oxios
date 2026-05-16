# Oxios AGENTS.md

> Onboarding document for AI agents working on this codebase.
> Hand-written. Every sentence is intentional. Do not auto-regenerate.

## What

Oxios is an **Agent Operating System** in Rust. It's an OS where AI agents execute real work on behalf of users — fork, exec, wait, kill, just like Unix processes.

**Stack:** Rust 2021, tokio async, serde (JSON+TOML), oxi-sdk/oxi-ai/oxi-agent (crates.io).

```
User → Channel (Web/CLI/Telegram) → Gateway → Kernel (supervisor + ouroboros + agent_runtime)
```

```
oxios/                     # Main binary (src/main.rs, src/kernel.rs, src/cmd_run.rs)
├── crates/
│   ├── oxios-kernel/      # Core: supervisor, event bus, state store, tools, memory
│   ├── oxios-ouroboros/   # Spec-first protocol (interview → seed → execute → evaluate → evolve)
│   └── oxios-gateway/     # Channel-agnostic message hub
├── channels/
│   ├── oxios-web/         # Web dashboard (Axum backend + Dioxus/WASM frontend)
│   ├── oxios-cli/         # CLI channel
│   └── oxios-telegram/    # Telegram channel
├── .programs/             # OS-level programs (code-review, debug, deploy, guardian, refactor)
├── share/                 # Default skills, programs, config
└── docs/                  # Architecture docs, RFCs, design docs
```

**Dependency graph:**
```
oxios → oxios-kernel → oxi-sdk/oxi-agent/oxi-ai (crates.io, NOT path deps)
                    → oxios-ouroboros
      → oxios-gateway
      → oxios-web/oxios-cli/oxios-telegram (channel plugins, feature-gated)
```

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
cargo run                  # Run oxios
```

## CLI for Self-Testing

Oxios CLI is designed for **programmatic consumption** — agents can call it via `exec` tool:

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
  "seed_id": "uuid | null",
  "agent_id": "uuid | null",
  "phase_reached": "Execute",
  "evaluation_passed": true,
  "exit_code": 0,
  "duration_ms": 3500
}
```

## Conventions

- **Language:** Code, comments, docs, commits — English. User-facing messages — Korean.
- **Rust:** `#![warn(missing_docs)]` on public crates. `anyhow` for apps, `thiserror` for libs.
- **Naming:** Crates `oxios-<component>`, public API `verb_noun` (`fork_agent`, `create_seed`).
- **Testing:** Unit tests in `#[cfg(test)] mod tests`. Integration tests in `tests/` per crate.
- **Commits:** `<type>(<scope>): <description>` — scopes: kernel, ouroboros, gateway, web, cli, docs.

## Key Architecture Points

- **Supervisor** (`supervisor.rs`) — Agent lifecycle: fork/exec/wait/kill. The "init" of Oxios.
- **Orchestrator** (`orchestrator.rs`) — Runs the Ouroboros protocol end-to-end. The "brain".
- **AgentRuntime** (`agent_runtime.rs`) — Wraps oxi-agent's tool-calling loop.
- **ExecTool** (`tools/exec_tool.rs`) — Two modes: `shell` (bash -c, RBAC-enforced) and `structured` (binary allowlist + metacharacter blocking).
- **AccessManager** (`access_manager/`) — OWASP-inspired least-privilege. RBAC, path sandboxing, audit logging.
- **Memory** (`memory/`) — Vector store with hyperbolic embeddings, HNSW indexing, flash attention, reasoning bank.
- **KernelHandle** (`kernel_handle/`) — Facade exposing typed APIs (AgentApi, SpaceApi, SecurityApi, etc.) to tools.
- **Kernel** (`src/kernel.rs`) — `Kernel::builder().build().await` assembles all components. `execute_prompt_with_session()` for CLI execution.
- **Program** (`program/`) — OS-level installable capabilities. See `.programs/` for examples.
- **A2A** (`a2a.rs`) — Google's agent-to-agent protocol. Horizontal agent communication.

## Detailed Docs (read when relevant)

| File | When to read |
|------|-------------|
| `docs/ARCHITECTURE.md` | Before modifying kernel structure or adding modules |
| `docs/channel-plugin-guide.md` | Before adding a new channel (Web, Telegram, etc.) |
| `docs/rfc-001-kernel-facade.md` | Before modifying KernelHandle or tool APIs |
| `docs/refactoring-design.md` | Before large-scale refactoring |
| `docs/program-development.md` | Before creating or modifying programs |
| `share/default-config.toml` | Before changing configuration options |

## Pitfalls

- **Workspace deps**: If `cargo build` fails with missing `oxi-ai`/`oxi-agent`, ensure they're in `[workspace.dependencies]` in `Cargo.toml` AND `[dependencies]` in the crate using them.
- **Stdin blocking**: `oxios run --context-file -` reads stdin to EOF. Don't use with interactive input — it blocks.
- **Session IDs**: Sessions live in orchestrator memory. Process restart loses them. Use `--session` only within a single CLI session chain.
