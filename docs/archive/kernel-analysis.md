# Oxios Kernel Source Analysis

**Date:** 2026-05-06  
**Scope:** `/Volumes/MERCURY/PROJECTS/oxios/crates/oxios-kernel/src/`  
**Total files:** 28  
**Total lines:** 13,026

---

## Executive Summary

The oxios-kernel crate is the core of the Oxios Agent OS. It implements 20 modules covering agent lifecycle, event bus communication, state persistence, container management, scheduling, access control, persona management, A2A inter-agent communication, MCP tool integration, and program management.

**Overall assessment:** The codebase is well-structured with strong documentation, good error handling, and substantial test coverage. Most modules are fully implemented with production-quality code. A few areas have minor unwrap() usage in non-test code and some `#[allow(dead_code)]` annotations that should be cleaned up.

---

## Per-File Analysis

### 1. `lib.rs` — 71 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 71 |
| Implemented | ✅ Complete — module declarations and public re-exports |
| Error handling | N/A (declarative only) |
| Tests | None (not needed) |
| TODO/FIXME/unwrap | None |
| Public API | Complete — exports all major types via `pub use` |
| Quality | Clean, well-organized. Exports are grouped by module with clear comments. |

**Notes:** `#![warn(missing_docs)]` is enabled on the crate. All modules and major types are re-exported for convenient access.

---

### 2. `supervisor.rs` — 200 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 200 |
| Implemented | ✅ Full implementation — `Supervisor` trait + `BasicSupervisor` |
| Error handling | ✅ Good — uses `anyhow::Result`, `bail!` for agent-not-found |
| Tests | None (no `#[cfg(test)]` module) |
| TODO/FIXME/unwrap | `let _ = self.event_bus.publish(...)` silently ignores publish errors (acceptable) |
| Public API | `Supervisor` trait (fork/exec/wait/kill/list) + `BasicSupervisor` struct |
| Quality | Clean async trait implementation. `parking_lot::RwLock` for agents map. |

**Gaps:** No unit tests. The `wait()` method is synchronous (just reads current status) — it doesn't actually await agent completion, which could be misleading.

---

### 3. `agent_runtime.rs` — 671 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 671 |
| Implemented | ✅ Full — 4-tier tool registration, AgentLoop execution, prompt building |
| Error handling | ✅ Good — proper error propagation, context-aware errors |
| Tests | ✅ 4 unit tests (tool validation, system prompt building) |
| TODO/FIXME/unwrap | `#[allow(dead_code)]` on `with_config`. 2 `unwrap()` in `make_placeholder_container_manager` |
| Public API | `AgentRuntime`, `AgentRuntimeConfig` |
| Quality | Excellent. Complex 4-tier tool registration (oxi native → container → program → MCP). Well-documented `spawn_blocking` workaround for `!Send` futures. |

