# Oxios Channels, Gateway, Tools & Tests — Detailed Analysis Report

> Generated: 2026-05-14
> Total files analyzed: 12
> Total lines reviewed: ~5,523

---

## Executive Summary

The analyzed layer spans the full vertical from user-facing HTTP routes down to kernel-level tools, protocol implementations, and test suites. Overall code quality is **high**: thorough documentation, consistent patterns, strong security posture, and comprehensive test coverage. A few architectural concerns and minor issues are noted below.

| Area | Rating | Summary |
|------|--------|---------|
| Gateway | ⭐⭐⭐⭐ | Clean plugin architecture, minimal but well-structured |
| Ouroboros | ⭐⭐⭐⭐ | Strong spec-first protocol, clear phase definitions |
| Web Channel | ⭐⭐⭐⭐ | Good path traversal protection, feature-rich routes |
| ExecTool | ⭐⭐⭐⭐⭐ | Best-in-class security, excellent test coverage |
| McpTool | ⭐⭐⭐⭐ | Clean wrapper, minimal but sufficient |
| Program System | ⭐⭐⭐⭐⭐ | Robust lifecycle management, extensive tests |
| MCP Client | ⭐⭐⭐⭐ | Solid JSON-RPC implementation, good lifecycle management |
| A2A Protocol | ⭐⭐⭐⭐ | Well-designed agent discovery and messaging |
| Integration Tests | ⭐⭐⭐⭐⭐ | Comprehensive mock-based integration, covers all paths |
| E2E Tests | ⭐⭐⭐⭐ | Thorough subsystem testing, good cross-cutting coverage |
| KernelHandle | ⭐⭐⭐⭐⭐ | Elegant facade pattern, clean API decomposition |

---

## 1. Gateway (`oxios-gateway/src/lib.rs`) — 16 LOC

### Key Structs/Traits/Functions
- **`Channel`** — trait (pub re-export from `channel` module)
- **`Gateway`** — main router struct (pub re-export)
- **`IncomingMessage` / `OutgoingMessage`** — message types
- **`ChannelBundle` / `ChannelContext` / `ChannelPlugin`** — plugin system types

### Code Quality Observations
- **Excellent**: Module is a pure re-export facade — zero logic, zero risk. Classic lib.rs pattern.
- `#![warn(missing_docs)]` enforced at crate level.
- Module-level doc comment clearly describes the purpose and Channel trait relationship.

### API Design Quality
- Clean separation: the gateway crate only defines the abstraction, not the implementation.
- Plugin system via `ChannelPlugin` suggests a compile-time plugin model.

