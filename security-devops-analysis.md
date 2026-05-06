# Oxios Security, DevOps & Production Readiness Analysis

**Date:** 2026-05-06  
**Version:** 0.2.0-alpha  
**Total Source Lines:** ~21,141 (Rust, excluding target/)  
**Crates Analyzed:** oxios-kernel, oxios-ouroboros, oxios-gateway, oxios-web (channel), oxios-frontend  

---

## Executive Summary

Oxios is at **alpha stage** — well-architected with solid security primitives (RBAC, access manager, audit logging), comprehensive test coverage (120+ test functions), and proper tracing/observability. However, it has critical production blockers: **no authentication on the HTTP API**, **permissive CORS**, **no CI/CD**, **no containerization/deployment config**, and **`anyhow` everywhere** without typed error handling. The codebase is clean (zero `unsafe`, zero `TODO/FIXME/HACK`) but needs hardening before any production use.

---

## 1. `unwrap()`, `panic!()`, `expect()` Audit

### Production Code (non-test)

| Category | Count | Severity |
|----------|-------|----------|
| `unwrap()` in production code | **0** | ✅ Clean |
| `panic!()` in production code | **0** | ✅ Clean |
| `expect()` in production code | **3** | ⚠️ Low |

The 3 `expect()` calls in production code are in `orchestrator.rs`:

```
crates/oxios-kernel/src/orchestrator.rs:119   sessions.get(&session_id).expect("session exists");
crates/oxios-kernel/src/orchestrator.rs:304   current_seed.as_ref().expect("seed exists");
crates/oxios-kernel/src/orchestrator.rs:390   current_seed.expect("at least one seed exists");
```

**Risk:** These represent invariant assumptions. If violated (race condition, logic bug), the orchestrator panics and takes down the process.

**Recommendation:** Replace with proper error propagation using `anyhow::Context` or `bail!()`.

### Test Code

| Category | Count | Context |
|----------|-------|---------|
| `unwrap()` in tests | ~80+ | Expected in test code |
| `expect()` in tests | ~25+ | Expected — with descriptive messages |
| `panic!()` in tests | 1 | `assert_eq!` pattern match |

Test usage is appropriate — `unwrap()` and `expect()` in test code is idiomatic Rust.

### Other Production `expect()` Call Sites

- `server.rs`: `.expect("Invalid bind address")` — acceptable at startup
- `mcp.rs`: `.expect("stdin/stdout not captured")` — reasonable for MCP protocol
- `main.rs`: `.expect("Failed to install Ctrl+C handler")` — acceptable at startup

---

## 2. TODO / FIXME / HACK Comments

**Result: ZERO found.** ✅

No TODO, FIXME, or HACK comments in the codebase. This is exceptionally clean.

---

## 3. `unsafe` Blocks

**Result: ZERO found.** ✅

No `unsafe` blocks anywhere in the codebase. This is excellent from a memory safety perspective.

---

## 4. Dependency Version Analysis

### Workspace Dependencies (`Cargo.toml`)

| Dependency | Version | Pinned? | Notes |
|------------|---------|---------|-------|
| tokio | `"1"` | Minor-wide | ✅ Standard |
| futures | `"0.3"` | Minor-wide | ✅ Standard |
| serde | `"1"` | Major-wide | ✅ Standard |
| serde_json | `"1"` | Major-wide | ✅ Standard |
| toml | `"0.8"` | Minor-wide | ✅ Standard |
| uuid | `"1"` | Major-wide | ✅ Standard |
| tracing | `"0.1"` | Minor-wide | ✅ Standard |
| tracing-subscriber | `"0.3"` | Minor-wide | ✅ Standard |
| anyhow | `"1"` | Major-wide | ✅ Standard |
| thiserror | `"1"` | Major-wide | ✅ Standard |
| chrono | `"0.4"` | Minor-wide | ✅ Standard |
| parking_lot | `"0.12"` | Minor-wide | ✅ Standard |
| axum | `"0.8"` | Minor-wide | ✅ Standard |
| tower-http | `"0.6"` | Minor-wide | ✅ Standard |
| clap | `"4"` | Major-wide | ✅ Standard |
| reqwest | `"0.12"` | Minor-wide | ✅ Standard |
| dioxus | `"0.7"` | Minor-wide | ✅ Frontend |
| gloo-net | `"0.6"` | Minor-wide | ✅ Frontend |

