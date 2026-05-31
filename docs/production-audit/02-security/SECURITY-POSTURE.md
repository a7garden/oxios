# Oxios Security Posture — 2026-05-31

**Audit scope:** Dependency vulnerabilities, unsafe blocks, command execution, WASM sandbox  
**Auditor:** AI-assisted (pi coding agent)  
**Review date:** 2026-05-31

---

## Vulnerability Summary

| ID | Severity | Crate | Status | Action Required |
|----|----------|-------|--------|-----------------|
| RUSTSEC-2026-0096 | 🔴 9.0 Critical | wasmtime 22.0.1 | **Open** | Upgrade wasmtime to ≥ 43.0.1 (see WASMTIME-UPGRADE-PLAN.md) |
| RUSTSEC-2026-0095 | 🔴 9.0 Critical | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0149 | 🟡 7.5 High | wasmtime-wasi 22.0.1 | **Open** | Same as above |
| RUSTSEC-2023-0071 | 🟡 5.9 Medium | rsa 0.9.10 | **Accepted** | No fix available. External dependency (oxi-ai). Monitor upstream. |
| RUSTSEC-2026-0020 | 🟡 6.9 Medium | wasmtime 22.0.1 | **Open** | Same as WASMTIME-UPGRADE-PLAN.md |
| RUSTSEC-2026-0021 | 🟡 6.9 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0085 | 🟡 5.6 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0087 | 🟡 4.1 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0089 | 🟡 5.9 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0091 | 🟡 6.1 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0092 | 🟡 5.9 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0093 | 🟡 6.9 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0094 | 🟡 6.1 Medium | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0086 | 🟢 2.3 Low | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2025-0118 | 🟢 1.8 Low | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2024-0438 | 🟢 — | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2025-0046 | 🟢 3.3 Low | wasmtime 22.0.1 | **Open** | Same as above |
| RUSTSEC-2026-0002 | ⚠️ Unsound | lru 0.10.1 | **Not affected** | `IterMut` API not used. No action needed. |

**Critical path risk for default build:** 🟢 **LOW** — Only RUSTSEC-2023-0071 (rsa) is reachable. All wasmtime vulnerabilities are gated behind `wasm-sandbox` which is **not** in the default feature set.

**Critical path risk with `wasm-sandbox` enabled:** 🔴 **CRITICAL** — Do not enable in production.

---

## Attack Surface

### Exec Tool (highest risk)

Oxios provides two execution modes, both with distinct threat models:

#### Shell Mode (`mode = "shell"`)
Executes arbitrary shell strings via `bash -c <cmd>`.

| Control | Status | Notes |
|---------|--------|-------|
| `allow_shell_mode` config | ✅ Off by default | Default: `false`. Must be explicitly enabled. |
| RBAC (AccessManager) | ✅ Enforced | `can_use_tool(agent, "bash")` checked before execution. |
| Command audit logging | ✅ Enabled | First 200 chars logged via `tracing::info!`. |
| Timeout enforcement | ✅ Enforced | Clamped to `max_timeout_secs` (default 1h). |
| Child process kill on shutdown | ✅ Implemented | `oneshot::Receiver` kills child on signal. |
| Environment isolation | ✅ Implemented | `env_clear()` + safe subset (HOME, USER, PATH, LANG, TERM). |

**Risk:** When both `allow_shell_mode = true` AND agent has `bash` permission → full shell access. Acceptable for trusted agents (e.g., `code-agent`) performing build/test tasks. **Do not grant `bash` to untrusted agents.**

#### Structured Mode (`mode = "structured"`)
Executes a single binary with pre-validated arguments.

| Control | Status | Notes |
|---------|--------|-------|
| Binary allowlist | ✅ Enforced | `ExecConfig::is_binary_allowed()`. Empty = permissive (dev). Enforced = only listed binaries. |
| Binary name validation | ✅ Enforced | No `/` (paths) or `..` (traversal) allowed. |
| Shell metachar blocking | ✅ Enforced | 14 characters blocked: `| & ; $ \` < > ( ) { } \n \r \0` |
| Path traversal in args | ✅ Enforced | `..` rejected. |
| RBAC (AccessManager) | ✅ Enforced | `can_use_tool(agent, binary)` checked before execution. |
| Timeout enforcement | ✅ Enforced | Same as shell mode. |

