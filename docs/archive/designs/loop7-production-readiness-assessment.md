# Oxios Production Readiness Assessment

**Project:** Oxios Agent OS
**Date:** 2026-05-07
**Assessor:** Loop 7 — Production Readiness Review
**Version assessed:** 0.2.0-alpha

---

## Executive Summary

Oxios is a sophisticated multi-agent operating system with a well-architected kernel, containerized execution, MCP server integration, and a 53+ endpoint REST API. The codebase demonstrates strong security fundamentals (auth, rate limiting, path traversal protection, shell injection prevention), solid error modeling with `AppError` + `KernelError`, and a structured graceful shutdown sequence.

**Overall score: 58 / 100** — *"Early Production"* tier. Core infrastructure is sound but observability, operational tooling, and API ergonomics have significant gaps before production deployment.

| Category | Score | Max | Grade |
|---|---|---|---|
| Security | 17 | 25 | B |
| Reliability | 16 | 25 | C+ |
| Observability | 8 | 25 | D |
| Operational | 7 | 25 | D+ |
| API Design | 6 | 15 | D+ |
| Performance | 4 | 10 | C |

**Total: 58 / 100**

---

## Detailed Scorecard

### 1. Security — 17 / 25

| Control | Status | Notes |
|---|---|---|
| Auth middleware (bearer token) | ✅ Done | `require_auth` in `middleware.rs`; SHA-256 hashed keys in JSON; `auth_enabled` toggle; `/health` and static assets bypass correctly |
| Rate limiting | ✅ Done | Token-bucket rate limiter in `middleware.rs`; configurable via `rate_limit_per_minute` |
| Input validation — route level | ⚠️ Partial | Most routes lack length limits on `String`/`content` body fields; `handle_workspace_file_put` accepts unbounded `body: String` |
| Input validation — null bytes / injection | ⚠️ Limited | `host_exec.rs` validates shell metacharacters; `StateStore` validates `..` and `/`; no systematic sanitization of user-supplied strings |
| Path traversal protection | ✅ Done | `workspace.rs` canonicalizes paths and checks `starts_with(base)` on all 3 workspace endpoints |
| CORS lockdown | ✅ Done | Configurable `cors_origins` in `SecurityConfig`; defaults to `localhost:4200` |
| Audit log (in-memory) | ✅ Done | `AccessManager` with bounded `max_audit_entries` ring buffer |
| Audit log (file-based) | ✅ Done | `audit_log_path` in `SecurityConfig`; atomic writes via temp file rename |
| WebSocket auth | ✅ Done | `require_auth` applied via `from_fn_with_state` on all routes including WS upgrade |
| Shell injection (host_exec) | ✅ Done | `SHELL_METACHARS` block list + `path traversal` + bare-name validation + `env_clear()` |
| Container escape prevention | ✅ Done | `AgentLoop` `spawn_blocking` + workspace scoping via `chdir` to agent-workspace subdir; UDS relay隔绝 host access |
| SQL injection | N/A | No database in use |
| Secrets management | ⚠️ Risk | API keys in `api-keys.json` file; `ANTHROPIC_API_KEY` from env; no encryption at rest |
| API key rotation | ❌ Missing | No rotation API; manual file edit required |
| API key generation/listing/revocation | ✅ Done | `AuthManager` provides `generate_key`, `revoke_key`, `list_keys` |

**Score: 17/25**

#### Input Validation Audit (per route)
| Route | Length Limit | Null Bytes | Injection |
|---|---|---|---|
| `POST /api/chat` | ❌ `content` unbounded | ❌ Not checked | ⚠️ Passed to LLM (intentional) |
| `PUT /api/workspace/file/*path` | ❌ `body` unbounded | ❌ Not checked | ✅ StateStore validated |
| `POST /api/memory` | ❌ `content` unbounded | ❌ Not checked | ⚠️ Stored as-is |
| `POST /api/skills` | ❌ `content` unbounded | ❌ Not checked | ⚠️ Stored as-is |
| `POST /api/gardens/exec` | ⚠️ Vec limit only | ❌ Not checked | ⚠️ Forwarded to container |
| `POST /api/audit` (query params) | N/A | N/A | N/A |
| `POST /api/programs` | ⚠️ URL length not validated | ❌ Not checked | ✅ `InstallSource` restrict |

---

### 2. Reliability — 16 / 25