### Test Coverage
- No tests in this file (expected — it's a re-export facade).
- Gateway routing is tested in integration tests (`test_gateway_routes_message_through_orchestrator`).

### Issues/Concerns
- **None**. This is a textbook crate root.

---

## 2. Ouroboros Protocol (`oxios-ouroboros/src/lib.rs`) — 21 LOC

### Key Structs/Traits/Functions
- **`OuroborosEngine`** — the main engine
- **`OuroborosProtocol`** — trait for the 5-phase lifecycle
- **`Phase`** — enum (interview → seed → execute → evaluate → evolve)
- **`Seed`** — spec artifact with constraints, acceptance criteria, ontology
- **`AmbiguityScore`** / **`Entity`** — interview types
- **`EvaluationResult`** / **`InterviewResult`** / **`ExecutionResult`** — phase results
- **`eval_cache`** — evaluation caching module

### Code Quality Observations
- Doc comment states the key invariant: *"Never execute without a spec. Clarify until ambiguity ≤ 0.2."*
- Phase ordering is clearly communicated through re-exports.
- `eval_cache` module is interesting — suggests performance optimization for repeated evaluations.

### API Design Quality
- The `OuroborosProtocol` trait is well-designed: each phase maps to one method.
- Result types are re-exported at crate root for easy consumption.
- The `Seed` type with `parent_seed_id` and `generation` fields supports evolutionary lineage tracking.

### Test Coverage
- Thoroughly tested via `MockOuroboros` in integration tests (happy path, evolution loop, event publishing).
- The mock covers all 5 phases with call-counting atomics.

### Issues/Concerns
- **Minor**: `ExecutionResult` is exported from `protocol` module but also used by the Supervisor trait — coupling between ouroboros and kernel types is worth monitoring.

---

## 3. Web Channel (`oxios-web/src/lib.rs`) — 20 LOC

### Key Structs/Traits/Functions
- **`WebChannel` / `WebChannelHandle`** — channel implementation
- **`WebPlugin`** — gateway plugin adapter
- **`AppState`** — shared application state (kernel handle + config)
- **Modules**: `api_docs`, `channel`, `error`, `middleware`, `persona_routes`, `plugin`, `routes`, `server`

### Code Quality Observations
- Clean module organization. Each concern gets its own module.
- `#![warn(missing_docs)]` enforced.
- Doc comment clearly states this implements the `Channel` trait.

### API Design Quality
- Separation of `WebChannel` (trait impl) from `WebPlugin` (gateway adapter) is good.
- `AppState` as the shared state container with `Arc<AppState>` pattern is idiomatic axum.

### Test Coverage
- Workspace routes tested indirectly through workspace.rs handler structure.
- No dedicated unit tests visible in this lib.rs (expected).

### Issues/Concerns
- **None** at this level. The real substance is in routes and server modules.

---

## 4. Web Routes — Workspace (`oxios-web/src/routes/workspace.rs`) — 685 LOC

### Key Structs/Traits/Functions

**Workspace endpoints:**
- `handle_workspace_tree` — GET /api/workspace/tree (file listing)
- `handle_workspace_file_get` — GET /api/workspace/file/*path
- `handle_workspace_file_put` — PUT /api/workspace/file/*path

**Seed endpoints:**
- `handle_seeds_list` — GET /api/seeds
- `handle_seed_get` — GET /api/seeds/:id
- `handle_seed_evolution` — GET /api/seeds/:id/evolution

**Skill endpoints:**
- `handle_skills_list` — GET /api/skills
- `handle_skill_get` — GET /api/skills/:name
- `handle_skill_create` — POST /api/skills
- `handle_skill_delete` — DELETE /api/skills/:name

**Memory endpoints:**
- `handle_memory_list` — GET /api/memory
- `handle_memory_get` — GET /api/memory/:name
- `handle_memory_create` — POST /api/memory
- `handle_memory_search` — POST /api/memory/search
- `handle_memory_semantic_search` — POST /api/memory/semantic

**Helper types:** `TreeQuery`, `TreeEntry`, `SeedSummary`, `EvolutionEntry`, `SkillSummary`, `MemorySummary`, `MemoryCreateRequest`, `MemorySearchRequest`, `SemanticSearchRequest`

### Code Quality Observations
- **Security**: Every file-access endpoint performs canonical path validation with `canonicalize()` + `starts_with()` to prevent path traversal. This is done correctly and consistently.
- **Consistent error handling**: All handlers return `Result<_, AppError>` with descriptive error types (`NotFound`, `Forbidden`, `PayloadTooLarge`, `Internal`, `BadRequest`).
- **Pagination**: `paginate()` helper is used consistently across list endpoints.
- **MIME guessing**: `guess_mime()` function handles common types gracefully with sensible defaults.
- **File size limits**: Enforced (1MB for workspace files, 64KB for skills, 32KB for memory).
- **Evolution lineage**: `build_lineage_iterative` uses a work-stack approach to avoid stack overflow on deep lineage chains — good defensive programming.

### API Design Quality
- RESTful design: resources map cleanly to URLs, verbs are correct.
- Query parameter extraction via `Query<T>` and path extraction via `Path<T>` — idiomatic axum.
- Request bodies use strongly-typed structs with serde deserialization.
- Memory type validation on create (`fact`, `episode`, `knowledge`) with clear error message.

### Test Coverage
- **Gap**: No visible unit tests for these handlers in this file. Handler logic is moderately complex (especially evolution lineage). This is a risk area.
- Integration testing likely covers some paths via the full stack.

### Issues/Concerns
- **Medium**: `handle_seed_evolution` has a nested `fn build_lineage_iterative` with a `Pin<Box<dyn Future>>` return — this is necessary for recursion but makes the code harder to follow. Consider extracting to a standalone function or method.
- **Low**: `handle_memory_search` and `handle_memory_semantic_search` have duplicated type-filter parsing logic. Could be extracted to a helper.
- **Low**: `handle_seeds_list` silently skips malformed seeds — this is probably intentional (graceful degradation) but could log a warning.
- **Low**: Memory type filter mapping has `"conversation"` → `MemoryType::Conversation` and `"session"` → `MemoryType::Session` in `handle_memory_search` but NOT in `handle_memory_create` — inconsistent accepted types between create and search.

---

## 5. ExecTool (`oxios-kernel/src/tools/exec_tool.rs`) — 874 LOC

### Key Structs/Traits/Functions
- **`ExecTool`** — unified execution tool implementing `AgentTool`
- **`ExecResult`** — stdout, stderr, exit_code, duration_ms
- **`shell_exec()`** — raw bash -c execution
- **`structured_exec()`** — binary+args with allowlist enforcement
- **`has_metacharacters()`** — argument validation helper
- **`format_exec_output()`** — human-readable result formatting
- **`SHELL_METACHARS`** — const blocklist of dangerous characters
- **`AgentTool` impl** — `name()`, `label()`, `description()`, `parameters_schema()`, `execute()`

### Code Quality Observations
- **Excellent documentation**: Module-level doc explains both modes, security model, and the relationship with `AccessManager`.
- **Security is first-class**:
  - Shell metacharacter blocklist is comprehensive: `|`, `&`, `;`, `$`, `` ` ``, `<`, `>`, `()`, `{}`, `\n`, `\r`, `\0`.
  - Path traversal (`..`) blocked in both binary name and arguments.
  - Structured mode requires bare binary name (no `/`).
  - Allowlist enforcement with dev-mode escape hatch (empty allowlist).
  - Timeout clamped to `max_timeout_secs` to prevent runaway processes.
  - Environment is stripped to minimal set (`HOME`, `USER`, `LOGNAME`, `PATH`, `LANG`, `TERM`).
- **Access control**: `for_agent()` binds the tool to a specific agent; `new()` (agent_name=None) bypasses checks for test/dev mode.
- **Logging**: Audit-friendly with command preview (truncated to 200 chars).

### API Design Quality
- Single tool, two modes dispatched by `mode` parameter — clean AgentTool interface.
- JSON schema correctly declares required fields conditionally per mode.
- `format_exec_output()` produces clear, human-readable output with timing information (handles both seconds and minutes formatting).

### Test Coverage — ⭐⭐⭐⭐⭐ Outstanding (45+ tests)
| Category | Tests | Notes |
|----------|-------|-------|
| shell_exec | 5 | echo, pipeline, nonzero exit, empty cmd, timeout |
| structured_exec | 6 | echo, blocked binary, path binary, traversal, metachar args, clean args |
| AgentTool interface | 7 | shell mode, structured mode, missing/invalid mode, missing params, nonzero exit |
| format_exec_output | 4 | success, failure, no output, minutes formatting |
| has_metacharacters | 5 | clean, semicolon, pipe, dollar, backtick, traversal |
| Access control | 6 | allowed/denied structured, allowed/denied shell, bypass, agent name |
| **Total** | **~33+** | |

### Issues/Concerns
- **Minor**: Both `shell_exec` and `structured_exec` duplicate the environment setup (`env_clear()` + 6 `.env()` calls). Could be extracted to a helper method.
- **Minor**: The `signal` parameter in `AgentTool::execute()` is received but ignored. Future work to support cancellation.
- **Design note**: `shell_exec` relies entirely on upstream `AccessManager` for sandboxing since "cannot sandbox arbitrary shell" — this is honest and correct, but worth documenting which agent names get bash access.

---

## 6. MCP Tool Wrapper (`oxios-kernel/src/tools/mcp_tool.rs`) — 177 LOC

### Key Structs/Traits/Functions
- **`McpToolWrapper`** — wraps an MCP server tool as an `AgentTool`
- **`format_content_block()`** — formats `McpContentBlock` (Text, Image, Resource) for display
- **`AgentTool` impl** — delegates to `McpBridge::call_tool()`

### Code Quality Observations
- Clean delegation pattern: wrapper holds `Arc<McpBridge>` and routes calls.
- Namespacing via `mcp:{server_name}:{tool_name}` prevents collisions — good design.
- Error handling: MCP-level errors (is_error flag) and transport errors both produce `AgentToolResult::error()`.
- `Debug` impl is provided and shows only `full_name`.

### API Design Quality
- Follows the `AgentTool` interface cleanly.
- `label()` returns the short tool name (without server prefix) — correct for display purposes.
- `parameters_schema()` delegates to the MCP server's schema — zero duplication.

### Test Coverage
- **2 tests**: `test_tool_wrapper_debug` and `test_name_format`.
- **Gap**: No integration test that actually executes a tool through the wrapper (would need a mock McpBridge).

### Issues/Concerns
- **Medium**: No rate limiting or timeout on MCP tool calls. A misbehaving MCP server could block an agent indefinitely. The `McpClient` has a configurable timeout, but the wrapper doesn't set one.
- **Low**: Image content blocks display byte count but not the image — acceptable for a text-based agent interface.

---

## 7. Program System (`oxios-kernel/src/program/mod.rs`) — 1,282 LOC

### Key Structs/Traits/Functions
- **`ProgramManager`** — manages program lifecycle (install, uninstall, enable/disable, upgrade)
- **`Program` / `ProgramMeta` / `ToolDef` / `ArgumentDef`** — type definitions (in `types` module)
- **`ProgramState`** — persistent enabled/disabled state
- **`HostRequirementsCheck`** — result of checking host dependencies
- **`compare_versions()`** — SemVer comparison (handles `v` prefix, missing components)
- **`copy_dir_all()`** — recursive directory copy
- **`bootstrap_defaults()`** — auto-installs programs from `.programs/` directory
- **`install_from_git()` / `install_from_tarball()`** — remote installation sources

### Code Quality Observations
- **Comprehensive lifecycle**: install → list → get → enable/disable → upgrade → uninstall.
- **Upgrade logic is sophisticated**: preserves enabled state across upgrades, handles same-version no-op, warns on downgrade.
- **Bootstrap mechanism**: Copies programs from `.programs/` in the repo root to the runtime directory — enables zero-config defaults.
- **State persistence**: `state.json` in each program directory tracks enabled state, survives restarts.
- **Version comparison**: Handles `v` prefix, missing components (e.g., `"1.0"` vs `"1.0.0"`).
- **Installation sources**: Local, git clone (with `--depth 1`), tarball (curl + tar) — flexible.

### API Design Quality
- `install_from()` enum dispatch pattern is clean.
- `RwLock<HashMap>` for in-memory cache is appropriate for read-heavy workloads.
- `check_host_requirements()` validates both required and optional tools.

### Test Coverage — ⭐⭐⭐⭐⭐ Outstanding (30+ tests)
| Category | Tests | Notes |
|----------|-------|-------|
| ProgramMeta parsing | 7 | minimal, tools+args, dependencies, missing file, empty sections, requires_tools |
| ProgramManager CRUD | 10 | init, list empty, get nonexistent, install, duplicate, uninstall, set_enabled, all_tool_schemas, get_skill_content, check_host_requirements |
| State persistence | 4 | state.json created, set_enabled persists, enabled survives reload |
| Version comparison | 5 | equal, newer, older, v-prefix, missing components |
| Upgrade | 4 | same version noop, newer version, preserves enabled state, installs if not present |
| Infrastructure | 1 | copy_dir_all |
| **Total** | **~31+** | |

### Issues/Concerns
- **Medium**: `install_from_git()` and `install_from_tarball()` use `tokio::process::Command` to run `git`/`curl`/`tar` on the host without sandboxing or validation of the URL. A malicious URL could be a security risk if program sources are user-supplied.
- **Medium**: The `upgrade()` method is not atomic — it uninstalls first, then installs. If the install fails after uninstall, the program is lost. Consider installing to a temp location first, then swapping.
- **Low**: `bootstrap_defaults()` uses `CARGO_MANIFEST_DIR` at compile time — this is correct for development but may not work in packaged distributions.
- **Low**: `fs::read_dir` ordering is non-deterministic — program list order may vary between runs.

---

## 8. MCP Client (`oxios-kernel/src/mcp/client.rs`) — 353 LOC

### Key Structs/Traits/Functions
- **`McpClient`** — manages a single MCP server process lifecycle over stdio JSON-RPC
- **`initialize()`** — spawns process, sends initialize request, sends initialized notification
- **`send_request()` / `do_request()`** — JSON-RPC request/response over persistent I/O handles
- **`send_notification()`** — fire-and-forget JSON-RPC notification
- **`list_tools()` / `refresh_tools()`** — tool discovery with caching
- **`call_tool()` / `call_tool_text()`** — tool invocation
- **`shutdown()` / `restart()`** — process lifecycle management

### Code Quality Observations
- **Persistent I/O handles**: stdin/stdout stored separately from child process — enables multiple requests on the same connection without consuming handles via `take()`.
- **Request serialization**: Write locks on both stdin and stdout ensure correct ordering of concurrent requests.
- **Timeout handling**: Configurable request timeout via `with_timeout()`, applied to both write and read phases independently.
- **JSON-RPC compliance**: Sends `notifications/initialized` after the handshake (required by MCP spec).
- **ID mismatch warning**: Logs a warning if response ID doesn't match request ID — defensive but non-blocking.

### API Design Quality
- Builder pattern for timeout: `McpClient::new(config).with_timeout(Duration::from_secs(60))`.
- `call_tool_text()` convenience method returns first text block — useful for simple tools.
- Tool caching with explicit `refresh_tools()` — good balance of performance and freshness.

### Test Coverage
- **Gap**: No tests in this file. The client requires a real or mock subprocess to test, which is challenging.
- The `McpBridge` and `McpToolWrapper` have tests, but the client itself is untested.

### Issues/Concerns
- **Medium**: `do_request()` acquires write locks on stdin and stdout sequentially, not atomically. Under high concurrency, two concurrent requests could interleave (request A writes, request B writes, request A reads B's response). The per-request write lock on stdin + write lock on stdout within the same `do_request` call should serialize correctly, but it's worth verifying with a stress test.
- **Medium**: No reconnection logic. If the MCP server crashes, all subsequent calls will fail. Consider auto-restart on communication errors.
- **Low**: `stderr` is set to `Stdio::null()` — MCP server error output is lost. Consider capturing or logging stderr for debugging.
- **Low**: Tool cache is never invalidated except by explicit `refresh_tools()` — if the MCP server adds/removes tools dynamically, the cache will be stale.

---

## 9. A2A Protocol (`oxios-kernel/src/a2a.rs`) — 870 LOC

### Key Structs/Traits/Functions
- **`A2AMessage`** — tagged enum: TaskDelegation, StatusUpdate, ResultSharing, CapabilityQuery, Handshake
- **`A2AProtocol`** — protocol handler with registry, queues, and delegation handler
- **`AgentCardRegistry`** — capability-based agent discovery
- **`AgentCard`** — agent capability description (builder pattern)
- **`AgentQueue`** — per-agent message queue with `Notify`-based async notification
- **`A2ARequest` / `A2AResponse`** — request/response envelopes
- **`TaskSpec`** — structured task specification with priority and deadline
- **`DelegationHandler`** — callback type for task delegation execution
- **`send_and_wait()`** — send message with timeout-based response matching

### Code Quality Observations
- **Clean separation of concerns**: Registry (discovery), Queue (messaging), Handler (execution).
- **`AgentCard` builder pattern** is ergonomic: `AgentCard::new(id, name, desc).with_capability("x").with_skill("y")`.
- **Queue design is clever**: `parking_lot::Mutex` for cheap push/drain + `tokio::sync::Notify` for async wake-up — avoids polling.
- **`send_and_wait()`** is sophisticated: uses `tokio::select!` with `Notify` for efficient response waiting, matches by task_id for delegation or by request_id for other messages.
- **Event bus integration**: Registry changes publish `KernelEvent::AgentCreated` / `AgentStopped`.
- **Priority levels**: Low → Normal → High → Critical, with `Default` = Normal.

### API Design Quality
- `delegate_task()`, `send_status_update()`, `share_result()`, `query_capabilities()`, `send_handshake()` — each maps to a clear A2A message type.
- `execute_delegation()` is separate from `delegate_task()` — the former runs the handler, the latter just enqueues. Good separation.
- `receive_messages()` drains the queue atomically — no partial reads.

### Test Coverage
- **5 tests**: agent card creation, registry register/unregister, find by capability, send/receive, delegate task.
- **Gap**: No test for `send_and_wait()`, `execute_delegation()`, or the delegation handler. These are the most complex methods.
- **Gap**: No test for `deliver_pending_messages()` or `has_messages()`.

### Issues/Concerns
- **Medium**: `send_and_wait()` holds no lock while waiting, so multiple concurrent `send_and_wait` calls from the same agent could each consume the wrong response. The task_id matching mitigates this, but for non-delegation messages, matching by `request_id` in the payload string is fragile.
- **Medium**: The `DelegationHandler` type alias uses `Arc<dyn Fn(...)>` — this is a global handler, not per-task. All delegations go through the same handler, which limits composability.
- **Low**: `receive_messages()` acquires a read lock on the queues map, then a lock on the individual queue's messages. This is correct but means adding/removing queues is blocked during message delivery.
- **Low**: `AgentQueue` is private — no way for external code to inspect queue state.

---

## 10. Integration Tests (`oxios-kernel/tests/integration_tests.rs`) — 1,142 LOC

### Key Structs/Traits/Functions

**Mock implementations:**
- **`MockOuroboros`** — deterministic mock with call-counting and configurable evaluation pass/fail
- **`MockSupervisor`** — in-memory agent tracking with fork/exec/wait/kill
- **`MockChannel`** — captures outgoing messages, uses mpsc channel for incoming

**Test categories (21 tests):**
| Category | Count | Tests |
|----------|-------|-------|
| EventBus | 3 | publish/subscribe, multiple subscribers, no subscribers |
| StateStore | 5 | save/load markdown, load nonexistent, list category, save/load JSON, path traversal |
| Orchestrator | 3 | happy path, evolution loop, events published |
| Gateway | 2 | route through orchestrator, unknown channel |
| Scheduler | 2 | orchestrator integration, priority ordering |
| Programs | 2 | install then orchestrate, all tool schemas |
| AccessManager | 4 | dangerous tools blocked, path restrictions, audit log, network/fork permissions, lifecycle |
| **Total** | **21** | |

### Code Quality Observations
- **Mock design is excellent**: `MockOuroboros` uses `AtomicBool` to toggle evaluation pass/fail, enabling both happy-path and evolution-loop tests.
- **`SchedulerAwareSupervisor`**: Extends the mock to integrate with the real scheduler — tests the actual scheduling behavior.
- **StateStore path traversal test**: Comprehensive — tests `../`, `\`, empty, leading `/`, trailing `/`, `//`, but allows `foo/bar` (sub-directories).

### API Design Quality
- Tests validate the full stack: Gateway → Orchestrator → Supervisor → EventBus → StateStore.
- Mock implementations are realistic enough to catch integration issues.

### Test Coverage Assessment
- **Strong**: Covers the critical orchestrator lifecycle (happy path, evolution, event publishing).
- **Gap**: No test for concurrent orchestration (multiple messages in flight).
- **Gap**: Gateway test doesn't verify the response content (comment acknowledges this).

### Issues/Concerns
- **Low**: Gateway test creates `MockChannel` but doesn't verify outgoing messages because the channel is moved into `gateway.register()`. Could use `Arc` to share state.
- **Low**: `test_orchestrator_events_published` uses a 5-second deadline — could be slow on CI if the orchestrator is slow.

---

## 11. E2E Kernel Tests (`tests/e2e_kernel.rs`) — 848 LOC

### Key Structs/Traits/Functions

**Test categories (40 tests):**
| Subsystem | Count | Tests |
|-----------|-------|-------|
| StateStore | 5 | save/load JSON, save/load session, list category, delete file, markdown |
| GitLayer | 8 | commit+log, tag operations, verify, restore file, disabled noop, batch commit, remove file |
| AuditTrail | 9 | append generates hash, hash chain, verify chain, verify multiple, entries range, query by agent, export JSON, len, auto-prune |
| BudgetManager | 9 | set+remaining, reserve success, reserve exceed, can schedule, reset window, track call, release tokens, remove budget, no configured |
| ResourceMonitor | 4 | snapshot, set metrics, history, overload threshold, is overloaded |
| Cross-subsystem | 2 | state+git integration, audit+budget integration |
| Direct system calls | 2 | git system calls direct, resource monitor system calls direct |
| **Total** | **~39** | |

### Code Quality Observations
- **Systematic**: Each kernel subsystem gets its own test section with clear delimiters.
- **AuditTrail tests are thorough**: Hash chain integrity, blake3 hex length verification, range queries, agent filtering, JSON export, auto-pruning.
- **BudgetManager tests cover edge cases**: Exhaustion, release, reset, removal, unconfigured agent.
- **GitLayer tests include disabled mode**: Verifies graceful degradation when git is turned off.

### API Design Quality
- Tests follow a consistent pattern: setup → act → assert, making them easy to understand.
- Cross-subsystem tests (`state_and_git_integration`, `audit_and_budget_integration`) validate that subsystems compose correctly.

### Test Coverage Assessment
- **Excellent**: Covers all major kernel subsystems at the API level.
- **Gap**: No test for `KernelHandle` facade methods (e.g., `save_and_commit`, `delete_and_commit`, `flush_audit`).
- **Gap**: No test for cron scheduling or event subscription at this level.

### Issues/Concerns
- **Low**: Some tests are slightly redundant with integration tests (StateStore tests appear in both files). This is acceptable for defense-in-depth.

---

## 12. KernelHandle Facade (`oxios-kernel/src/kernel_handle/mod.rs`) — 235 LOC

### Key Structs/Traits/Functions
- **`KernelHandle`** — main facade composed of 7 domain Facades:
  1. **`StateApi`** — data persistence, sessions
  2. **`AgentApi`** — agent lifecycle, budgets, memory
  3. **`SecurityApi`** — auth, audit trail, RBAC, approvals
  4. **`PersonaApi`** — multi-persona management
  5. **`ExtensionApi`** — programs, skills, host tools
  6. **`McpApi`** — MCP server bridge
  7. **`InfraApi`** — Git, scheduler, cron, resources, events, system

**Convenience methods:**
- `save_and_commit()` — State + Infra cross-facade
- `save_markdown_and_commit()` — State + Infra
- `delete_and_commit()` — State + Infra
- `commit_all()` — State + Infra
- `flush_audit()` — Security + Infra
- `schedule()` / `unschedule()` / `list_schedules()` — Infra cron wrapper
- `load_json()` — State wrapper
- `start_time()` — Infra accessor

### Code Quality Observations
- **Excellent facade pattern**: Each domain is a first-class struct, not just a module. This enables independent testing.
- **`from_subsystems()` is deprecated** with a clear migration note — good API evolution practice.
- **Cross-facade methods are minimal and intentional**: Only `save_and_commit`, `delete_and_commit`, `flush_audit`, and cron wrappers combine facades.
- **Conditional git commits**: `if git.is_enabled()` prevents errors when git layer is disabled.

### API Design Quality
- **Best practice**: Public fields (`pub state`, `pub agents`, etc.) allow direct access to facade methods while convenience methods handle common cross-cutting operations.
- The 7-facade decomposition follows domain-driven design principles.
- Cron convenience wrappers handle UUID parsing errors gracefully.

### Test Coverage
- **Gap**: No direct tests for `KernelHandle` methods. The E2E tests exercise the underlying subsystems but not the facade methods.
- The `save_and_commit()` pattern (save JSON + git commit) should be tested as a unit.

### Issues/Concerns
- **Medium**: `commit_all()` delegates to `StateApi::commit_all()` which takes `&InfraApi` — this creates a dependency from StateApi to InfraApi that could complicate testing.
- **Low**: `schedule()` creates a `CronJob` with a generated ID but doesn't validate the cron expression — invalid expressions will fail silently until execution time.
- **Low**: `unschedule()` catches errors and returns `Ok(false)` — this swallows the error message, making debugging harder.

---

## Cross-Cutting Analysis

### Security Posture
- **Strong**: Path traversal protection in web routes, allowlist enforcement in ExecTool, shell metacharacter blocking, environment stripping, access control via RBAC, audit trail with hash chain integrity.
- **Consistent**: Security checks appear at every layer (web routes, tools, access manager).

### Error Handling
- **Consistent**: `anyhow::Result` for applications, typed errors in web layer (`AppError`).
- **Audit-friendly**: Error paths log before returning, especially in tools and routes.

### Documentation
- **Excellent**: Every file has module-level doc comments. Public types have `///` doc comments.
- `#![warn(missing_docs)]` enforced on all crates.

### Test Coverage Summary
| Component | Unit Tests | Integration Tests | E2E Tests |
|-----------|-----------|-------------------|-----------|
| Gateway | — | ✅ 2 tests | — |
| Ouroboros | — | ✅ via MockOuroboros | — |
| Web Routes | ❌ None | — | — |
| ExecTool | ✅ 33+ tests | — | — |
| McpTool | ✅ 2 tests | — | — |
| Program System | ✅ 31+ tests | ✅ 2 tests | — |
| MCP Client | ❌ None | — | — |
| A2A Protocol | ✅ 5 tests | — | — |
| KernelHandle | ❌ None | — | — |
| StateStore | ✅ | ✅ 5 tests | ✅ 5 tests |
| EventBus | — | ✅ 3 tests | — |
| GitLayer | — | — | ✅ 8 tests |
| AuditTrail | — | — | ✅ 9 tests |
| BudgetManager | — | — | ✅ 9 tests |
| ResourceMonitor | — | — | ✅ 5 tests |

### Top Issues by Priority

1. **[Medium] MCP Client reconnection**: No auto-restart on MCP server crash. A dead server blocks all subsequent tool calls.
2. **[Medium] Program upgrade atomicity**: `uninstall → install` is not atomic. Install failure after uninstall loses the program.
3. **[Medium] A2A `send_and_wait` concurrent safety**: Multiple concurrent waits on the same agent could match wrong responses.
4. **[Medium] Remote program installation security**: `install_from_git/tarball` runs arbitrary `git`/`curl`/`tar` without URL validation or sandboxing.
5. **[Medium] MCP Client no tests**: The JSON-RPC client is entirely untested.
6. **[Medium] Web workspace routes no tests**: 685 LOC of handler logic with zero direct tests.
7. **[Low] Duplicated environment setup in ExecTool**: Could extract to a builder method.
8. **[Low] Duplicated type-filter parsing in workspace.rs**: Memory type filter mapping appears in 3 handlers.
9. **[Low] MCP stderr discarded**: Server error output is sent to `/dev/null`, making debugging harder.
10. **[Low] KernelHandle facade untested**: Cross-facade convenience methods have no dedicated tests.

---

## Recommendations

1. **Add tests for web route handlers** — Mock `AppState` and test path traversal, file size limits, memory type validation, skill CRUD.
2. **Add tests for MCP Client** — Create a mock subprocess or use a test MCP server binary.
3. **Make program upgrades atomic** — Install to temp location, then swap directories.
4. **Add reconnection to MCP Client** — Auto-restart on communication errors.
5. **Extract shared helpers** — Memory type filter parsing, environment setup, etc.
6. **Test `send_and_wait` concurrently** — Verify response matching under concurrent sends.
7. **Add KernelHandle facade tests** — Especially `save_and_commit` and `flush_audit`.