**Risk:** Well-controlled. Defense in depth with 3 independent checks. No injection path found.

#### Security Verification — Exec Tool
```
Command input path:
  Agent tool call (LLM) → exec_tool.rs → [shell|structured]_exec()
                                                  ↓
                              ┌─────────────────────┴──────────────────────┐
                              ↓                                             ↓
                     shell_exec()                                    structured_exec()
                              ↓                                             ↓
               allow_shell_mode? ──no──→ Error                    has bash permission? ──no──→ Error
                              ↓                                             ↓
                         yes                                              yes
                              ↓                                             ↓
               can_use_tool(agent, "bash")?                            binary in allowlist?
               ──no──→ Error                                              ──no──→ Error
                              ↓                                             ↓
                            yes                                            yes
                              ↓                                             ↓
                    bash -c "<command>"                            binary + args (metachar checked)
```
✅ No unmediated path from agent tool call to `Command::new`.

---

### WASM Sandbox

**Feature gate:** `wasm-sandbox` (NOT in default features)

| Item | Status |
|------|--------|
| Default features include wasm-sandbox? | ❌ **No** — `default = ["browser", "sqlite-memory"]` |
| WASM memory limit enforced? | ✅ 50 MB default |
| WASM instruction limit enforced? | ✅ 10M default (via fuel) |
| WASM module size limit enforced? | ✅ 10 MB default |
| Known sandbox escapes (ARM64)? | ✅ 2 critical advisories open (wasmtime 22.x) |
| ARM64 host risk? | ⚠️ High — if `wasm-sandbox` is enabled |

**Recommendation:** Do not enable `wasm-sandbox` in production until wasmtime is upgraded to ≥ 43.0.1. Track in WASMTIME-UPGRADE-PLAN.md.

---

### MCP Client (stdio transport)

| Item | Status | Notes |
|------|--------|-------|
| Command source | ✅ Admin-controlled | `config.toml [mcp]` section only |
| Arguments sanitized | ✅ Via config | No runtime injection possible |
| Agent modifies config? | ℹ️ File system access | Agent has workspace access — config should be owner-read-only |
| Transport security | ℹ️ stdio only | Local subprocess, no network exposure |

**Recommendation:** Ensure `~/.oxios/config.toml` has restrictive permissions (`chmod 600`) after onboarding.

---

### Web API

| Item | Status | Notes |
|------|--------|-------|
| Bind address | ✅ localhost only | `host = "127.0.0.1"` (default) |
| CORS | ✅ Restricted | `cors_origins = ["http://localhost:4200"]` (default) |
| TLS | ❌ None | localhost only — no TLS configured by default |
| Authentication | ✅ Via oxi auth | `~/.oxi/auth.json` credentials |
| Production exposure | ℹ️ Review required | Change `host = "0.0.0.0"` only with TLS and auth |

---

### Telegram Bot

| Item | Status | Notes |
|------|--------|-------|
| Token storage | ✅ Environment variable | `TELEGRAM_BOT_TOKEN` env var (not in config file) |
| Webhook vs polling | ℹ️ Review | Check `src/` for Telegram integration mode |
| Bot commands | ℹ️ Review | Validate bot command handlers in channels/ |

---

## Security Controls