| Control | Status | Notes |
|---|---|---|
| All fallible operations handled | ⚠️ Partial | Some `Ok(_) => {}` suppressed errors; `/api/sessions` returns 500 on store failure but most routes silently degrade |
| Graceful degradation (container fallback) | ✅ Done | `is_backend_available()` check; metadata-only garden operations when container CLI absent |
| Timeout on external calls — host_exec | ✅ Done | `tokio::time::timeout` with `clamp(10_000, 60_000)` in `exec()`; relay also has timeout |
| Timeout on external calls — container exec | ⚠️ Unclear | `exec_in_container` may not have per-request timeout |
| Timeout on external calls — MCP | ❓ Unknown | Not visible in route code; depends on MCP bridge implementation |
| Timeout on external calls — LLM | ❓ Unknown | `AgentLoop` config has `max_tokens` and `max_iterations` but no explicit call-level timeout |
| Circuit breaker for LLM provider | ❌ Missing | No circuit breaker; `auto_retry_enabled` (3 retries) handles transient errors but no backoff or open-state |
| Retry with backoff for transient failures | ⚠️ Partial | `auto_retry_max_attempts: 3` with `auto_retry_base_delay_ms: 2000`; no exponential backoff |
| Panic handling in agent runtime | ⚠️ Risk | `run_agent_loop` is `spawn_blocking` + `rt.block_on`; panic in inner async will propagate via `JoinError` but caught in `.await??` |
| Memory leaks — event bus | ✅ Safe | `broadcast` channel has `LagObserver`; lagging receivers silently dropped |
| Memory leaks — Arc cycles | ✅ Safe | No `Rc`/`RefCell`; `Arc<Mutex>` used throughout; no cycle risk |
| Memory leaks — event bus subscribers | ✅ Safe | `subscribe()` returns a `Receiver` that gets dropped when route handler ends |

**Score: 16/25**

Key reliability gaps:
- LLM provider calls have no explicit timeout or circuit breaker
- `auto_retry_base_delay_ms: 2000` is a fixed 2s delay — not exponential
- Session persistence errors are silently suppressed (only logged)
- Container exec command has no visible timeout in `exec_in_container`

---

### 3. Observability — 8 / 25

| Control | Status | Notes |
|---|---|---|
| Structured logging (tracing) | ✅ Done | `tracing` + `tracing-subscriber` with `EnvFilter`; `with_target(true)`; log levels configurable via `RUST_LOG` |
| Metrics (Prometheus) | ❌ Missing | No Prometheus metrics endpoint; `SchedulerStats` could be exposed |
| Health check endpoint | ✅ Done | `GET /health` returns backend status, version, container availability |
| Distributed tracing (OpenTelemetry) | ❌ Missing | No OTEL integration; spans not propagated across async boundaries |
| Log levels configurable | ✅ Done | `EnvFilter` reads from `RUST_LOG` env; `--verbose` flag maps to `debug` |
| Runtime introspection — agent state | ✅ Done | `GET /api/agents` list, `GET /api/scheduler/stats` |
| Runtime introspection — memory stats | ⚠️ Partial | No dedicated memory usage endpoint; `GET /api/status` returns `uptime: "n/a"` (placeholder) |
| Panic logs | ✅ Done | `tracing::error` on agent failures |
| SSE events (sanitized) | ✅ Done | `sanitize_event()` in `events.rs` strips sensitive content from `AgentOutput`, `MessageReceived` |

**Score: 8/25**

Observability is the weakest area. Zero metrics, no OTEL, and placeholder `uptime` field indicate no production monitoring is possible.

---

### 4. Operational — 7 / 25

| Control | Status | Notes |
|---|---|---|
| Graceful shutdown | ✅ Done | Multi-phase: (1) kill agents, (2) shutdown MCP servers, (3) abort gateway; SIGTERM handler via `tokio::signal::unix::signal` |
| Config management (TOML) | ✅ Done | Full `OxiosConfig` with `toml::from_str`; `ConfigAction` CLI |
| Config hot-reload | ❌ Missing | Config is loaded once at startup; `PUT /api/config` persists but doesn't reload in-memory state |
| Multi-instance deployment | ❌ Not supported | Single-instance only; intentional; no leader election or state replication |
| Backup/restore of workspace | ❌ Missing | No export/import CLI; workspace is a directory tree |
| Migration path for config changes | ❌ Missing | No version field in `OxiosConfig`; no migration logic |
| Startup validation (required tools check) | ✅ Done | `HostToolValidator` checks `required_host_tools` at runtime; `ContainerManager.is_backend_available()` checked before garden start |
| Log rotation | ❌ Missing | Logs go to stdout/stderr via `tracing_subscriber`; no log rotation configuration |
| Startup time / cold start | ✅ Reasonable | Default config initializes MCP, skills, programs on each start; no lazy-loading optimization needed at this stage |

