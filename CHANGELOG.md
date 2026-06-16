# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] - 2026-06-16

### Added
- **RFC-024 web↔daemon reliability** — Atomic static-asset distribution with content-hash references, hard timeouts on all SSE/WS streams, and a readiness gate (SP4) so the web surface only serves after the kernel is fully initialized. Gateway gains a SP1/SP2 reliability layer.
- **RFC-025 Mount + Project system** — Unified notion of host directories mounted into the workspace as first-class Project bundles:
  - Mount core + Workspace Context injection (Phase 1) and Project bundle layer + agent enrichment (Phase 2/3).
  - Frontend Mount UI with detection badge and Project bundle rendering.
  - Phase 4 Mount rescan; Phase 5 frequent-path auto-promotion to Mounts.
  - Project-tree sidebar with drag-to-reparent and data migration.
- **Mobile responsive design (Web)** — Full responsive redesign (Phases 1–5) across chat, control, browse, and settings surfaces.
- **Settings UX overhaul (Web)** — Range sliders, full tool checklist (replacing the allowed_tools tag-input), CORS editor, and field-control polish.

### Changed
- **Version bump to 1.4.0** — All crates updated to 1.4.0; web `package.json` aligned to 1.4.0.
- **Rust 2024 edition + oxi-sdk 0.35.0** — Workspace migrated to edition 2024 and bumped to oxi-sdk 0.35.0 (native-browser fix).
- **wasm-sandbox wasmtime 22 migration** — Resolved `WasiCtx`, `fuel_remaining`, `define_wasi`, and `Memory::read` API drift; `cargo build/clippy --workspace --all-features` now passes cleanly.
- **Iconography (Web)** — Replaced emoji across the UI with lucide-react icons.

### Fixed
- **RFC-025 review pass** — Fixed all critical, major, and minor issues identified in the review across the stack (remaining substantive bugs, last design issues).
- **Settings** — Phantom memory changes from a non-existent field key; `dream_interval_hours` slider max reduced from 168h to 72h; settings shell flex layout-break on narrow screens.
- **Web** — Accidental text selection on interactive UI chrome.
- **Frontend provider catalog** — Missing provider models added to the fallback catalog.

## [1.3.0] - 2026-06-13

### Added
- **Agent History Log** — Persistent agent records survive daemon restarts.
  - Dual-tier storage: filesystem JSON (source of truth, `state/agents/<id>.json`) + SQLite query index (`state/agent_log.db`) with FTS5 full-text search.
  - `AgentLogDb` query engine: filtering (status, date range, session/project/seed), sorting (cost, duration, tokens, name), pagination, search across agent name / error / tool names / tool outputs.
  - `KernelHandle::reindex()` rebuilds the SQLite index from filesystem JSON at any time. SQLite is optional via the `sqlite-memory` feature; falls back to filesystem scan when disabled.
- **`AgentStatus::Completed`** — New terminal status for agents that finish successfully; integrated into the agent stats aggregation (`Idle`/`Stopped`/`Completed` → `completed`).
- **RFC-015 knowledge/memory separation** — Distinguished agent memory (`MemoryManager`) from user knowledge notes (`KnowledgeBase`), clarifying the two-system boundary.
- **RFC-016 autonomous persistence** — Agent-generated notes persist with provenance metadata automatically.
- **RFC-022 knowledge provenance, quality metadata & dream curation** — Notes carry `source` (Hook/Agent) and `quality` (Raw/Reviewed) frontmatter; dream consolidation curates based on quality.
- **Interactive interview wizard (Web)** — Multi-round Ouroboros interview UI with Q&A preserved across turns, typing indicator, and structured question rendering.
- **Chat & dashboard redesign (Web)** — Redesigned chat (tool-name transparency, session titles, keyboard shortcuts) and dashboard (agent status, system health, live activity feed, approvals queue).

### Changed
- **Version bump to 1.3.0** — All crates updated to 1.3.0.
- **Interview multi-turn context** — Original user message and prior Q&A are now included in interview context so the LLM understands follow-up rounds.
- **Evaluation semantics** — `evaluation_passed` modelled as `Option<bool>` end-to-end (gateway → web → frontend) for correct null semantics.
- **Async-trait restoration** — Replaced manual `Pin<Box<...>>` boilerplate with the `async-trait` macro in the kernel.

