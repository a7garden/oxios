# Changelog

All notable changes to this project are documented in this file.

and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.10.0] - 2026-06-25

### Added
- **RFC-029: execution resilience** — OTP-style recovery layered on the existing Unix supervisor: snapshot/restore, `SupervisorPolicy` + `RestartBackoff`, and `ModelSwitched` lifecycle events (adopted from oxi-sdk). A bounded recovery ladder runs on provider failure: L0 execute → L1 restart (same model) → L2 snapshot+restore-with-new-model → L3 compact-or-larger → L4 A2A delegate → L5 terminal `ResilientFailure`. Backed by error classification (`FailureClass`), a shared `AttemptBudget`, and a per-provider circuit breaker (`ProviderHealthRegistry`) that replaces the global `LLM_CIRCUIT_BREAKER`.

### Fixed
- **P0: provider errors now propagate as `Err`** — `run_agent` previously swallowed provider failures as `Ok(success:false)`, burying them in `ExecutionResult.output`. It now returns `Err`, so the lifecycle boundary and the recovery ladder can react. `ExecutionResult` carries `failure_class` + `restore_state` so the class/state survive even when a caller returns `Ok(success:false)`.

### Changed
- `oxios-ouroboros` gains a resilience bridge for directive-level recovery; the orchestrator wires `RecoveryCoordinator` behind a read lock and falls back to the direct lifecycle when unconfigured.

## [1.9.0] - 2026-06-24

### Added
- **RFC-027: single-path intent pipeline** — Ouroboros type reorg consolidating the intent (assess → crystallize → execute → review) flow into a single path; orchestrator/agent-runtime/gateway migrated to root-level ouroboros types.
- **RFC-024: web↔daemon reliability** — full SP1–SP4 close: message ordering + replay buffer, atomic web-dist swap (no 404 window), subsystem readiness gate (503 until warm, Degraded counts as ready), and client-side WS keepalive/resume.

### Fixed
- **Chat WebSocket connects when auth is disabled** — v1.8.1's F3 token hardening blocked the WS in the default no-auth config (no login UI exists to set a token), leaving chat stuck on "재연결 중". The frontend now reads `auth_enabled` from `/api/status` (newly exposed) and connects without credentials when auth is off.
- **Auth-enabled browser WebSocket** — `/api/chat/stream` no longer fails the upgrade under `require_auth` (browsers cannot attach a Bearer header to a WebSocket); authentication is enforced by the handler via the `?ticket=` query param.
- **Memory HTTP API wired to the MemoryManager** — list/get/stats/pin/delete previously read the legacy category state-store while `create` wrote to the SQLite MemoryManager, so the memory page was always empty and mutations 404'd. All five handlers now use the MemoryManager (via four new `AgentApi` methods), and four missing routes are registered (`dream/status`, `dream/reports`, `{id}/pin`, `DELETE {name}`). Response shapes match the frontend `MemoryDetail`/`MemoryStats` types.
- **Memory overview renders in production builds** — recharts 3.x `BarChart`/`PieChart` threw `TypeError: t is not a function` when bundled by rolldown (vite v8); replaced with a dependency-free CSS bar.
- **Web lint** — auto-fixed pre-existing biome violations (`useLiteralKeys`, `organizeImports`) that failed the v1.8.1 release CI.

### Changed
- Kernel/orchestrator/gateway refactored to root-level ouroboros types; legacy five-phase integration tests dropped.

## [1.7.1] - 2026-06-22

### Changed
- **Cargo.lock update** — Lockfile refresh to include the correct dependency resolution for the v1.7.0 release.

## [1.8.1] - 2026-06-22

### Changed
- **oxi-sdk 0.37.1 → 0.45.1.** Workspace dependency bumped. `oxi-agent`'s `AgentConfig` gained four `#[serde(skip, default)]` fields (`ttsr_engine`, `memory`, `todo`, `agent_pool`); the single construction site in `crates/oxios-kernel/src/agent_runtime.rs::run_agent` now ends with `..Default::default()`. Catalog-port (0.37.0), `ask` tool rename (0.40.0), edition-2024 lift (0.41.x), and `resolve_model_from_id` catalog fallback (0.45.0) are all additive; no source-level behavior change for oxios.
- **ProjectManager schema initialization** — `ProjectManager::new` now calls `ensure_project_schema` to bootstrap the project database tables, mirroring `MountManager`'s startup behavior.