### Path Dependencies

| Dependency | Source | Risk |
|------------|--------|------|
| oxi-ai | `path = "../oxi/oxi-ai"` | ⚠️ Local path — not published to crates.io |
| oxi-agent | `path = "../oxi/oxi-agent"` | ⚠️ Local path — not published to crates.io |

**Risk:** Path dependencies mean builds are not reproducible without the sibling `oxi/` directory. No version pinning for these critical dependencies.

### Security Concerns

- **No `cargo-audit` integration** — no CI to check for known CVEs in dependencies
- **`reqwest` with `json` feature** — network client present, ensure it's not used for untrusted endpoints without validation
- **`Cargo.lock` committed** (3303 lines, 328 packages) — ✅ Good for reproducible builds

**Recommendation:** 
1. Add `cargo-audit` to CI pipeline
2. Consider publishing oxi-ai/oxi-agent or using git dependencies with tag-based versioning
3. Run `cargo outdated` periodically

---

## 5. CI/CD Configuration

**Result: ❌ NONE**

No CI/CD configuration found:
- No `.github/workflows/`
- No `.gitlab-ci.yml`
- No `Jenkinsfile`
- No `.ci/` directory
- No Makefile or Justfile with CI targets

**Recommendation — Minimum CI Pipeline:**

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  check:
    runs-on: macos-latest  # Apple Container requires macOS
    steps:
      - uses: actions/checkout@v4
        with: { path: 'oxios' }
      - uses: actions/checkout@v4
        with: { repository: 'owner/oxi', path: 'oxi' }
      - run: cargo fmt --check
      - run: cargo clippy --workspace -- -D warnings
      - run: cargo test --workspace
      - run: cargo audit
```

---

## 6. Dockerfile / Deployment Configuration

**Result: ❌ NONE**

- No `Dockerfile`
- No `docker-compose.yml`
- No Kubernetes manifests
- No deployment scripts

**Note:** The project uses Apple Container (macOS Silicon only), which doesn't use Docker. However, there's still no deployment automation.

The `.programs/deploy/` directory contains a program definition with a SKILL.md describing deployment procedures, but this is an agent skill, not infrastructure configuration.

**Recommendation:**
1. Document deployment steps for macOS hosts
2. Consider a `Makefile` or `justfile` for common operations
3. Add launchd plist template for macOS daemonization
4. Document reverse proxy (nginx/caddy) configuration

---

## 7. `.gitignore` Analysis

```gitignore
/target
channels/oxios-web/frontend/target/
channels/oxios-web/static/dioxus/wasm/*.wasm
channels/oxios-web/static/dioxus/wasm/snippets/
channels/oxios-web/static/dioxus/assets/
*.swp
*.swo
.DS_Store
.env
.secrets.toml
```

**Assessment:** ✅ Good

- Properly excludes build artifacts
- Excludes `.env` and `.secrets.toml` — good practice
- Excludes editor swap files and macOS `.DS_Store`

**Missing:**
- No exclusion for `*.pem`, `*.key`, `*.p12` certificate files
- No exclusion for IDE directories (`.idea/`, `.vscode/`)
- Consider adding `*.log` for runtime logs

---

## 8. Logging & Observability

### Tracing Setup ✅

The project uses `tracing` + `tracing-subscriber` throughout — this is the Rust gold standard.

**Main binary initialization:**
```rust
tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| { /* info or debug based on -v flag */ })
    )
    .with_target(true)
    .compact()
    .init();