### Fixed
- **Test compile & clippy** — Resolved incomplete `agent_log_db` module (added `AgentStatus::Completed` variant, completed `parse_status` mapping) and cleared all `clippy -D warnings` lints in the new code.
- **Agent stats SQL NULL handling** — `SUM(CASE …)` / `AVG(…)` / `MIN`/`MAX` aggregates now wrapped in `COALESCE` and read as `Option`, so stats queries succeed on empty/all-NULL tables.
- **i18n** — Added missing `common.justNow` / `minutesAgo` / `hoursAgo` translation keys.
- **Frontend provider catalog** — Added missing provider models to the frontend fallback catalog.

## [1.1.0] - 2026-06-06

### Added
- **OxiBrowser Observability v0.12 — Phases 3 & 4** — Real-time tool progress flows from the oxi-agent loop through oxios-kernel → oxios-web → frontend.
  - `KernelEvent::ToolExecutionProgress` variant + `agent_runtime` forwarding of `AgentEvent::ToolExecutionUpdate { partial_result }`
  - oxios-web converts the new event into a `tool_progress` WS chunk (and SSE event)
  - Frontend: `StreamChunk.tool_progress` → `ChatActivity.tool_call` with `progress` and `isRunning: true`; `tool_start` sets `isRunning: true`, `tool_end` clears it
  - `ActivityCard` renders a `Loader2` spinner for running tool calls and shows the latest progress text inline
- **OxiBrowser Observability v0.12 — Phase 5 (tab-id propagation)** — Browser tab id propagation through kernel → web → frontend, enabling concurrent tab distinction in the chat transparency timeline.
  - `KernelEvent::ToolExecutionProgress` gains `tab_id: Option<Uuid>` (optional, serde skip-if-none for back-compat).
  - WS/SSE events include `tab_id`; frontend `ActivityCard` shows a short tab-id badge.
  - Audit-action detail string appends `:tab=<id>` when tab is known.
- **RFC-018 b.1: Memory extraction** — `chunking`, `normalizer`, `hyperbolic` modules extracted from `oxios-kernel::memory` to new `oxios-memory` leaf crate.
  - Back-compat: `use oxios_kernel::chunk_fixed` etc. all continue to work.
- **oxios-calendar** — New `.ics`-based calendar event management crate (parse, query, CRUD).
- **Email subsystem** — SMTP-based email sending integration (`leitner`), template management, sent history, provider config.
- **Calendar CLI** — `oxios calendar` subcommand with `list`, `add`, `delete`, `search`, `import`, `export`.
- **Email CLI** — `oxios email` subcommand with `setup`, `test`, `history`, `templates`.
- **Email & Calendar REST API** — Full CRUD endpoints on `/api/email/*` and `/api/calendar/*`.

### Changed
- **Version bump to 1.1.0** — All crates updated to 1.1.0 for first crates.io publication.
- **Memory re-export layer** — `oxios-kernel` re-exports the moved memory types so downstream crates (web, gateway) require no source changes.
- **Release profile applied** — `[profile.release]` with `lto = "thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`, `opt-level = 3`. Binary size ~50 MB.
- **CI workflow hardened** — Workflow-level `permissions: contents: read`; `cargo-audit` uses `taiki-e/install-action`; target cache key includes `${{ github.sha }}`.
- **Release workflow permissions** — Read-only default; release job keeps `contents: write`.

### Fixed
- **TSC errors** — All 96 pre-existing + 3 v0.12-scope TypeScript errors cleared to 0.
- **Clippy warnings** — 14 warnings in binary crate (`src/main.rs`, `src/kernel.rs`, `src/web_dist.rs`) resolved.
- **CI formatting drift** — `cargo fmt` inconsistencies across kernel, web, and binary crate rectified.
- **CI clippy feature flag** — Fixed `browser` feature not existing on core crates in CI workflow.
- **Dead-code warning** — `WebDistResult::Embedded` marked `#[allow(dead_code)]`.

### Removed
- **Legacy `share/default-programs/`** — Superseded by `share/default-skills/` per RFC-009.

### Release Infrastructure
- **Publish order** — `release.yml` updated: `oxios-memory` and `oxios-calendar` added to publish sequence in correct dependency order.

## [1.0.2] - 2026-05-31

