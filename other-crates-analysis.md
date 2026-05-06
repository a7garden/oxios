# Oxios Other-Crates Analysis

> Comprehensive analysis of all non-kernel crate source files.
> Generated: 2026-05-06

---

## Table of Contents

1. [oxios-ouroboros](#1-oxios-ouroboros)
2. [oxios-gateway](#2-oxios-gateway)
3. [oxios-web (channel)](#3-oxios-web-channel)
4. [oxios-web/frontend (Dioxus)](#4-oxios-webfrontend-dioxus)
5. [Main Binary (src/main.rs)](#5-main-binary)
6. [Integration Tests](#6-integration-tests)
7. [Documentation Files](#7-documentation-files)
8. [Cross-Cutting Issues](#8-cross-cutting-issues)
9. [Summary Scorecard](#9-summary-scorecard)

---

## 1. oxios-ouroboros

### Files

| File | Lines | Purpose |
|------|-------|---------|
| `lib.rs` | 20 | Public exports and re-exports |
| `protocol.rs` | 76 | `OuroborosProtocol` trait + `Phase` enum + `ExecutionResult` |
| `interview.rs` | 51 | `InterviewResult` data type |
| `evaluation.rs` | 43 | `EvaluationResult` data type (3-stage) |
| `seed.rs` | 156 | `Seed`, `AmbiguityScore`, `Entity` data types |
| `ouroboros_engine.rs` | 506 | LLM-backed `OuroborosEngine` implementation |

### Implementation Status

**Fully Implemented:**
- `Phase` enum with Display impl
- `OuroborosProtocol` async trait with 5 phases + persona hook
- `InterviewResult` with Q&A tracking and ambiguity updates
- `EvaluationResult` with 3-stage (mechanical/semantic/consensus) model
- `Seed` with generation tracking, evolution lineage (`evolved_from()`)
- `AmbiguityScore` with weighted scoring (goal 40%, constraints 30%, criteria 30%)
- `OuroborosEngine` — full LLM-backed implementation using `oxi_ai::Provider`
  - `llm_complete()` — stream-collecting LLM calls with persona prepending
  - `parse_json()` — markdown fence tolerant JSON parsing
  - `interview()` — generates Socratic questions, scores ambiguity
  - `generate_seed()` — crystallizes interview into structured Seed
  - `execute()` — **stub/delegated** (returns placeholder, notes Supervisor handles real execution)
  - `evaluate()` — mechanical (substring check) + semantic (LLM) + consensus (skipped)
  - `evolve()` — LLM-generated improved seed from evaluation feedback

**Stub/Placeholder:**
- `execute()` — Returns a placeholder `ExecutionResult` with `success: false` and `steps_completed: 0`. Comment says "The Orchestrator calls Supervisor::run_with_seed() directly."
- Consensus evaluation (Stage 3) — Always `None`, comment says "would require a second model; skip for now"
- `set_persona_prompt()` on the private method has `#[allow(dead_code)]` (only called through the trait impl)

### Error Handling Quality: **Good**

- All public methods return `anyhow::Result`
- LLM response parsing uses `unwrap_or_else` with fallback defaults and `tracing::warn` — graceful degradation
- Stream error handling extracts text content from error before falling back to bail
- JSON parse failures produce sensible defaults rather than crashing
- `anyhow::bail!` used appropriately for stream errors

### Test Coverage: **Minimal**

- **0 unit tests** in the crate
- `Seed::new()` has a doc test (asserts goal not empty, generation 0)
- `AmbiguityScore::new()` has a doc test (asserts ambiguity < 0.2)
- No tests for `OuroborosEngine` (would require mock Provider)
- No tests for `parse_json()` helper
- No tests for `EvaluationResult::all_passed()`
- No tests for `InterviewResult` mutations
- Tested indirectly via integration tests with `MockOuroboros`

### TODO/FIXME/unwrap/panic: **0**

- No `TODO`, `FIXME`, `unwrap()`, or `panic!` found
- All `.unwrap_or_else()` usages have proper fallback paths

### Integration Quality: **Excellent**

- Clean trait-based design (`OuroborosProtocol`) allows mock substitution
- Uses `oxi_ai::Provider` + `oxi_ai::Model` correctly
- `parking_lot::Mutex` for phase state (not async, appropriate for short locks)
- Persona prompt properly prepended to all LLM calls
- Seed lineage tracking (`parent_seed_id`, `generation`) well-designed
- Serde derive on all data types enables JSON persistence

---

## 2. oxios-gateway

### Files

| File | Lines | Purpose |
|------|-------|---------|
| `lib.rs` | 14 | Public exports |
| `channel.rs` | 22 | `Channel` trait definition |
| `message.rs` | 89 | `IncomingMessage` + `OutgoingMessage` types |
| `gateway.rs` | 163 | `Gateway` routing + event loop |

### Implementation Status

**Fully Implemented:**
- `Channel` trait — minimal, clean: `name()`, `receive()`, `send()`
- `IncomingMessage` / `OutgoingMessage` — UUID IDs, timestamps, metadata HashMap
- `Gateway` — channel registry, message routing through orchestrator, event loop
- `Gateway::route()` — full Ouroboros lifecycle with response metadata
- `Gateway::run()` — polling event loop with configurable interval (100ms)
- `Gateway::register()` — dynamic channel registration

**Potential Issues:**
- `Gateway::run()` uses a polling loop (100ms sleep) rather than async notification — may introduce latency
- `Gateway` holds `Arc<oxios_kernel::Orchestrator>` directly, coupling gateway to kernel

### Error Handling Quality: **Good**

- `route()` catches orchestrator errors and sends error messages back to the channel
- `send_to()` silently drops messages to unknown channels (with warning log)
- All public methods return `anyhow::Result`

### Test Coverage: **None (in crate)**

- **0 unit tests** in the gateway crate itself
- **No test files** exist under `crates/oxios-gateway/tests/`
- Tested via integration tests (`test_gateway_routes_message_through_orchestrator`, `test_gateway_unknown_channel`)

### TODO/FIXME/unwrap/panic: **0**

- No `TODO`, `FIXME`, `unwrap()`, or `panic!` found

### Integration Quality: **Good**

- Clean `Channel` trait allows plugging in any transport
- `Gateway::new()` requires an `Orchestrator` — creates tight coupling to kernel
- Message types are self-contained (no kernel types in gateway messages)
- Gateway depends on `oxios-kernel` (for `Orchestrator`) — this is a circular concern since kernel shouldn't depend on gateway, but gateway-to-kernel dependency is fine

---

## 3. oxios-web (channel)

### Files

| File | Lines | Purpose |
|------|-------|---------|
| `lib.rs` | 15 | Public exports |
| `channel.rs` | 168 | `WebChannel` + `WebChannelHandle` |
| `server.rs` | 194 | `AppState` + `WebServer` + graceful shutdown |
| `routes.rs` | 1,798 | **Massive** route handler file (40+ endpoints) |
| `persona_routes.rs` | 222 | Persona CRUD routes |

### Implementation Status

**Fully Implemented (routes.rs — 40+ endpoints):**

| Category | Endpoints | Status |
|----------|-----------|--------|
| Chat | POST /api/chat, GET /api/chat/stream (WebSocket) | ✅ Full |
| Control | GET /api/status, GET /api/agents, POST /api/agents/:id/kill | ✅ Full |
| Config | GET /api/config, PUT /api/config (with file persistence) | ✅ Full |
| Workspace | GET /api/workspace/tree, GET/PUT /api/workspace/file/* | ✅ Full (with path traversal protection) |
| Seeds | GET /api/seeds, GET /api/seeds/:id, GET /api/seeds/:id/evolution | ✅ Full |
| Skills | GET/POST /api/skills, GET/DELETE /api/skills/:name | ✅ Full |
| Memory | GET /api/memory, GET /api/memory/:name | ✅ Full |
| Gardens | Full CRUD + exec (7 endpoints) | ✅ Full |
| Scheduler | GET /api/scheduler/stats, GET /api/scheduler/tasks | ✅ Full |
| Audit | GET /api/audit | ✅ Full |
| Permissions | GET/PUT /api/permissions/:agent | ✅ Full |
| Programs | Full CRUD + enable/disable + host-requirements (7 endpoints) | ✅ Full |
| Host Tools | GET /api/host-tools | ✅ Full |
| MCP | 4 endpoints (commented out routes, but handlers exist) | ⚠️ Reserved |
| Events | GET /api/events (SSE stream) | ✅ Full |
| Sessions | GET/DELETE /api/sessions/:id, GET /api/sessions | ✅ Full |
| Approvals | GET /api/approvals, POST approve/reject | ✅ Full |
| Personas | Full CRUD + active management (7 endpoints) | ✅ Full |

**Stub/Reserved:**
- MCP endpoints — Handlers exist with `#[allow(dead_code)]` but routes are commented out in `build_routes()`
- `handle_status` returns `uptime: "n/a"` — no actual uptime tracking
- `handle_workspace_file_get` uses `unwrap_or_else` for canonicalize fallback (acceptable)

**Notable Implementation Details:**
- WebSocket chat: bidirectional (receives text → pushes to gateway, forwards outgoing to WS)
- SSE events: broadcast stream with keep-alive pings every 30s
- Seed evolution: iterative lineage building using work stack
- Path traversal protection on workspace file endpoints (canonicalize + starts_with check)
- Config PUT validates against `OxiosConfig` schema before persisting

### Error Handling Quality: **Fair**

- Route handlers return typed errors: `StatusCode`, `(StatusCode, String)`, or `Result<Json<T>, StatusCode>`
- Inconsistent error types across handlers — some use `StatusCode` only (loses error detail), others use `(StatusCode, String)`
- `handle_chat` returns `StatusCode::INTERNAL_SERVER_ERROR` with no error detail
- `handle_config_put` properly validates and returns descriptive errors
- Garden routes consistently use `(StatusCode, String)` for good error messages
- Several handlers silently swallow errors: `Err(_) => Json(Vec::new())` pattern
- `handle_program_host_requirements`: uses `.unwrap()` on `serde_json::to_value()` — could panic on non-serializable types

### Test Coverage: **None**

- **0 unit tests** in the web channel crate
- **No test files** under `channels/oxios-web/tests/`
- No HTTP handler tests (would benefit from `axum::test` helpers)

### TODO/FIXME/unwrap/panic: **1 concern**

- **`routes.rs:1381`** — `serde_json::to_value(&check).unwrap()` in `handle_program_host_requirements`
- 8 `#[allow(dead_code)]` annotations on MCP handler structs/functions (reserved, acceptable)

### Integration Quality: **Good**

- `WebChannel` properly implements `Channel` trait with mpsc + broadcast bridging
- `WebChannelHandle::send_and_wait()` uses oneshot correlation for request-response matching
- `AppState` is comprehensive but has 17 fields — consider grouping related state
- `WebServer::serve()` duplicates router setup that `main.rs` also does separately
- **Bug: Duplicate route registration** — `/api/events` is registered twice in `build_routes()` (lines 110 and 128)

---

## 4. oxios-web/frontend (Dioxus)

### Files

| File | Lines | Purpose |
|------|-------|---------|
| `main.rs` | 20 | App entry point, Panel context |
| `api/mod.rs` | 573 | API types + fetch helpers |
| `components/mod.rs` | 3 | Module re-exports |
| `components/layout.rs` | 35 | AppLayout with Panel routing |
| `components/sidebar.rs` | 85 | Sidebar navigation (17 panels) |
| `components/chat.rs` | 64 | ChatInput, ChatMessage, ProcessingIndicator |
| `views/mod.rs` | 21 | View module declarations |
| `views/chat.rs` | 77 | Chat view with session tracking |
| `views/dashboard.rs` | 94 | Dashboard with stat cards + agent table |
| `views/agents.rs` | 92 | Agent list with kill buttons |
| `views/seeds.rs` | 94 | Seed cards with detail view |
| `views/workspace.rs` | 172 | File tree browser with navigation |
| `views/gardens.rs` | 115 | Garden management (create/start/stop/remove/exec) |
| `views/skills.rs` | 54 | Skill list with detail view |
| `views/programs.rs` | 86 | Program list with install/uninstall/enable/disable |
| `views/memory.rs` | 96 | Memory entries list with detail |
| `views/scheduler.rs` | 135 | Scheduler stats + task queue display |
| `views/security.rs` | 75 | Audit log display |
| `views/approvals.rs` | 117 | Approval list with approve/reject actions |
| `views/config.rs` | 49 | Config viewer (read-only) |
| `views/events.rs` | 128 | SSE event stream viewer |
| `views/personas.rs` | 111 | Persona CRUD + active selection |
| `views/host_tools.rs` | 87 | Host tool status display |
| `views/protocol.rs` | 95 | Ouroboros phase visualization |
| `views/placeholder.rs` | 15 | Empty placeholder view |

**Total: 2,514 lines across 24 files**

### Implementation Status

**Fully Implemented:**
- Complete Dioxus SPA with 17 navigation panels
- API client layer with typed fetch helpers (GET/POST/PUT/DELETE)
- 50+ API type definitions matching backend responses
- Chat with session continuity
- Dashboard with stat cards
- Agent monitor with kill action
- Seed browser with detail view
- Workspace file browser with directory navigation + file viewing
- Garden lifecycle management
- Skill/Program/Memory browsing
- Scheduler stats + task queue
- Security audit log
- HitL approval management
- Persona CRUD + active selection
- SSE event viewer

**All views are functional** — no placeholders remain (the `placeholder.rs` exists but is unused in navigation).

### Error Handling Quality: **Fair**

- API helpers return `Result<T, String>` — stringly-typed errors
- Views show error boxes for failed API calls
- Uses `unwrap_or_default()` and `unwrap_or_else()` appropriately in UI rendering
- 4 instances of `unwrap` in frontend views (all safe variants for display purposes)

### Test Coverage: **None**

- No tests in the Dioxus frontend (common for UI code)
- Would benefit from API type deserialization tests

### Integration Quality: **Good**

- API types are carefully aligned with backend response shapes
- Some `#[allow(dead_code)]` on fields kept "for backward compat" — minor tech debt
- 50+ API type definitions in `api/mod.rs` is a large file — consider splitting
- Uses `gloo-net` for HTTP (appropriate for WASM target)
- Dioxus signals used correctly for state management

---

## 5. Main Binary

### File: `src/main.rs` — **995 lines**

### Implementation Status

**Fully Implemented:**
- CLI with `clap`: `run`, `garden`, `status`, `config`, `pkg` subcommands
- `init_kernel()` — complete kernel wiring (28 lines of return type!)
- MCP bridge initialization from config + environment variables
- `oxios garden` subcommands: new, up, down, remove, list, exec
- `oxios pkg` subcommands: install (git/tarball/local), uninstall, list, search
- `oxios run` — single prompt execution
- `oxios status` — comprehensive system status
- `oxios config` — show/get (set not implemented)
- Interactive mode: web server + gateway loop + graceful shutdown
- Port availability checking with `lsof` process identification
- Workspace initialization with default config/skills/programs

**Notable Issues:**
1. **Duplicate persona_manager creation** (lines 255 and 332) — Two `PersonaManager::new()` calls in `init_kernel()`. The first is used for wiring (persona prompt set on ouroboros), then overwritten by a second empty instance that's returned. **Bug: The persona_manager returned from `init_kernel()` has no personas.**
2. **Duplicate a2a_protocol creation** (lines 264 and 267) — `A2AProtocol::new(event_bus.clone())` called twice, second shadows the first. Same event bus so functionally equivalent but wasteful.
3. **Config `set` not implemented** — `ConfigAction::Set` always bails with "Edit ~/.oxios/config.toml directly."
4. **Default model hardcoded** — `anthropic/claude-sonnet-4-20250514` appears in 3 places
5. **WebServer created but not used for serve** — `_web_server` is created but the actual serving is done inline in `main()` with a manually constructed router, duplicating `WebServer::serve()` logic

### Error Handling Quality: **Good**

- `init_kernel()` uses `.context()` for descriptive errors
- `ensure_workspace()` properly creates directories
- Garden commands check `is_backend_available()` before container operations
- Port check provides helpful error message with instructions
- `expand_path()` handles tilde expansion
- `cmd_garden exec` properly exits with container exit code

### Test Coverage: **None**

- No tests in the binary crate (expected for entry points)

### TODO/FIXME/unwrap/panic: **2 concerns**

- **Line 720**: `signal(SignalKind::terminate()).unwrap()` — could panic in unusual environments
- **Line 962**: `#[allow(unused_variables)]` on `check_port_occupant` parameters

### Integration Quality: **Fair**

- `init_kernel()` is a 100+ line initialization function with a 16-element return tuple — needs refactoring
- State is threaded through manually rather than using a builder pattern
- Gateway loop spawned as a detached task (no join on shutdown)
- Default skill/program installation done inline in main rather than in kernel init
- MCP initialization split between `init_kernel()` and `main()`

---

## 6. Integration Tests

### File: `crates/oxios-kernel/tests/integration_tests.rs` — **1,090 lines**

### Test Inventory

| Test | Lines (approx) | What It Tests |
|------|----------------|---------------|
| `test_event_bus_publish_subscribe` | 15 | EventBus basic publish/receive |
| `test_event_bus_multiple_subscribers` | 15 | EventBus fan-out |
| `test_event_bus_no_subscribers_ok` | 8 | EventBus graceful no-receiver |
| `test_state_store_save_load_markdown` | 12 | StateStore markdown persistence |
| `test_state_store_load_nonexistent` | 8 | StateStore None on missing |
| `test_state_store_list_category` | 14 | StateStore listing |
| `test_state_store_save_load_json` | 12 | StateStore JSON persistence |
| `test_state_store_path_traversal_blocked` | 16 | StateStore security |
| `test_orchestrator_happy_path` | 35 | Full Ouroboros lifecycle |
| `test_orchestrator_evolution_loop` | 30 | Evolve → re-execute cycle |
| `test_orchestrator_events_published` | 45 | Phase events during orchestration |
| `test_gateway_routes_message_through_orchestrator` | 30 | Gateway → Orchestrator routing |
| `test_gateway_unknown_channel` | 25 | Gateway graceful unknown channel |
| `test_scheduler_orchestrator_integration` | 30 | Scheduler + Orchestrator coexistence |
| `test_scheduler_priority_ordering_in_orchestration` | 35 | Priority queue ordering |
| `test_program_install_then_orchestrate` | 60 | Program lifecycle + orchestration |
| `test_program_manager_all_tool_schemas` | 40 | Multi-program tool aggregation |
| `test_access_manager_blocks_dangerous_tools` | 20 | Tool permission enforcement |
| `test_access_manager_enforces_path_restrictions` | 25 | Path glob permission enforcement |
| `test_access_manager_audit_log_on_denied_access` | 20 | Audit logging on denied ops |
| `test_access_manager_network_and_fork_permissions` | 35 | Network/fork/time/memory limits |
| `test_access_manager_permission_lifecycle` | 25 | Create → use → remove permissions |

**22 tests total**

### Test Infrastructure Quality

- **Mock implementations**: `MockOuroboros`, `MockSupervisor`, `MockChannel`, `SchedulerAwareSupervisor`
- Mocks use `AtomicUsize`/`AtomicBool` for call tracking (thread-safe, no async locks)
- `MockOuroboros` supports configurable evaluation pass/fail for evolution testing
- Tests use `tempfile::tempdir()` for isolation
- Tests cover the critical integration path: Gateway → Orchestrator → Ouroboros → Supervisor

### Gaps

- No tests for `WebChannel` (mpsc/broadcast bridging)
- No HTTP route handler tests
- No WebSocket chat tests
- No SSE event streaming tests
- No tests for `OuroborosEngine` (real LLM engine)
- No tests for workspace file operations through API
- No tests for config persistence (PUT /api/config)
- No tests for persona CRUD
- No tests for MCP bridge
- No test for the `init_kernel()` wiring

---

## 7. Documentation Files

### README.md — 516 lines

**Quality: Excellent**
- Architecture diagram with ASCII art
- Quick start guide (build → configure → run)
- Complete CLI command reference
- Full API reference with request/response shapes
- Configuration reference with types and defaults
- Environment variables documented
- Development guide (build/test/lint)
- Project structure overview
- Dependency map

### DESIGN.md — 673 lines

**Quality: Excellent**
- Philosophy section (Unix + Ouroboros principles)
- Unix ↔ Oxios mapping table
- Detailed component descriptions with code examples
- AIOS-inspired extensions documented (Scheduler, ContextManager, AccessManager)
- Program system with toml format examples
- Container minimalism philosophy
- Build order roadmap with phase completion markers
- All phases marked as complete (✓)

### Cargo.toml — 65 lines

**Quality: Good**
- Workspace members correctly declared
- Workspace dependencies properly factored
- Path dependencies to oxi crates
- Version 0.2.0-alpha

---

## 8. Cross-Cutting Issues

### Critical

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 1 | **Duplicate persona_manager** — `init_kernel()` creates two instances; the returned one is empty | `main.rs:255,332` | Active persona set on Ouroboros but returned persona_manager has no personas → web API personas empty |
| 2 | **Duplicate a2a_protocol** — created twice, first is dropped | `main.rs:264,267` | Minor memory waste, confusing code |
| 3 | **Duplicate route registration** — `/api/events` registered twice in `build_routes()` | `routes.rs:110,128` | May cause runtime error or unpredictable behavior with axum |

### High

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 4 | `routes.rs` is 1,798 lines — too large | `routes.rs` | Maintainability concern |
| 5 | `init_kernel()` returns 16-element tuple | `main.rs:244-337` | Extremely hard to maintain, error-prone |
| 6 | WebServer created but not used for serving | `main.rs` | Duplicated router setup |
| 7 | `handle_program_host_requirements` uses `.unwrap()` | `routes.rs:1381` | Could panic on serialization failure |
| 8 | No unit tests in ouroboros, gateway, or web crates | All crates | Untested edge cases |

### Medium

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 9 | `Gateway::run()` uses polling loop (100ms) | `gateway.rs:109-132` | Latency, unnecessary wakeups |
| 10 | `execute()` is a stub in OuroborosEngine | `ouroboros_engine.rs:271-281` | Protocol incomplete by design |
| 11 | Consensus evaluation always `None` | `ouroboros_engine.rs:316` | 2/3 evaluation stages implemented |
| 12 | `ConfigAction::Set` not implemented | `main.rs` | Users must edit config file manually |
| 13 | Default model ID hardcoded in 3 places | `main.rs` | Should be configurable |
| 14 | MCP routes commented out (handlers exist) | `routes.rs` | Dead code |
| 15 | `api/mod.rs` is 573 lines of type definitions | `frontend/src/api/mod.rs` | Should be split |

### Low

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 16 | `handle_status` returns `uptime: "n/a"` | `routes.rs` | Cosmetic |
| 17 | Several `Err(_) => Json(Vec::new())` patterns | `routes.rs` | Silent error swallowing |
| 18 | Frontend `api/mod.rs` has many `#[allow(dead_code)]` types | `frontend/src/api/mod.rs` | "Backward compat" types that may be unused |
| 19 | `shutdown_signal()` uses `.expect()` for signal handlers | `server.rs` | Could panic in constrained environments |

---

## 9. Summary Scorecard

| Component | Lines | Implemented | Error Handling | Tests | Integration | Overall |
|-----------|-------|-------------|----------------|-------|-------------|---------|
| **oxios-ouroboros** | 852 | 90% (execute stub, consensus skipped) | ⭐⭐⭐⭐ | ⭐ (doc tests only) | ⭐⭐⭐⭐⭐ | **B+** |
| **oxios-gateway** | 288 | 100% | ⭐⭐⭐⭐ | ⭐ (none) | ⭐⭐⭐⭐ | **B** |
| **oxios-web** | 2,397 | 95% (MCP reserved) | ⭐⭐⭐ | ⭐ (none) | ⭐⭐⭐⭐ | **B-** |
| **oxios-web/frontend** | 2,514 | 100% | ⭐⭐⭐ | ⭐ (none, expected) | ⭐⭐⭐⭐ | **B** |
| **main binary** | 995 | 95% (config set missing) | ⭐⭐⭐⭐ | ⭐ (none, expected) | ⭐⭐⭐ | **B-** |
| **integration tests** | 1,090 | N/A | N/A | 22 tests | ⭐⭐⭐⭐ | **B+** |
| **documentation** | 1,254 | 100% | N/A | N/A | ⭐⭐⭐⭐⭐ | **A** |

### Top 3 Priorities

1. **Fix the duplicate `persona_manager` bug** — The returned persona_manager is empty, breaking the web API's persona endpoints
2. **Fix the duplicate `/api/events` route registration** — Could cause runtime issues
3. **Add unit tests to ouroboros and gateway crates** — These are core protocol components with zero in-crate tests

### Codebase Health

- **Total lines analyzed**: ~9,390 (excluding docs)
- **TODO/FIXME count**: 0
- **Unsafe `unwrap()` count**: 2 (1 in routes.rs, 1 in main.rs)
- **Test coverage**: 22 integration tests + 2 doc tests = **24 total tests**
- **Documentation quality**: Excellent (README + DESIGN + AGENTS.md)
- **Dead code**: MCP handlers (8 `#[allow(dead_code)]`), legacy compat types in frontend