```

### Coverage by Module

| Module | tracing calls | Level |
|--------|--------------|-------|
| access_manager | 12+ | info, warn, debug |
| host_exec | 10+ | info, warn, error |
| supervisor | 6+ | info, error |
| scheduler | 12+ | info, warn, debug |
| agent_runtime | 6+ | info, warn, error |
| orchestrator | 5+ | info |
| context_manager | 8+ | info, debug |
| container_manager | 3+ | info |
| ouroboros_engine | 8+ | info, warn |
| gateway | 8+ | info, warn, error |

**Assessment:** ✅ Excellent coverage. Structured logging with fields (agent_id, seed_id, task_id, error).

### Missing Observability

- ❌ No metrics (no `prometheus`, `metrics`, or `opentelemetry` integration)
- ❌ No distributed tracing spans for request tracing
- ❌ No health check endpoint (or not verified)
- ❌ No structured error reporting (e.g., Sentry integration)

**Recommendation:**
1. Add `tracing-opentelemetry` for distributed tracing
2. Add a `/health` endpoint
3. Add `metrics` crate for Prometheus-compatible metrics
4. Consider `tracing-error` for enriched error spans

---

## 9. Error Type Analysis

### Current State

| Crate | Error Strategy | Assessment |
|-------|---------------|------------|
| oxios-kernel | `anyhow` everywhere (22 modules) | ⚠️ |
| oxios-ouroboros | `anyhow` everywhere | ⚠️ |
| oxios-gateway | `anyhow` everywhere | ⚠️ |
| oxios-web | `anyhow` everywhere | ⚠️ |
| mcp.rs | `McpError` struct (manual) | ⚠️ Partial |

**Key Findings:**

- **`thiserror` is in `Cargo.toml` but NEVER used** in any crate. Every module uses `anyhow::Result`.
- Only `McpError` exists as a typed error, but it's a plain struct (not `thiserror` derive).
- `anyhow` is used consistently — which is fine for applications but problematic for a library crate like `oxios-kernel` that should expose typed errors for downstream consumers.

**Risk:** API consumers cannot match on specific error variants. No structured error codes for the HTTP API.

**Recommendation:**
1. Use `thiserror` (already in deps!) for `oxios-kernel` public errors:
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum KernelError {
       #[error("Agent {id} not found")]
       AgentNotFound { id: AgentId },
       #[error("Permission denied: {reason}")]
       PermissionDenied { reason: String },
       #[error("Container {name} unavailable")]
       ContainerUnavailable { name: String },
   }
   ```
2. Keep `anyhow` for the binary crate only
3. Map typed errors to HTTP status codes in the web layer

---

## 10. Hardcoded Credentials, Paths & Configuration

### Hardcoded Paths

| Location | Value | Risk |
|----------|-------|------|
| `config.rs:88` | `"127.0.0.1"` (default gateway host) | ✅ Safe — localhost only |
| `main.rs` | `"~/.oxios/config.toml"` (default config path) | ✅ Standard convention |
| `main.rs` | `"anthropic/claude-sonnet-4-20250514"` (default model) | ⚠️ Hardcoded model |
| `main.rs` | `4200` (default port) | ✅ Safe |

### Environment Variables Checked