### Changed

- **Version bump to 1.0.2** — All crates updated: oxios, oxios-kernel, oxios-markdown, oxios-ouroboros, oxios-gateway, oxios-mcp, oxios-web, oxios-cli, oxios-telegram
- **Path dependencies updated** — All internal workspace dependencies now reference 1.0.2

### Notes

- This release prepares crates for publication to crates.io
- Web UI dist should be published to GitHub Releases separately

## [0.5.0] - 2026-05-30

### Added

#### Architecture Review Implementation (RFC-013~020)

- **Gateway Event-Driven** (RFC-013) — `tokio::select!` + shared `mpsc` channel replacing polling loop. Semaphore-bounded concurrency (32). Per-channel `tokio::spawn` receive tasks with graceful shutdown
- **Channel UX Unification** (RFC-014) — Shared `format.rs` module (CLI/Telegram/Web). `ErrorKind` classification (`error_classify.rs`). Typed `ResponseMeta` (session_id, space_id, seed_id, phase, evaluation_passed, duration_ms). `ChannelFormatter` trait
- **Security Model Integration** (RFC-015) — 4-layer `AccessGate` (CSpace → RBAC → Permissions → ExecConfig) with short-circuit evaluation. `AuditSink` for policy decision recording. `AgentContext` (who/why/where) tracking. `GatedTool` wrapper for permission enforcement
- **Proactive Recall & SONA** (RFC-020) — Activated proactive recall at session start and topic transitions. SONA learning engine: trajectory recording, pattern distillation, embedding-based similarity
- **Ouroboros Evolution Loop** (RFC-019) — Full evaluate + evolve cycle connected. `should_evaluate()`, structured evaluation with caching, LLM-based seed evolution with max iteration control

#### Memory Infrastructure (RFC-012)

- **SQLite Memory Store** — Persistent memory backend replacing in-memory-only storage
- **GGUF Embedding Provider** — Local embedding via llama-gguf (replacing MLX for cross-platform support)
- **PageRank** — Importance scoring via link graph analysis
- **Hyperbolic Embeddings** — Hierarchical memory representation
- **Flash Attention** — Efficient context window utilization
- **Auto Memory Bridge** — Automatic memory operations during agent execution

#### Observability & Routing

- **Observability Module** — `Tracer`, `CostTracker`, `AuditLog` for production monitoring
- **Model Routing** — `EngineConfig` + `RoutingControl` for complexity-based model selection
- **ProviderPool** — Rate limiting across LLM providers
- **AgentPool** — Session persistence for multi-turn conversations without re-creation
- **StructuredOutput** — Evaluation result parsing with typed output

#### Frontend

- **i18n** — English and Korean support with react-i18next
- **Session Prune API** — `DELETE /api/sessions/prune` for stale session cleanup

#### Coordination

- **Middleware Pipeline** — Audit logging middleware for agent execution
- **Coordination Module** — Multi-agent coordination primitives

### Changed

- **oxi-sdk 0.22.0 → 0.23.0** — Removed direct `oxi-ai` deps, use `oxi_sdk::Oxi` via `OxiBuilder`
- **Agent Runtime** — Uses `Agent::run_streaming()` instead of deprecated `AgentLoop`
- **Kernel Re-exports** — 33 dead re-exports moved to `sdk_exports` module
- **Web surface promotion** — `channels/oxios-web` → `surface/oxios-web` (first-class citizen)
- **Frontend auth** — `getToken()` / `api-client` / `sse-client` unified to `useAuthStore` (single source of truth)
- **Config UX** — `toml_edit`-based `config set` (comment-preserving). Added `config list`, `config reset` subcommands
- **Clippy** — 82 → 0 warnings across entire workspace
- **Version bumped** to `0.5.0`

### Fixed

- **MutexGuard across await** in `sona.rs` — potential deadlock eliminated
- **agent_id RBAC bug** — `can_access_path_in_workspace` now receives real `AgentId` instead of random UUID
- **ExecTool production connection** — `with_exec_tool()` properly wired in kernel assembly
- **SQLite deadlocks** in memory tests + CJK BM25 tokenization support
- **Engine credential injection** — `validate_key` improvement for multi-provider setup
- **Release workflow** — Path corrected from `channels/oxios-web` to `surface/oxios-web`
- **`ko-KR` hardcoded locale** → browser default locale in chat UI

