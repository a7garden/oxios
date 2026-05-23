# Cross-Crate Dependency Analysis

> Generated: 2026-05-23  
> Workspace version: 0.1.3 / crate versions 0.2.0

## Dependency Graph

```
                    ┌─────────────────┐
                    │  oxios (binary)  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
      ┌──────────────┐ ┌──────────┐ ┌──────────────┐
      │ oxios-kernel │ │ oxios-   │ │ oxios-       │
      │              │ │ gateway  │ │ markdown     │
      └──┬───────┬───┘ └────┬─────┘ └──────────────┘
         │       │          │
         │       │          │ (depends on kernel)
         ▼       │          ▼
  ┌────────────┐  │   ┌──────────┐
  │ oxios-     │  │   │ channels │
  │ ouroboros  │  │   │ (web/cli │
  └────────────┘  │   │  /tele)  │
                  │   └──┬───┬───┘
                  │      │   │
                  └──────┘   │
                   (gateway) │
                             │
                    ┌────────┘
                    ▼
             oxios-kernel (web only)
             oxios-markdown (web only)
             oxios-ouroboros (web only)
```

**Simplified DAG:**

```
oxios-ouroboros  ←─  oxios-kernel  ←─  oxios-gateway  ←─  channels (cli, telegram)
oxios-markdown   ←─  oxios-kernel                       ←─  channels (web: also kernel+markdown+ouroboros)
                     oxios-kernel  ←──────────────────────  oxios-web (direct)
```

---

## Crate-by-Crate Breakdown

### 1. `oxios-ouroboros` (leaf crate)

**Workspace deps:** None

**Imports from other workspace crates:** None

This is a pure leaf crate. It defines the spec-first protocol (Interview → Seed → Execute → Evaluate → Evolve) and has no dependencies on other workspace crates.

---

### 2. `oxios-markdown` (leaf crate)

**Workspace deps:** None

**Imports from other workspace crates:** None

Another leaf crate. Provides `VirtualFs`, `BacklinkIndex`, markdown parsing, checklist operations, and `KnowledgeBase`. Fully standalone.

---

### 3. `oxios-kernel` (core crate)

**Declared in `Cargo.toml`:**
- `oxios-ouroboros` (path dep)
- `oxios-markdown` (path dep)

#### Imports from `oxios-ouroboros`

| File | Types Used |
|------|-----------|
| `orchestrator_files/mod.rs` | `Phase` (re-exported) |
| `supervisor.rs` | `Seed`, `ExecutionResult` |
| `agent_lifecycle.rs` | `ExecutionResult`, `Seed` |
| `agent_runtime.rs` | `ExecutionResult`, `Seed`, `Entity` |
| `agent_group.rs` | `Seed` |
| `orchestrator.rs` | `EvaluationResult`, `InterviewResult`, `OuroborosProtocol`, `Phase`, `Seed` |

#### Imports from `oxios-markdown`

| File | Types Used |
|------|-----------|
| `tools/kernel/knowledge_tool.rs` | `KnowledgeBase` |
| `kernel_handle/knowledge_lens.rs` | `knowledge::FileChange::*` (enum variants) |

---

### 4. `oxios-gateway` (message router)

**Declared in `Cargo.toml`:**
- `oxios-kernel` (path dep, `default-features = false`)

#### Imports from `oxios-kernel`

| File | Types Used |
|------|-----------|
| `gateway.rs` | `Orchestrator` (Arc-wrapped, constructor + method calls) |
| `plugin.rs` | `KernelHandle` (in `ChannelContext.kernel`), `OxiosConfig` (in `ChannelContext.config`) |

The gateway holds an `Arc<Orchestrator>` for routing messages and passes `Arc<KernelHandle>` + `Arc<OxiosConfig>` to channel plugins via `ChannelContext`.

---

### 5. `oxios-web` (web dashboard channel)

**Declared in `Cargo.toml`:**
- `oxios-gateway` (path dep)
- `oxios-kernel` (path dep, `default-features = false`)
- `oxios-markdown` (path dep)
- `oxios-ouroboros` (path dep)

This is the most coupled channel — it directly imports from **4** workspace crates.

#### Imports from `oxios-gateway`

| File | Types Used |
|------|-----------|
| `channel.rs` | `Channel` (trait), `IncomingMessage`, `OutgoingMessage` |
| `plugin.rs` | `ChannelBundle`, `ChannelContext`, `ChannelPlugin` |
| `routes/cron_jobs.rs` | `IncomingMessage` |
| `routes/chat.rs` | `IncomingMessage` |

#### Imports from `oxios-kernel`

| File | Types Used |
|------|-----------|
| `server.rs` | `config`, `KernelHandle`, `OxiosConfig` |
| `persona_routes.rs` | `Persona` |
| `routes/events.rs` | `state_store::SessionId`, `event_bus::KernelEvent` |
| `routes/workspace.rs` | `memory::{MemoryEntry, MemoryType}` |
| `routes/infra.rs` | `access_manager::AuditEntry`, `metrics::registry`, `ArgumentDef` |
| `routes/budget_routes.rs` | `budget::BudgetLimit`, `types::AgentId` |
| `routes/resources.rs` | `InstallSource` |
| `routes/cron_jobs.rs` | `CronJob`, `Priority` |

#### Imports from `oxios-markdown`