- `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `API_KEY` — ✅ Properly loaded from env
- `OXIOS_MCP_*` — ✅ MCP server configuration from env
- `RUST_LOG` — ✅ Via tracing-subscriber env filter
- `HOME` — ✅ For path expansion

### Hardcoded Credentials: ❌ NONE found ✅

No passwords, API keys, tokens, or secrets found in source code.

### Other Hardcoded Values

| Location | Value | Notes |
|----------|-------|-------|
| `main.rs` | Default model: `anthropic/claude-sonnet-4-20250514` | Should be configurable via config.toml |
| `config.rs` | Default port: `4200` | Reasonable |
| `config.rs` | Default container image | Not verified |

**Recommendation:** Make the default model configurable in `config.toml` rather than hardcoded in main.rs.

---

## 11. Integration & Unit Test Coverage

### Test Inventory

| Location | Type | Test Functions | Lines |
|----------|------|---------------|-------|
| `oxios-kernel/tests/integration_tests.rs` | Integration | 44 | 1,090 |
| `oxios-kernel/src/access_manager.rs` | Unit | 43 | ~600 |
| `oxios-kernel/src/tools/container_exec.rs` | Unit | 4 | ~100 |
| `oxios-kernel/src/tools/host_exec_tool.rs` | Unit | 10 | ~150 |
| `oxios-kernel/src/tools/mcp_tool.rs` | Unit | 2 | ~40 |
| `oxios-kernel/src/tools/program_tool.rs` | Unit | 3 | ~80 |
| `oxios-kernel/src/host_exec.rs` | Unit | 10 | ~150 |
| `oxios-kernel/src/container_manager.rs` | Unit | 4 | ~100 |
| `oxios-kernel/src/context_manager.rs` | Unit | 1+ | ~20 |
| `oxios-kernel/src/mcp.rs` | Unit | 20 | ~400 |
| `oxios-kernel/src/program.rs` | Unit | 21 | ~400 |
| **Total** | | **~162** | |

### Coverage by Module

| Module | Has Tests | Coverage |
|--------|-----------|----------|
| access_manager | ✅ 43 unit + integration | Excellent |
| host_exec | ✅ 10 unit | Good |
| scheduler | ✅ Integration | Good |
| state_store | ✅ Integration | Good |
| event_bus | ✅ Integration | Good |
| supervisor | ✅ Integration | Good |
| orchestrator | ✅ Integration | Good |
| program | ✅ 21 unit | Excellent |
| mcp | ✅ 20 unit | Excellent |
| container_manager | ✅ 4 unit | Good |
| context_manager | ✅ 1 unit | Minimal |
| skill | ❌ No tests | Gap |
| persona/persona_manager | ❌ Minimal | Gap |
| a2a | ❌ No tests | Gap |
| config | ❌ No tests | Gap |
| **oxios-ouroboros** | ❌ No tests | **Major Gap** |
| **oxios-gateway** | ❌ No tests | **Major Gap** |
| **oxios-web** | ❌ No tests | **Major Gap** |

### Missing Test Coverage (Critical)

1. **oxios-ouroboros** — The spec-first protocol engine has **zero tests**. Interview → Seed → Execute → Evaluate → Evolve lifecycle is untested.
2. **oxios-gateway** — Message routing has **zero tests**.
3. **oxios-web** — HTTP routes (1798 lines!) have **zero tests**. No API endpoint tests.
4. **a2a.rs** — Inter-agent communication untested.
5. **config.rs** — Configuration loading/parsing untested.

**Recommendation:** Priority order for test additions:
1. Ouroboros protocol tests (interview, seed, evaluate, evolve)
2. HTTP API route tests (using `axum::test`)
3. Gateway message routing tests
4. Config parsing tests

---

## 12. `Cargo.lock` Analysis

- **Packages:** 328 (workspace) + frontend lock
- **Lock file committed:** ✅ Yes
- **Format version:** 4 (Cargo 1.78+)

### Key Dependency Versions (from lock file)

| Package | Version | Status |
|---------|---------|--------|
| tokio | 1.x | ✅ Current |
| axum | 0.8.x | ✅ Current |
| serde | 1.x | ✅ Current |
| tracing | 0.1.x | ✅ Current |
| oxi-ai | 0.5.0 | ⚠️ Local path |
| oxi-agent | 0.5.0 | ⚠️ Local path |
| reqwest | 0.12.x | ✅ Current |

---

## Security Deep Dive

### ✅ Strengths

1. **Access Manager (RBAC)** — Comprehensive 3-tier RBAC (User/Superuser/Admin) with:
   - Tool-level access control
   - Path-based sandbox restrictions (glob patterns)
   - Agent identity tracking
   - Audit logging of all authorization decisions
   - Container workspace isolation

2. **Host Exec Bridge** — Sandboxed command execution with:
   - Allowlist-based command filtering
   - Path traversal protection (`../` detection)
   - Argument validation

3. **Container Isolation** — Apple Container-based per-project isolation

4. **No `unsafe`** — Pure safe Rust throughout

5. **No hardcoded credentials** — All secrets from environment variables

### ❌ Critical Security Issues

| # | Issue | Severity | Details |
|---|-------|----------|---------|
| 1 | **No API Authentication** | 🔴 Critical | HTTP API has zero auth — anyone who can reach port 4200 has full admin access |
| 2 | **Permissive CORS** | 🔴 Critical | `CorsLayer::permissive()` allows any origin — CSRF and data exfiltration risk |
| 3 | **No Rate Limiting on HTTP** | 🟠 High | No rate limiting on API endpoints — DoS and API abuse risk |
| 4 | **No HTTPS/TLS** | 🟠 High | HTTP only — all traffic including API keys in plaintext |
| 5 | **No Input Validation** | 🟠 High | Routes accept arbitrary strings without sanitization (1798 lines of routes, no validation layer) |
| 6 | **Host Command Injection** | 🟡 Medium | Host exec bridge has allowlist, but complex command construction could bypass in edge cases |

### Security Recommendations (Priority Order)

1. **Add authentication middleware** — API key, JWT, or session-based auth
2. **Configure CORS properly** — Restrict to known origins
3. **Add rate limiting** — `tower-governor` or `tower-limit`
4. **Add HTTPS** — TLS termination via reverse proxy or `axum-server` with rustls
5. **Input validation** — Add `validator` crate for request payloads
6. **Security headers** — Add `tower-http` security headers middleware
7. **Audit log persistence** — Current audit log is in-memory only; persist to disk

---

## Production Readiness Checklist

| Category | Status | Notes |
|----------|--------|-------|
| Error handling | ⚠️ Partial | `anyhow` everywhere; no typed errors |
| Logging | ✅ Good | `tracing` throughout with structured fields |
| Metrics | ❌ Missing | No metrics collection |
| Health checks | ❌ Missing | No `/health` endpoint |
| Authentication | ❌ Missing | No auth on HTTP API |
| Authorization | ✅ Good | RBAC + audit logging |
| CORS | ❌ Insecure | Permissive CORS |
| HTTPS/TLS | ❌ Missing | HTTP only |
| Rate limiting | ❌ Missing | No HTTP rate limiting |
| CI/CD | ❌ Missing | No automated pipeline |
| Container deployment | ❌ Missing | No deployment config |
| Secrets management | ✅ Good | Env vars, .gitignore excludes secrets |
| Database migrations | N/A | Uses file-based state store |
| Graceful shutdown | ✅ Good | SIGINT/SIGTERM handling |
| Configuration | ✅ Good | TOML config with defaults |
| Documentation | ✅ Good | AGENTS.md, SKILL.md, inline docs |
| Test coverage | ⚠️ Partial | 162 tests but gaps in ouroboros/gateway/web |
| Dependency audit | ❌ Missing | No `cargo-audit` in pipeline |
| Reproducible builds | ⚠️ Partial | Cargo.lock present but path deps break it |

---

## Priority Action Items

### Immediate (Before Any External Access)

1. **Add API authentication** — API key header middleware
2. **Fix CORS** — Restrict to `localhost` or specific origins
3. **Add `/health` endpoint** — Essential for any deployment

### Short-term (Before Staging)

4. **Add CI pipeline** — fmt, clippy, test, audit
5. **Add typed errors** — Use `thiserror` for kernel public API
6. **Add Ouroboros tests** — Core protocol must be tested
7. **Add HTTP route tests** — 1798 lines of untested routes

### Medium-term (Before Production)

8. **Add metrics/monitoring** — Prometheus + Grafana
9. **Add TLS termination** — Reverse proxy or built-in
10. **Add rate limiting** — Per-IP and per-endpoint
11. **Publish oxi dependencies** — Or switch to git deps with tags
12. **Add deployment automation** — Document or script the deploy process

---

## Summary

Oxios has a **solid architectural foundation** with excellent security primitives (RBAC, sandboxing, audit logging) and clean code (zero unsafe, zero TODOs). The main risks are **operational security gaps** (no auth, permissive CORS, no TLS) and **missing infrastructure** (no CI/CD, no deployment config). The test coverage is good for kernel internals but critically missing for the protocol engine and HTTP layer.

**Production readiness: ~40%** — Solid core, needs hardening in operational security, observability, and deployment infrastructure before any external-facing use.