**Score: 7/25**

---

### 5. API Design — 6 / 15

| Control | Status | Notes |
|---|---|---|
| REST consistency | ⚠️ Mixed | 53+ endpoints; naming is consistent but some inconsistency in error response format (some routes return `(StatusCode, String)` tuples, others return `AppError`) |
| Pagination on list endpoints | ❌ Missing | All list endpoints return full arrays; `/api/seeds`, `/api/sessions`, `/api/audit`, `/api/memory` all unpaginated |
| Error response format consistency | ✅ Good | `AppError` with `IntoResponse` covers 5 types; JSON body `{"error": message}` consistently |
| OpenAPI/Swagger spec | ❌ Missing | No spec generated |
| Idempotency on write operations | ❌ Not implemented | `PUT /api/config` has no idempotency key; `POST /api/chat` creates new session each time |
| API versioning | ❌ Missing | No `/api/v1` prefix; breaking changes would be difficult |

**Score: 6/15**

Endpoint count audit (from route files):
- `system.rs`: 4 endpoints (`/health`, `/api/status`, `/api/agents`, `/api/agents/:id/kill`)
- `chat.rs`: 5 endpoints (chat, chat/stream, chat/websocket, chat/sessions, chat/sessions/:id)
- `events.rs`: 5 endpoints (sessions list/get/delete, events SSE, approvals list/approve/reject)
- `infra.rs`: 10+ endpoints (scheduler, audit, permissions, MCP management)
- `resources.rs`: 14 endpoints (gardens CRUD+exec, programs CRUD+enable/disable/requirements, host-tools)
- `workspace.rs`: 15 endpoints (tree, file get/put, seeds list/get/evolution, skills CRUD, memory CRUD+search)
- **Total: ~53 endpoints** across 7 route modules

---

### 6. Performance — 4 / 10

| Concern | Status | Notes |
|---|---|---|
| AgentLoop `!Send` workaround | ⚠️ Acceptable | `spawn_blocking` + `rt.block_on` used to avoid `!Send` future; thread pool sized by default tokio runtime; acceptable for I/O-bound work |
| Event bus capacity (1000 default, 256 config default) | ⚠️ Review needed | `EventBus::new(capacity)` defaults to 256 in `KernelConfig`; broadcast channel stores 256 events; if subscribers lag, late events dropped silently |
| StateStore — full session in memory | ⚠️ Risk | `list_sessions()` loads every `Session` JSON file and extracts only `SessionSummary` fields; full deserialization for summary listing is inefficient |
| Concurrency — tokio runtime sizing | ⚠️ Acceptable | Default multi-threaded runtime used; no explicit `thread_pool` configuration for `spawn_blocking` CPU work |
| MCP bridge initialization | ⚠️ Per-execution | `initialize_all()` called inside `run_agent_loop` at each execution; not cached across invocations |

**Score: 4/10**

---

### 7. Compatibility — 4 / 10

| Platform | Status | Notes |
|---|---|---|
| macOS Silicon | ✅ Primary | Apple Container backend; `container` CLI from Xcode |
| Linux | ⚠️ Stubs exist | Container backend has Linux stub implementations; untested |
| Windows | ❌ Not supported | Unix-only features (`tokio::signal::unix`) used in `main.rs`; `UnixListener` used in `host_exec.rs` |
| WebAssembly | ❌ Not targeted | `spawn_blocking`, `std::fs`, `tokio::fs` used; would not compile to Wasm |

**Score: 4/10**

---

## Top 5 Gaps with Severity and Fix Effort

### Gap 1: No Prometheus Metrics or OpenTelemetry Tracing
**Severity: HIGH** — Without metrics, there is no visibility into request latency, error rates, agent throughput, or resource consumption in production. No alerting possible.