## [1.8.0] - 2026-06-22

### Added — RFC-028: Web UI Delivery
- **AgentStopped `success` flag (SP-1a)** — `KernelEvent::AgentStopped` now carries `success: bool`. `sanitize_event` serializes it as `agent_stopped.success` on the SSE wire. The supervisor emits `result.success` on the Ok path and `false` on kill/terminate. `#[serde(default)]` keeps older consumers working.
- **Completion notifications (SP-1b)** — `use-global-events.ts` handles `agent_stopped` events: `success:true` → "Task Completed" (success severity), `success:false` → "Task Failed" (warning). Cross-event dedup suppresses `agent_stopped(success:false)` when `agent_failed` was already emitted within 30s.
- **Notification persistence (SP-1c)** — Zustand `persist` middleware stores unread notifications (max 30) in `localStorage` under `oxios-notifications`. Read notifications are transient.
- **Desktop notifications + sound (SP-1d)** — New `desktop-notify.ts` (Notification API, background-tab only) and `sound.ts` (Web Audio oscillator, severity-distinct tones). Integrated into `use-global-events`.
- **Notification preferences (SP-1e)** — Client-side toggles for desktop notifications, sound, completion sound, and error sound in a new Settings → Notifications section. Stored in `localStorage`.
- **Declarative config sections (SP-2a)** — Six config sections now editable in Settings: `calendar`, `otel`, `agent_log`, `resource_monitor`, `browser`, `budget`. All use the existing declarative field-defs framework; no backend changes needed.
- **Secrets API (SP-2b)** — `GET/PUT/DELETE /api/secrets[/{key}]` and `GET /api/secrets/{key}/source`. Stores credentials in `~/.oxi/auth.json` via `CredentialStore`, never in `config.toml` plaintext. Responses are masked (`has_value`, `source`, `preview`).
- **Secrets UI (SP-2c)** — Settings → Secrets section with per-key password inputs, source badges, and masked previews.
- **Trace trajectory join (SP-3a)** — `GET /api/agents/{id}/trace` now merges session trajectory steps with `agent.tool_calls` (deduped by `tool_call_id`). Trace steps carry a `kind` field (`tool` | `memory` | `reasoning`) for future expansion.
- **UI polish (SP-4)** — Shadow tokens added (`--shadow-sm/md/lg`) with dark-mode alpha 0.2–0.4 vs light 0.04–0.08. Background raised to `oklch(0.99 0 0)` for card elevation. `focus-visible` added to header/sidebar buttons. Global `<kbd>` styling.

### Changed
- `CredentialStore` gains `delete()` and `resolve_secret()` methods for non-provider key management.
- `settings.tsx` `buildPayload` now parses `multiline` fields as JSON (for `browser.engine`); form population JSON.stringifies multiline object values.
- `SectionIconKey` union extended with 8 new icon keys; `section-icons.tsx` `ICON_MAP` updated.
- Settings consistency test updated to include `secrets` and `notifications` custom sections.
## [1.6.1] - 2026-06-21

### Fixed
- **Web daemon startup reliability** — Hardened `oxios start` / `oxios serve` against silent failure modes (RFC-024 territory):
  - Pre-spawn port guard detects an orphaned oxios process still holding the port past a stale/missing pidfile, so the spawned daemon's bind no longer fails silently while the readiness probe reports success against the old listener.
  - A readiness-probe miss now surfaces the daemon log tail and fails the start instead of printing a misleading "started".
  - `oxios serve` refuses to start a daemon whose web assets could not be obtained (it would have served 503 on every web request); CLI/Telegram-only configs with the web surface disabled are unaffected.
  - `web_dist` auto-download from GitHub Releases now retries with a bounded backoff so a transient network blip or rate-limit does not strand the daemon.
  - Unit tests added for `port_in_use` and the startup guards.