| File | Types Used |
|------|-----------|
| `server.rs` | `KnowledgeBase` |
| `plugin.rs` | `KnowledgeBase` |

#### Imports from `oxios-ouroboros`

| File | Types Used |
|------|-----------|
| `routes/workspace.rs` | `Seed` |

---

### 6. `oxios-cli` (interactive CLI channel)

**Declared in `Cargo.toml`:**
- `oxios-gateway` (path dep only — no kernel dependency!)

#### Imports from `oxios-gateway`

| File | Types Used |
|------|-----------|
| `channel.rs` | `Channel` (trait), `IncomingMessage`, `OutgoingMessage` |
| `plugin.rs` | `ChannelBundle`, `ChannelContext`, `ChannelPlugin` |

**No imports from `oxios-kernel`.** The CLI is a thin shell — it gets `KernelHandle` via `ChannelContext` from the gateway plugin system, never importing kernel types directly.

---

### 7. `oxios-telegram` (Telegram bot channel)

**Declared in `Cargo.toml`:**
- `oxios-gateway` (path dep only — no kernel dependency!)

#### Imports from `oxios-gateway`

| File | Types Used |
|------|-----------|
| `lib.rs` | `Channel` (trait), `IncomingMessage`, `OutgoingMessage` |
| `plugin.rs` | `ChannelBundle`, `ChannelContext`, `ChannelPlugin` |

Like CLI, **no imports from `oxios-kernel`.** Pure gateway plugin.

---

### 8. `oxios` (binary crate — `src/`)

**Declared in root `Cargo.toml`:**
- `oxios-kernel`
- `oxios-markdown`
- `oxios-ouroboros`
- `oxios-gateway`
- `oxios-web` (optional, feature-gated)
- `oxios-cli` (optional, feature-gated)
- `oxios-telegram` (optional, feature-gated)

The binary crate (`src/kernel.rs`) is the **assembler**. It:
1. Instantiates all kernel components from `oxios-kernel`
2. Creates `OuroborosEngine` from `oxios-ouroboros`
3. Creates `KnowledgeBase` from `oxios-markdown`
4. Creates `Gateway` from `oxios-gateway`
5. Wires them into a `Kernel` struct
6. Provides `KernelHandle` facade via `Kernel::handle()`
7. Registers channels via `Kernel::register_channel()`

---

### 9. `oxios-bench` (benchmarking)

**No workspace crate imports detected in source.** (Likely depends on them via `Cargo.toml` but doesn't `use` them directly in the benchmark source examined.)

---

## Summary Table

| Crate | Depends On | Direct Imports From |
|-------|-----------|-------------------|
| `oxios-ouroboros` | — (leaf) | — |
| `oxios-markdown` | — (leaf) | — |
| `oxios-kernel` | ouroboros, markdown | `Seed`, `Phase`, `ExecutionResult`, `EvaluationResult`, `InterviewResult`, `OuroborosProtocol`, `Entity`, `KnowledgeBase`, `FileChange` |
| `oxios-gateway` | kernel | `Orchestrator`, `KernelHandle`, `OxiosConfig` |
| `oxios-web` | gateway, kernel, markdown, ouroboros | `Channel`, `IncomingMessage`, `OutgoingMessage`, `ChannelPlugin`, `ChannelBundle`, `ChannelContext`, `KernelHandle`, `OxiosConfig`, `Persona`, `SessionId`, `KernelEvent`, `MemoryEntry`, `MemoryType`, `AuditEntry`, `ArgumentDef`, `BudgetLimit`, `AgentId`, `InstallSource`, `CronJob`, `Priority`, `KnowledgeBase`, `Seed` |
| `oxios-cli` | gateway | `Channel`, `IncomingMessage`, `OutgoingMessage`, `ChannelPlugin`, `ChannelBundle`, `ChannelContext` |
| `oxios-telegram` | gateway | `Channel`, `IncomingMessage`, `OutgoingMessage`, `ChannelPlugin`, `ChannelBundle`, `ChannelContext` |
| `oxios` (binary) | all above | Assembles and wires everything |

---

## Coupling Observations

1. **CLI and Telegram are well-isolated** — they only know about `oxios-gateway` and get kernel access through the plugin system's `ChannelContext`. This is the ideal pattern.

2. **Web is heavily coupled** — it directly imports from 4 workspace crates. This is partly by design (the web dashboard exposes kernel internals like audit trails, memory, budget, cron) and partly because it creates `KnowledgeBase` directly (RFC-003: web bypasses kernel for knowledge).

3. **Gateway depends on kernel** for `Orchestrator` (message routing) and `KernelHandle`/`OxiosConfig` (plugin context). This is a necessary coupling since the gateway must route messages through the orchestrator.

4. **Kernel's dependencies are clean** — it only imports protocol types from `ouroboros` and knowledge types from `markdown`. No circular dependencies exist.

5. **No circular dependencies** in the workspace. The DAG is clean:
   ```
   ouroboros, markdown → kernel → gateway → channels
   ```
   With the web channel being the only one that breaks the strict layering (reaching through gateway to kernel + markdown + ouroboros directly).

6. **`default-features = false`** is used for `oxios-kernel` in gateway, web, and CLI — this avoids pulling in the `browser` feature (with its `oxibrowser-core` dependency) unnecessarily.