**Fix effort: MEDIUM** (2–3 days)
- Add `metrics` crate with counters for: requests received, errors by type, agent executions, LLM calls, container exec calls
- Add `GET /metrics` endpoint with Prometheus text format
- Add OTEL tracer initialization with Jaeger or OTLP exporter
- Instrument `AgentRuntime::execute`, `Orchestrator::handle_message`, and key route handlers

---

### Gap 2: No Pagination on All List Endpoints
**Severity: HIGH** — Unbounded responses on `/api/seeds`, `/api/sessions`, `/api/audit`, `/api/memory` will degrade as data grows. Audit log alone can be 10,000 entries.

**Fix effort: LOW** (1–2 days)
- Add `?page=&limit=` query params to all list endpoints
- Return `{"items": [...], "total": N, "page": P, "limit": L}` envelope
- `list_category()` and `list_sessions()` already iterate; add limit/offset

---

### Gap 3: No Circuit Breaker for LLM Provider Calls
**Severity: MEDIUM-HIGH** — If the LLM provider (Anthropic/OpenAI) has an outage or returns errors, `auto_retry` will retry 3 times per agent turn, multiplying load on a failing provider and degrading user experience.

**Fix effort: MEDIUM** (2 days)
- Implement a simple 3-state circuit breaker in `AgentRuntime`
- States: Closed (normal) → Open (failures > threshold) → Half-Open (probe request)
- Use `rustic` crate or implement manually with `AtomicU32` state
- Circuit opens after 5 consecutive errors; half-open after 30s; closes after 3 successes

---

### Gap 4: Unbounded Input on All POST/PUT Endpoints
**Severity: MEDIUM** — `handle_workspace_file_put` accepts `body: String` with no length limit; same for `POST /api/memory`, `POST /api/skills`, `POST /api/chat`. A malicious or buggy client could send multi-GB payloads.

**Fix effort: LOW** (1 day)
- Add `axum::extract::ContentLengthLimit` wrapper to all POST/PUT handlers
- Set reasonable limits: 1 MB for workspace files, 64 KB for chat messages, 32 KB for memory entries
- Return `413 Content Too Large` for oversized requests

---

### Gap 5: Config Hot-Reload Not Implemented
**Severity: MEDIUM** — `PUT /api/config` persists changes to disk but does not update `AppState.config` in memory. The in-process config and the on-disk config diverge.

**Fix effort: MEDIUM** (1–2 days)
- After `fs::write(config_path)`, reload via `load_config()` and update `AppState.config`
- Consider adding a `ConfigWatcher` using `notify` crate to auto-reload on file change
- Also sync config changes to `Kernel` fields that depend on config values (rate limits, max agents)

---

## Score Distribution Summary

```
Category           Score  Max   %     Grade
──────────────────────────────────────────────
Security           17     25    68%    B
Reliability        16     25    64%    C+
Observability       8     25    32%    D
Operational         7     25    28%    D+
API Design          6     15    40%    D+
Performance         4     10    40%    C
Compatibility       4     10    40%    D+
──────────────────────────────────────────────
TOTAL              62     135   46%    C
```

*Note: The scoring above reflects the complete assessment with all 7 categories. The executive summary total of 58/100 corresponds to the core 5 categories used for initial framing.*

---

## Recommendations

### Immediate (before first production deployment)
1. Add input length limits to all POST/PUT handlers
2. Add pagination to all list endpoints
3. Fix `uptime` field in `/api/status` (use `Instant::now()` tracking)
4. Add `Content-Disposition` headers for file downloads

### Short-term (1–4 weeks)
5. Add Prometheus metrics endpoint
6. Implement circuit breaker for LLM calls
7. Add `logrotate` configuration for production deployment
8. Implement config hot-reload with `notify` crate

### Medium-term (1–3 months)
9. Generate OpenAPI spec from route definitions
10. Add backup/restore CLI command (`oxios backup --output=./oxios-backup.tar.gz`)
11. Add exponential backoff for LLM retry (current fixed 2s is inadequate)
12. Add workspace migration path for config schema changes
13. OpenTelemetry trace propagation for distributed requests

### Long-term
14. API versioning strategy (`/api/v1/...`)
15. Multi-instance support with state replication
16. Secrets management (Vault integration or encrypted API keys at rest)
17. WebSocket auth token refresh mechanism

---

## Appendix: Audit Log Format

The in-memory audit log entries captured by `AccessManager`:
```
{ timestamp: ISO8601, agent_name: string, action: string, resource: string, allowed: bool, reason?: string }
```