## [1.6.0] - 2026-06-21

### Added
- **Interview wizard a11y / keyboard** — Roving focus for option groups (ArrowLeft/Right on `single_choice` auto-selects like a native radiogroup), Space to focus-and-select, Shift+Enter inserts a newline in `free_text`, and `role="group"` / `aria-pressed` / `aria-label` on option buttons so screen readers announce selection state and group semantics. The `keyboardHint` strings (en/ko) are updated to reflect the new bindings. A new test file covers the keyboard + selection behavior across `single_choice`, `multi_choice`, and `free_text` kinds.

### Changed
- **Refactor: live model resolution via `ModelResolver` port** — All LLM-bound phases now read the live, post-hot-swap engine default through a new `ModelResolver` trait (`oxios-ouroboros::ModelResolver`) instead of capturing a frozen model id at construction. This eliminates the divergence where interview / seed / evaluate / evolve used a boot-time model while execute re-resolved via the engine handle, and surfaces a bad model id at the first phase call instead of silently at execute.
  - `OuroborosEngine::new` now takes `Arc<dyn ModelResolver>` and resolves the live default + provider at the start of every LLM-bound phase. Tests use a new `StaticModelResolver` helper.
  - `EngineHandle` (kernel) implements `ModelResolver`; `OxiosEngine` gains a provider cache that survives across reads within one generation and is cleared on `swap`.
  - `EngineApi::set_model` validates the new model BEFORE persisting (rejects unknown models / unconfigured providers), so a Web UI "switch succeeded" is truthful and a bad model id no longer surfaces only at execute time.
  - `AgentRuntime`, `PersistenceHook`, `KnowledgeDream`, `KnowledgeLens` drop their frozen `model_id` fields and resolve live on each call.
  - Boot-time validation: a broken configured model now fails the daemon fast instead of silently at every curation run (`KnowledgeDream`, `KernelBuilder`).

### Fixed
- **Clippy: clear pre-existing lints on v1.5.2** — A clippy upgrade since v1.5.2 surfaced 38 mechanical lints (in `option_map_unit_fn`, `field_reassign_with_default`, `items_after_test_module`, `needless_borrows_for_generic_args`, `nonminimal_bool`, `ptr_arg`, `useless_conversion`, `cloned_ref_to_slice_refs`, `unused_imports`, and `dead_code`). All are addressed without behavior change. `cargo clippy --workspace --all-targets -- -D warnings` (the documented quality gate) now passes locally and matches CI.

## [1.5.1] - 2026-06-17

### Fixed
- **Security: wasmtime-wasi RUSTSEC-2026-0182** — Upgraded the `wasmtime` / `wasmtime-wasi` dependency from 22 to 24.0.10 (the backport release that fixes the WASIp1 `fd_renumber` resource leak). `cargo audit` now reports zero vulnerabilities. `wasm-sandbox` is still an optional, non-default feature, so default builds were unaffected, but the published `oxios-kernel` now resolves to the patched transitive dependency.

## [1.5.0] - 2026-06-17

### Added
- **`oxios update` overhaul** — Progress bars for all three update stages (web UI download with byte/speed/ETA, zip extraction file count, `cargo install` spinner that reflects the live compile line) and automatic daemon restart after a successful update so the new binary/web UI takes effect immediately. A `--no-restart` flag opts out, and restart only fires when the daemon is already running.

### Fixed
- **Web i18n (Korean UI)** — Restored 189 translation keys that were missing from both `en.json` and `ko.json` (mounts, projects, email, knowledge UI, chat/questionnaire, agents/sessions, dataTable, shared common/settings), which had been rendering as raw `section.key` strings in the UI.
- **`oxios update`** — A daemon restart failure no longer masks a successful update; it now warns and points at `oxios start` for manual recovery instead of exiting as a failure.
- **Web i18n polish** — `questionnaire.count` singular/plural ("1 questions"), mounts rescan terminology consistency, and removal of a dead duplicate `chat.questionnaire.*` namespace.

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