### Removed

- **`reasoning_bank.rs`** — Unused module (RFC-017)
- **`rvf_store.rs`** — Unused module (RFC-017)
- **`lateral.rs` / `regression.rs`** in ouroboros — Superseded by integrated evolution loop
- **`oxi-ai` direct dependency** — All provider construction via `oxi-sdk`
- **280+ missing_docs warnings** — Resolved across kernel crate

## [0.4.0] - 2026-05-25

### Added

#### Tiered Memory System (RFC-008)

- **3-Tier Memory** (`memory/mod.rs`) — Hot (always loaded, ~3K tokens), Warm (on-demand), Cold (compressed archive)
- **Dream Process** (`memory/dream.rs`) — 4-phase background consolidation: Orient → Gather Signal → Consolidate → Prune & Index. Supports checkpointing for crash recovery.
- **Auto-Classification** (`memory/auto_classify.rs`) — Infers `MemoryType` (Fact, Decision, Episode, Knowledge, etc.) from content patterns
- **Auto-Protection** (`memory/auto_protect.rs`) — Automatically promotes protection level based on access frequency, session appearances, and user corrections
- **Decay Engine** (`memory/decay.rs`) — Ebbinghaus-inspired forgetting curve with protection-aware rate adjustment
- **Compaction Tree** (`memory/compaction.rs`) — 5-level compression: Raw → Daily → Weekly → Monthly → Root
- **ROOT Index** (`memory/root_index.rs`) — O(1) topic lookup so agents know what they know without scanning
- **Proactive Recall** (`memory/proactive.rs`) — Automatically injects relevant memories at session start and topic transitions
- **Auto Memory Bridge** (`memory/auto_memory_bridge.rs`) — Bridge between agent runtime and memory subsystem for automatic memory operations
- **Memory Types**: Conversation, Session, Fact, Episode, Knowledge, Skill, Preference, Decision, UserProfile
- **Protection Levels**: None → Low → Medium → High → Permanent (auto-calculated)

#### Unified Skill System (RFC-009)

- **SkillManager** (`skill.rs`) — Unified skill manager replacing `SkillStore` + `ProgramManager` + `HostToolValidator`
- **SKILL.md Frontmatter** — All metadata in YAML frontmatter (no separate `program.toml`)
- **4-Dimensional Requirements** — `bins`, `anyBins`, `env`, `config` checks per skill
- **Install Specs** — Automatic dependency installation: brew, node, go, uv, download
- **Skill Eligibility** — Per-skill status: Ready, NeedsSetup, Disabled with missing requirements details
- **Skill Source Hierarchy** — agent-specific > workspace > global user > bundled
- **Skill Snapshot** — XML prompt injection for agent initialization

### Changed

- **Memory system** upgraded from flat vector store to tiered memory with Dream-time consolidation
- **Skills and Programs merged** into a single unified Skill model
- Version bumped to `0.4.0`

### Removed

- **`program/` module** — replaced by unified `SkillManager` in `skill.rs`
- **`ProgramManager`** — merged into `SkillManager`
- **`SkillStore`** — merged into `SkillManager`
- **`HostToolValidator`** (`host_tools.rs`) — replaced by per-skill `check_requirements()`
- **`program.toml` format** — all metadata now in SKILL.md YAML frontmatter
- **`.programs/` directory** — skills migrated to `share/default-skills/`
- **Programs API endpoints** — merged into `/api/skills`
- **Host Tools API endpoint** — deprecated, functionality in skill eligibility checks

## [0.2.0-alpha] - 2026-05-03

### Added

#### AIOS-Inspired Kernel Extensions

- **AgentScheduler** (`scheduler.rs`) — Priority-based task scheduler with:
  - Priority queue (Critical > High > Normal > Low)
  - Rate-limit-aware admission control
  - Max concurrent task enforcement
  - Zombie task detection and automatic reaping
  - API endpoints: `GET /api/scheduler/stats`, `GET /api/scheduler/tasks`

- **ContextManager** (`context_manager.rs`) — 3-tier context hierarchy:
  - **Active tier**: In-memory, in-context (configurable tokens)
  - **Cache tier**: In-memory, not in-context (LRU entries)
  - **Archive tier**: Compressed on disk (unlimited)
  - Automatic demotion when active tier fills up