**Gaps:** `make_placeholder_container_manager()` uses `unwrap()` on tempdir and StateStore creation — should propagate errors. The MCP server registration uses `Arc::get_mut` which will fail if the Arc is shared (it's just been created, so this works but is fragile).

---

### 4. `event_bus.rs` — 146 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 146 |
| Implemented | ✅ Complete — broadcast-based event bus with 13 event variants |
| Error handling | ✅ Good — `publish` silently ignores no-subscribers (correct behavior) |
| Tests | None |
| TODO/FIXME/unwrap | None |
| Public API | `EventBus::new/subscribe/publish`, `KernelEvent` enum |
| Quality | Clean and minimal. Uses `tokio::sync::broadcast`. Events are `Serialize + Deserialize`. |

**Gaps:** No unit tests. Could benefit from a `try_subscribe` that returns error info.

---

### 5. `state_store.rs` — 498 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 498 |
| Implemented | ✅ Complete — filesystem-backed markdown/JSON persistence, session management |
| Error handling | ✅ Excellent — path traversal validation, proper `NotFound` handling |
| Tests | ✅ 5 unit tests (session CRUD, list sorting, get-or-create) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `StateStore`, `Session`, `SessionId`, `SessionSummary`, `AgentResponse` |
| Quality | Very good. Path traversal validation on category and name. Proper `async` file I/O. |

---

### 6. `config.rs` — 380 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 380 |
| Implemented | ✅ Complete — TOML-based configuration with sensible defaults |
| Error handling | ✅ Good — `anyhow::Result` on `load_config` |
| Tests | None |
| TODO/FIXME/unwrap | None |
| Public API | `OxiosConfig`, `KernelConfig`, `GatewayConfig`, `ContainerConfig`, `SchedulerConfig`, `ContextConfig`, `SecurityConfig`, `PersonaConfig`, `McpConfig`, `McpServerDef`, `load_config` |
| Quality | Comprehensive configuration with 8 sub-configs. All fields have defaults. Uses serde derive macros. |

**Gaps:** No validation beyond TOML parsing. No unit tests for config loading/parsing.

---

### 7. `orchestrator.rs` — 552 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 552 |
| Implemented | ✅ Full Ouroboros lifecycle — interview → seed → execute → evaluate → evolve |
| Error handling | ✅ Good — `.context()` for error chains, proper propagation |
| Tests | None |
| TODO/FIXME/unwrap | `#[allow(dead_code)]` on `persona_manager`, `#[allow(unused)]` on `InterviewSession` |
| Public API | `Orchestrator::new/handle_message`, `OrchestrationResult` |
| Quality | Complex but well-structured. Handles multi-turn interviews, evolution loops, A2A registration, scheduler integration, and access management in a single `handle_message` flow. |

**Gaps:** No tests. The `handle_message` method is 300+ lines — could benefit from extraction of sub-methods. `InterviewSession` has `#[allow(unused)]` fields. Two `let _ =` on scheduler results that silently discard errors.

---

### 8. `container.rs` — 890 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 890 |
| Implemented | ✅ Full — `ContainerBackend` trait + `AppleBackend` with Apple Container CLI |
| Error handling | ✅ Excellent — `.context()` chains, platform checks, graceful degradation |
| Tests | ✅ 8 unit tests (parsing, status, stats, naming) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `ContainerBackend` trait, `AppleBackend`, `ContainerConfig`, `ContainerStatus`, `ContainerStats`, `ExecResult`, `ContainerWorkspaceInfo` |
| Quality | Excellent. Thorough implementation of Apple Container CLI interaction. JSON and tabular output parsing fallbacks. Platform requirement checking. Workspace mount extraction from inspect output. |

---

### 9. `container_manager.rs` — 457 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 457 |
| Implemented | ✅ Complete — container lifecycle, workspace management, metadata persistence |
| Error handling | ✅ Good — name validation, path checks, `.context()` chains |
| Tests | ✅ 4 unit tests (create, duplicate rejection, bad name, remove) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `ContainerManager`, `ContainerInfo` |
| Quality | Good. Validates container names (alphanumeric/hyphen/underscore). Persists metadata to StateStore. Default Containerfile included. |

---

### 10. `host_exec.rs` — 524 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 524 |
| Implemented | ✅ Complete — UDS relay, direct exec, security validation |
| Error handling | ✅ Excellent — allowlist enforcement, metacharacter blocking, timeout, size limits |
| Tests | ✅ 9 unit tests (command validation, metacharacter detection, echo execution, blocked commands, empty allowlist rejection) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `HostExecBridge`, `HostExecResult`, `RelayRequest`, `RelayResponse`, security validation functions |
| Quality | Excellent security model. Length-prefixed JSON protocol over UDS. Environment variable whitelisting. Timeout enforcement with clamping. |

---

### 11. `host_tools.rs` — 291 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 291 |
| Implemented | ✅ Complete — host tool discovery and validation |
| Error handling | ✅ Good — multiple version-flag probes for tool detection |
| Tests | ✅ 11 unit tests (validation, optional, full check, serialization, constants) |
| TODO/FIXME/unwrap | None |
| Public API | `HostToolValidator`, `HostToolStatus`, `common` module |
| Quality | Clean. Multiple detection strategies (`--version`, `-v`, `version`). Well-documented tool categories. |

---

### 12. `program.rs` — 1,136 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 1,136 (largest file) |
| Implemented | ✅ Complete — TOML parsing, install/uninstall, host requirements, git/tarball install, bootstrap from `.programs/` |
| Error handling | ✅ Good — context chains, duplicate prevention, missing file errors |
| Tests | ✅ 17 unit tests (meta loading, tools, dependencies, install/uninstall, enable/disable, host requirements, copy_dir_all) |
| TODO/FIXME/unwrap | 3 `unwrap()` in production code (git clone first-entry, tarball first-entry, bootstrap file_name) |
| Public API | `ProgramMeta`, `Program`, `ProgramManager`, `InstallSource`, `ToolDef`, `ArgumentDef`, `HostRequirementsCheck`, `McpServerConfig` |
| Quality | Very comprehensive. Handles local, git, and tarball installation sources. Bootstrap from `.programs/` directory. `copy_dir_all` utility. |

**Gaps:** The 3 `unwrap()` calls in `install_from_git`, `install_from_tarball`, and `bootstrap_defaults` could panic on unexpected directory structures.

---

### 13. `skill.rs` — 299 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 299 |
| Implemented | ✅ Complete — frontmatter parsing, CRUD operations, default initialization |
| Error handling | ✅ Good — graceful handling of missing frontmatter |
| Tests | ✅ 3 unit tests (with metadata, no metadata, quoted values) |
| TODO/FIXME/unwrap | None |
| Public API | `Skill`, `SkillMeta`, `SkillStore` |
| Quality | Simple but effective YAML frontmatter parser (manual, no YAML library dependency). `init_defaults` copies from embedded defaults. |

---

### 14. `mcp.rs` — 1,225 lines (second largest)

| Aspect | Assessment |
|--------|-----------|
| Lines | 1,225 |
| Implemented | ✅ Complete — JSON-RPC 2.0 client, stdio transport, tool listing/calling, multi-server bridge |
| Error handling | ✅ Excellent — timeout on reads/writes, spawn error handling, response ID mismatch warnings |
| Tests | ✅ 18 unit tests (serialization, error codes, tool conversion, registration, lifecycle, non-existent commands) |
| TODO/FIXME/unwrap | 1 `#[ignore]` test for echo server |
| Public API | `McpBridge`, `McpClient`, `McpServer`, `McpTool`, `McpCapabilities`, `McpRequest/Response/Error`, JSON-RPC types |
| Quality | Production-quality MCP implementation. Full JSON-RPC 2.0 protocol. Proper initialize handshake with `notifications/initialized`. Tool caching. Per-server client management. |

---

### 15. `scheduler.rs` — 842 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 842 |
| Implemented | ✅ Complete — priority queue, rate limiting, zombie detection, concurrent task management |
| Error handling | ✅ Good — proper error returns for unknown tasks |
| Tests | ✅ 24 unit tests (comprehensive — priority ordering, concurrency limits, zombie reaping, rate limiting, task lifecycle) |
| TODO/FIXME/unwrap | None |
| Public API | `AgentScheduler`, `ScheduledTask`, `Priority`, `TaskStatus`, `SchedulerStats` |
| Quality | Excellent test coverage. Priority queue with correct insertion ordering. Rate limiter with sliding window. Zombie detection with timestamp tracking. `parking_lot::Mutex` for sync. |

**Note:** `stats()` reports `completed: 0, failed: 0` always — these counters are not tracked. A comment acknowledges this could be optimized.

---

### 16. `context_manager.rs` — 719 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 719 |
| Implemented | ✅ Complete — 3-tier context management (active/cache/archive), token budgets, LRU eviction |
| Error handling | ✅ Good — automatic demotion on capacity overflow |
| Tests | ✅ 21 unit tests (store/retrieve, demotion, compression, restore, capacity, token limits, stats) |
| TODO/FIXME/unwrap | None |
| Public API | `ContextManager`, `ContextTier`, `ContextEntry`, `ContextStats` |
| Quality | Well-designed with good test coverage. Token-based capacity enforcement. LRU eviction for cache. Automatic demotion on overflow. |

---

### 17. `access_manager.rs` — 1,676 lines (largest file)

| Aspect | Assessment |
|--------|-----------|
| Lines | 1,676 |
| Implemented | ✅ Complete — RBAC, agent permissions, path sandboxing, garden isolation, HitL approvals, audit logging |
| Error handling | ✅ Excellent — denied-by-default, glob pattern matching, audit log with pruning |
| Tests | ✅ 30+ unit tests (permissions, tool access, path access, network, execution limits, forking, audit log, garden sandbox, RBAC) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `AccessManager`, `AgentPermissions`, `AuditEntry`, `RbacManager`, `RbacPolicy`, `Role`, `Subject`, `Action`, `PendingApproval`, `ApprovalStatus` |
| Quality | Most comprehensive module. Implements full OWASP-inspired security model. 3-tier RBAC (User/Superuser/Admin). Garden sandbox with workspace boundary enforcement. Audit log with configurable max entries. HitL approval system for high-risk actions. |

---

### 18. `a2a.rs` — 699 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 699 |
| Implemented | ✅ Complete — A2A protocol, agent card registry, message queue, capability discovery |
| Error handling | ✅ Good — proper error returns, event bus error propagation |
| Tests | ✅ 5 unit tests (card creation, registry CRUD, capability search, message send/receive, task delegation) |
| TODO/FIXME/unwrap | None |
| Public API | `A2AProtocol`, `AgentCardRegistry`, `AgentCard`, `A2AMessage`, `A2ARequest`, `A2AResponse`, `TaskSpec`, `TaskPriority` |
| Quality | Clean A2A implementation. Agent card capability matching. Pending message queue per agent. Event bus integration for message notifications. |

---

### 19. `persona.rs` — 147 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 147 |
| Implemented | ✅ Complete — Persona struct with 3 defaults (Dev, Review, Research) |
| Error handling | N/A (data types only) |
| Tests | None |
| TODO/FIXME/unwrap | None |
| Public API | `Persona`, `default_personas()` |
| Quality | Clean data model. Well-designed default personas with distinct personalities. |

---

### 20. `persona_manager.rs` — 127 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 127 |
| Implemented | ✅ Complete — persona lifecycle, active persona management, system prompt |
| Error handling | ✅ Good — validates persona exists and is enabled before setting active |
| Tests | None |
| TODO/FIXME/unwrap | None |
| Public API | `PersonaManager` (new, get/set active, active_system_prompt, create_default_personas) |
| Quality | Clean coordinator. Proper `Clone` implementation. Falls back to default prompt when no active persona. |

---

### 21. `persona_store.rs` — 183 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 183 |
| Implemented | ✅ Complete — in-memory CRUD for personas |
| Error handling | ✅ Good — proper error returns for missing personas |
| Tests | ✅ 4 unit tests (register/get, list_enabled, set_enabled, delete) |
| TODO/FIXME/unwrap | None |
| Public API | `PersonaStore`, `PersonaStoreHandle` |
| Quality | Simple and clean. Thread-safe with `parking_lot::RwLock`. `PersonaStoreHandle` for Arc sharing. |

---

### 22. `engine.rs` — 135 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 135 |
| Implemented | ✅ Complete — `EngineProvider` trait + `OxiEngineProvider` using oxi-ai |
| Error handling | ✅ Good — proper error returns for unknown providers/models |
| Tests | ✅ 6 unit tests (model resolution, provider creation, not-found cases) |
| TODO/FIXME/unwrap | None |
| Public API | `EngineProvider` trait, `OxiEngineProvider` |
| Quality | Clean abstraction over oxi-ai. Supports both `provider/model` and bare `model` forms. |

---

### 23. `types.rs` — 51 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 51 |
| Implemented | ✅ Complete — core type definitions |
| Error handling | N/A |
| Tests | None |
| TODO/FIXME/unwrap | None |
| Public API | `AgentId` (Uuid), `AgentStatus`, `AgentInfo` |
| Quality | Minimal and correct. `AgentStatus` has `Display` impl. |

---

### 24. `tools/mod.rs` — 15 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 15 |
| Implemented | ✅ Module declarations and re-exports |
| Tests | None |
| Quality | Clean module organization. |

---

### 25. `tools/container_exec.rs` — 298 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 298 |
| Implemented | ✅ Complete — container command execution via `AgentTool` trait |
| Error handling | ✅ Good — P0 security: no local fallback, proper error messages |
| Tests | ✅ 4 unit tests (schema, no-active-container security, missing command) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `ContainerExecTool` |
| Quality | Security-first design. Refuses to execute if no container is active (P0 fix). Consistent output formatting with timing info. |

---

### 26. `tools/host_exec_tool.rs` — 308 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 308 |
| Implemented | ✅ Complete — structured binary+args host execution |
| Error handling | ✅ Good — allowlist enforcement delegated to bridge, timeout handling |
| Tests | ✅ 8 unit tests (echo, blocked binary, missing params, stderr, non-zero exit, timeout, schema) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `HostExecTool` |
| Quality | Good security model. Structured binary+args API (not shell strings). Timeout with max cap of 60s. |

---

### 27. `tools/mcp_tool.rs` — 178 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 178 |
| Implemented | ✅ Complete — MCP tool as `AgentTool` with namespaced naming |
| Error handling | ✅ Good — error logging, graceful failure responses |
| Tests | ✅ 2 unit tests (debug format, name format) |
| TODO/FIXME/unwrap | None |
| Public API | `McpToolWrapper` |
| Quality | Clean wrapper. Namespaced as `mcp:{server}:{tool}` to avoid collisions. Handles text/image/resource content blocks. |

---

### 28. `tools/program_tool.rs` — 308 lines

| Aspect | Assessment |
|--------|-----------|
| Lines | 308 |
| Implemented | ✅ Complete — automatic host/container routing based on program requirements |
| Error handling | ✅ Good — delegates to Tier 2 tools which handle errors |
| Tests | ✅ 3 unit tests (host routing, container routing, global config routing) |
| TODO/FIXME/unwrap | None in production code |
| Public API | `ProgramTool` |
| Quality | Intelligent routing logic. Checks both program-level and global config host requirements. |

---

## Summary Statistics

| Metric | Value |
|--------|-------|
| Total files | 28 |
| Total lines | 13,026 |
| Files with tests | 20/28 (71%) |
| Total test functions | 162 |
| Files with `#[allow(dead_code)]` | 2 |
| Files with `#[allow(unused)]` | 1 |
| Production `unwrap()` calls | 5 |
| `TODO`/`FIXME` comments | 0 |
| `panic!()` calls | 0 |

---

## Top Files by Line Count

| File | Lines |
|------|-------|
| access_manager.rs | 1,676 |
| program.rs | 1,136 |
| mcp.rs | 1,225 |
| scheduler.rs | 842 |
| container.rs | 890 |
| context_manager.rs | 719 |
| a2a.rs | 699 |
| agent_runtime.rs | 671 |

---

## Test Coverage by Module

| Module | Tests | Assessment |
|--------|-------|-----------|
| access_manager | 30+ | ✅ Excellent |
| scheduler | 24 | ✅ Excellent |
| context_manager | 21 | ✅ Excellent |
| program | 17 | ✅ Very Good |
| mcp | 18 | ✅ Very Good |
| host_exec | 9 | ✅ Good |
| host_tools | 11 | ✅ Good |
| container | 8 | ✅ Good |
| container_manager | 4 | ✅ Good |
| state_store | 5 | ✅ Good |
| container_exec | 4 | ✅ Good |
| host_exec_tool | 8 | ✅ Good |
| program_tool | 3 | ✅ Adequate |
| persona_store | 4 | ✅ Adequate |
| agent_runtime | 4 | ✅ Adequate |
| skill | 3 | ✅ Adequate |
| engine | 6 | ✅ Adequate |
| a2a | 5 | ✅ Adequate |
| mcp_tool | 2 | ⚠️ Minimal |
| supervisor | 0 | ❌ Missing |
| event_bus | 0 | ❌ Missing |
| config | 0 | ❌ Missing |
| orchestrator | 0 | ❌ Missing |
| persona | 0 | ❌ Missing (data type only) |
| persona_manager | 0 | ❌ Missing |
| types | 0 | ❌ Missing (data type only) |
| tools/mod.rs | 0 | N/A |

---

## Production `unwrap()` Calls (Should Be Fixed)

1. **`agent_runtime.rs:83-84`** — `make_placeholder_container_manager()`: `tempfile::tempdir().unwrap()` and `StateStore::new(...).unwrap()`. Should propagate errors or use `expect()` with a message.

2. **`program.rs:295`** — `install_from_git()`: `entries.into_iter().next().unwrap().path()`. Assumes exactly one directory in clone result.

3. **`program.rs:360`** — `install_from_tarball()`: Same pattern as above. Assumes one extracted directory.

4. **`program.rs:448`** — `bootstrap_defaults()`: `src.file_name().unwrap()`. Should handle non-UTF8 filenames.

---

## `#[allow(dead_code)]` / `#[allow(unused)]` Annotations

1. **`agent_runtime.rs:122`** — `#[allow(dead_code)]` on `with_config()`. This is a legitimate builder method; the annotation should be removed or a `#[cfg(test)]` usage should be added.

2. **`orchestrator.rs:46`** — `#[allow(dead_code)]` on `persona_manager`. Comment says "Reserved for future persona-driven agent customization."

3. **`orchestrator.rs:476`** — `#[allow(unused)]` on `InterviewSession`. Fields are tracked but not all actively read.

---

## Error Handling Assessment

- **Pattern:** Consistent use of `anyhow::Result` throughout. `thiserror` is not used (all errors are `anyhow`).
- **Context chains:** Good use of `.with_context()` in container.rs, host_exec.rs, program.rs, and orchestrator.rs.
- **Validation:** Input validation is thorough (path traversal, shell metacharacters, name sanitization).
- **Silent error discards:** `let _ = self.event_bus.publish(...)` is used in supervisor.rs and orchestrator.rs — acceptable for event publishing.

---

## Architecture Observations

### Strengths
1. **Clean module boundaries** — Each module has a single responsibility.
2. **Trait-based abstraction** — `Supervisor`, `ContainerBackend`, `EngineProvider` allow testing and future extension.
3. **Security-first** — Access manager with RBAC, sandbox boundaries, audit logging, HitL approvals.
4. **Comprehensive config** — Well-structured TOML config with sensible defaults for all subsystems.
5. **Good documentation** — Module-level `//!` docs, doc comments on public types.

### Areas for Improvement
1. **Orchestrator method size** — `handle_message()` is ~300 lines. Should extract interview handling, evolution loop, and A2A registration into helper methods.
2. **Missing integration tests** — No `tests/` directory with integration test files. The module is only tested via unit tests.
3. **Scheduler stats incomplete** — `completed` and `failed` counters always report 0.
4. **No persistence for personas** — `PersonaStore` is in-memory only; personas are not saved to disk.
5. **`supervisor::wait()` is misleading** — Returns current status rather than actually waiting for completion.

---

## Recommendations

### Priority 1 (Fix)
- Replace 5 production `unwrap()` calls with proper error handling
- Add tests for `supervisor.rs`, `event_bus.rs`, `config.rs`, and `orchestrator.rs`
- Remove `#[allow(dead_code)]` from `agent_runtime::with_config` or add a usage

### Priority 2 (Improve)
- Extract sub-methods from `orchestrator::handle_message`
- Implement `completed`/`failed` counters in `AgentScheduler::stats()`
- Add `#[serde(deny_unknown_fields)]` to config structs for strict parsing
- Consider `thiserror` for library-style error types

### Priority 3 (Enhance)
- Add persistence layer for `PersonaStore`
- Make `supervisor::wait()` actually await agent completion
- Add integration tests in a `tests/` directory
- Consider adding a `Kernel` struct that ties all components together