| Control | Implementation | Verified |
|---------|----------------|----------|
| **RBAC** | `AccessManager` (access_manager/mod.rs) | ✅ Reviewed — `can_use_tool()` enforces per-agent tool permissions |
| **Path sandboxing** | `AccessManager` path checks | ✅ Reviewed — `FsError::UnsafePath` prevents path traversal |
| **Command allowlist** | `ExecConfig::is_binary_allowed()` | ✅ Reviewed — defended against path traversal and metachar injection |
| **Shell metachar blocking** | `SHELL_METACHARS` constant | ✅ Reviewed — 14 chars blocked in structured mode |
| **Audit trail** | `AuditTrail` (Merkle chain) | ✅ Reviewed — tamper-evident chain with `verify()` integrity check |
| **Budget enforcement** | `BudgetManager` | ✅ Implemented — token/cost limits per agent |
| **Circuit breaker** | `CircuitBreaker` (3-state) | ✅ Implemented — protection against cascading LLM failures |
| **Credential store** | `CredentialStore` | ✅ Reviewed — multi-source (env → config → oxi auth.json), no plaintext secrets |
| **Audit trail verification** | Agent-accessible via security tool | ✅ Available via `audit_trail.verify_chain`, `audit_trail.entries` |

---

## Recommendations

### 🔴 P0 — Immediate (before production)

1. **Do not enable `wasm-sandbox` in production builds.** All wasmtime vulnerabilities are gated behind this feature, but if a user builds with `--features wasm-sandbox`, they expose 16 vulnerabilities including 2 critical sandbox escapes.

2. **Keep `allow_shell_mode = false`** (already the default). Any change must be reviewed by a human. If shell mode is needed for a specific agent, grant `bash` permission only to that agent via `access_manager`.

3. **Set config.toml permissions:** `chmod 600 ~/.oxios/config.toml` to prevent agents with workspace access from modifying MCP server configurations.

### 🟡 P1 — Short-term (next sprint)

4. **Upgrade wasmtime** per WASMTIME-UPGRADE-PLAN.md. Estimated effort: 2–3 days. Target: wasmtime ≥ 43.0.1.

5. **Monitor RUSTSEC-2023-0071** (rsa Marvin attack). No fix available upstream. Track for updates from the `rsa` crate maintainers. Risk is low for Oxios's use case (API authentication, not user-controlled RSA decryption).

6. **Add SAFETY comments** to 3 `unsafe` blocks (already done in UNSAFE-COMMAND-AUDIT.md and committed during this audit).

### 🟢 P2 — Long-term (backlog)

7. **TLS for web API**: If binding to non-localhost addresses, add TLS termination (e.g., via a reverse proxy or native Axum TLS).

8. **MCP server permission model**: Consider adding per-MCP-server permissions to AccessManager so agents can be granted access to specific MCP tools without full MCP access.

9. **WebAssembly memory sandbox**: After wasmtime upgrade, evaluate adding a second layer of defense (seccomp, landlock, or AppArmor profile) for WASM sandboxing.

10. **Review unmaintained dependencies**: `bincode`, `fxhash`, `number_prefix`, `paste` are unmaintained. Plan migrations before they become blockers:
    - `bincode` → ` postcard`, `rmp-serde`, or `speedy`
    - `fxhash` → `ahash` (already used in many places)
    - `paste` → `concat_string!` (unstable) or manual `format!`

---

## Verification

| Check | Command | Expected |
|-------|---------|----------|
| No critical vulns (default build) | `cargo audit` | 0 errors (warnings only) |
| All tests pass | `cargo test --workspace` | 100% pass |
| WASM sandbox compiles | `cargo check -p oxios-kernel --features wasm-sandbox` | Compiles (vulns acknowledged) |
| Exec tool tests | `cargo test -p oxios-kernel exec_tool` | All pass |
| Unsafe blocks compile | `cargo check -p oxios-kernel` | No new errors |

---

## Artifacts

| File | Purpose |
|------|---------|
| `AUDIT-DEPS.md` | Full vulnerability classification with reachability analysis |
| `WASMTIME-UPGRADE-PLAN.md` | Step-by-step migration plan for wasmtime 22 → 43+ |
| `UNSAFE-COMMAND-AUDIT.md` | Unsafe block soundness review + Command::new security analysis |
| `SECURITY-POSTURE.md` | This document — executive summary + recommendations |

---

*This document is valid as of 2026-05-31. Re-run `cargo audit` and re-verify after any dependency update or feature change.*