- **AccessManager** (`access_manager.rs`) — OWASP-inspired security:
  - Tool access control (allow-list per agent)
  - Path sandboxing (glob patterns for allowed/denied paths)
  - Network restrictions (disabled by default)
  - Execution limits (time and memory)
  - Audit logging (timestamp, agent, action, resource, decision)
  - API endpoints: `GET /api/audit`, `GET/PUT /api/permissions/:agent`

#### Programs System

- **ProgramManager** (`program.rs`) — OS-level installable applications:
  - Install/uninstall programs from directories, git, or tarball URLs
  - Enable/disable programs
  - Host requirements validation
  - Program metadata parsing (program.toml)
  - API endpoints:
    - `GET /api/programs`, `POST /api/programs`
    - `GET /api/programs/:name`, `DELETE /api/programs/:name`
    - `POST /api/programs/:name/enable`, `POST /api/programs/:name/disable`
    - `GET /api/programs/:name/host-requirements`

- **SkillStore** (`skill.rs`) — Markdown-based instruction templates:
  - CRUD operations for skills
  - Storage in `~/.oxios/workspace/skills/`
  - API endpoints: `GET /api/skills`, `POST /api/skills`, `DELETE /api/skills/:name`

#### MCP & Host Tools

- **McpBridge** (`mcp.rs`) — Model Context Protocol awareness:
  - MCP server registration
  - Tool capability enumeration
  - Protocol handshake support
  - API endpoints: `GET /api/mcp/servers`, `POST /api/mcp/servers`

- **HostToolValidator** (`host_tools.rs`) — Minimal container validation:
  - Required vs optional host tool distinction
  - Presence checking via `which`
  - Full host environment audit
  - API endpoint: `GET /api/host-tools`

#### Seeds & Evaluation API

- `GET /api/seeds/:id/evolution` — Track seed evolution lineage with parent links and evaluation scores
- **ExecutionMetadata** (`oxios-ouroboros`) — Per-seed execution tracking:
  - Execution count and rolling average score
  - Success rate calculation
  - User-defined tags for categorization

#### Configuration Enhancements

- `[scheduler]` section — Max concurrent, rate limit, zombie timeout
- `[context]` section — Active/cache/archive tier configuration
- `[security]` section — Audit log size, default tool allowlists
- `[persona]` section — Default persona and concurrent persona limits

#### Persona System

- **PersonaManager** + **PersonaStore** (`persona_manager.rs`, `persona_store.rs`) — Multiple AI characters:
  - Three default personas: Dev, Review, Research
  - Per-persona system prompts and personality traits
  - Active persona switching for orchestrator

#### State & Sessions

- **StateStore** (`state_store.rs`) — Extended with Session management:
  - `SessionId`, `UserMessage`, `AgentResponse`, `Session` types
  - Full conversation history persistence
  - Path traversal protection

### Changed

- Kernel module structure expanded from core modules to include AIOS extensions
- API routes reorganized to group related endpoints logically
- Version bumped to `0.2.0-alpha` across all crates
- `Seed::new()` now includes `execution_metadata` field

### Fixed

- `parking_lots` typo corrected to `parking_lot` in persona modules
- `Deserialize` import added to `state_store.rs`
- `OxiosConfig` default initialization includes all config sections
- Tuple element count mismatch in `init_kernel` callers
- `mut` warning in `PersonaManager::with_defaults`

## [0.1.0-alpha] - 2026-05-03

### Added

- **Core kernel** (`oxios-kernel`) with supervisor, event bus, and state store
- **Ouroboros protocol** (`oxios-ouroboros`) — spec-first workflow:
  interview → seed → execute → evaluate → evolve
- **Gateway** (`oxios-gateway`) with channel-agnostic message routing
- **Web dashboard** (`oxios-web`) with chat, control, and browse panels
- **Removed** container layer — replaced with direct ExecTool execution
- **Host Exec Bridge** for secure macOS command execution
- **Skill system** for markdown-based agent instruction templates
- **CLI** with `run`, `status`, `config`, `pkg`, `agent`, `daemon` subcommands
- **38 tests** (25 unit + 13 integration)
- **7006 lines** of Rust code across 27 source files
- **1761 lines** of HTML for the web dashboard