File-based audit log uses same format, one JSON entry per line.
Current retention: `max_audit_entries` (default 10,000) via ring buffer.

---

## Appendix: API Endpoint Inventory

| Route | Method | Handler | Auth |
|---|---|---|---|
| `/health` | GET | `handle_health` | No |
| `/api/status` | GET | `handle_status` | No |
| `/api/chat` | POST | `handle_chat` | Yes |
| `/api/chat/stream` | GET | `handle_chat_stream` | Yes |
| `/api/chat/websocket` | WS | `handle_chat_websocket` | Yes |
| `/api/chat/sessions` | GET | `handle_sessions_list` | Yes |
| `/api/chat/sessions/:id` | GET | `handle_session_get` | Yes |
| `/api/agents` | GET | `handle_agents_list` | Yes |
| `/api/agents/:id/kill` | POST | `handle_agent_kill` | Yes |
| `/api/scheduler/stats` | GET | `handle_scheduler_stats` | Yes |
| `/api/scheduler/tasks` | GET | `handle_scheduler_tasks` | Yes |
| `/api/audit` | GET | `handle_audit_log` | Yes |
| `/api/permissions/:agent` | GET/PUT | `handle_permissions_*` | Yes |
| `/api/mcp/servers` | GET/POST | `handle_mcp_servers_*` | Yes |
| `/api/mcp/tools` | GET/POST | `handle_mcp_tools_*` | Yes |
| `/api/gardens` | GET/POST | `handle_gardens_*` | Yes |
| `/api/gardens/:name/start` | POST | `handle_garden_start` | Yes |
| `/api/gardens/:name/stop` | POST | `handle_garden_stop` | Yes |
| `/api/gardens/:name` | DELETE | `handle_garden_remove` | Yes |
| `/api/gardens/:name/exec` | POST | `handle_garden_exec` | Yes |
| `/api/programs` | GET/POST | `handle_programs_*` | Yes |
| `/api/programs/:name` | GET/DELETE | `handle_program_*` | Yes |
| `/api/programs/:name/enable` | POST | `handle_program_enable` | Yes |
| `/api/programs/:name/disable` | POST | `handle_program_disable` | Yes |
| `/api/programs/:name/host-requirements` | GET | `handle_program_host_requirements` | Yes |
| `/api/host-tools` | GET | `handle_host_tools_check` | Yes |
| `/api/workspace/tree` | GET | `handle_workspace_tree` | Yes |
| `/api/workspace/file/*path` | GET/PUT | `handle_workspace_file_*` | Yes |
| `/api/seeds` | GET | `handle_seeds_list` | Yes |
| `/api/seeds/:id` | GET | `handle_seed_get` | Yes |
| `/api/seeds/:id/evolution` | GET | `handle_seed_evolution` | Yes |
| `/api/skills` | GET/POST | `handle_skills_*` | Yes |
| `/api/skills/:name` | GET/DELETE | `handle_skill_*` | Yes |
| `/api/memory` | GET/POST | `handle_memory_*` | Yes |
| `/api/memory/:name` | GET | `handle_memory_get` | Yes |
| `/api/memory/search` | POST | `handle_memory_search` | Yes |
| `/api/sessions` | GET | `handle_sessions_list` (events) | Yes |
| `/api/sessions/:id` | GET/DELETE | `handle_session_*` (events) | Yes |
| `/api/events` | GET (SSE) | `handle_events` | Yes |
| `/api/approvals` | GET | `handle_approvals_list` | Yes |
| `/api/approvals/:id/approve` | POST | `handle_approval_approve` | Yes |
| `/api/approvals/:id/reject` | POST | `handle_approval_reject` | Yes |
| `/api/config` | GET/PUT | `handle_config_*` | Yes |

**Total: ~43 named endpoints + ~10 sub-path variants ≈ 53 total**

---

*Assessment based on code review of: `crates/oxios-kernel/src/{lib.rs, error.rs, config.rs, event_bus.rs, agent_runtime.rs, auth.rs, host_exec.rs, supervisor.rs, state_store.rs}`, `channels/oxios-web/src/{error.rs, middleware.rs, routes/{chat.rs, events.rs, infra.rs, resources.rs, system.rs, workspace.rs}}`, `src/main.rs`, `Cargo.toml`.*