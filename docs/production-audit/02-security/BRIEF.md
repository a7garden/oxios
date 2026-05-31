# Brief 02: Security — Dependency Vulnerabilities & Hardening

**Area:** Cargo dependencies, WASM sandbox, exec security  
**Severity:** 🔴 Critical  
**Estimated scope:** 18 known vulnerabilities, 1 critical  

---

## Context

`cargo audit` reports **18 vulnerabilities**. The critical ones:

| ID | Severity | Crate | Issue |
|----|----------|-------|-------|
| RUSTSEC-2026-0096 | 🔴 **9.0 Critical** | wasmtime 22.0.1 | Sandbox escape on aarch64 Cranelift |
| RUSTSEC-2026-0020 | 🟡 6.9 Medium | wasmtime 22.0.1 | Guest-controlled resource exhaustion |
| RUSTSEC-2026-0086 | 🟢 2.3 Low | wasmtime 22.0.1 | Host data leakage with 64-bit tables |
| RUSTSEC-2023-0071 | 🟡 5.9 Medium | rsa 0.9.10 | Marvin timing attack (no fix available) |

**wasmtime** is used via the `wasm-sandbox` feature gate in `oxios-kernel`
for executing untrusted code. The critical RUSTSEC-2026-0096 means a
malicious WASM module could **escape the sandbox** on ARM64 hosts.

**rsa** comes through `oxi-ai` → `oxi-sdk` dependency chain. No fix is
available upstream. This is an external dependency issue.

Additional security-relevant patterns:
- 8 `Command::new` / `std::process::Command` usages in production code
- 6 `unsafe` blocks (WASM-related)
- ExecTool has two modes: `shell` (bash -c, RBAC-enforced) and
  `structured` (binary allowlist + metacharacter blocking)

---

## Objective

1. **Eliminate the critical wasmtime sandbox escape vulnerability**
2. **Audit and document all remaining vulnerabilities**
3. **Verify the exec tool security model is sound**
4. **Produce a security posture document**

This does NOT mean:
- ❌ Rewriting the WASM sandbox from scratch
- ❌ Removing WASM support entirely
- ❌ Forking or patching upstream crates
- ❌ Adding runtime security scanning overhead

It DOES mean:
- ✅ Upgrading wasmtime to a patched version (≥36.0.7 or ≥42.0.2)
- ✅ If upgrade is blocked by API changes, document the gap and create
  a mitigation plan
- ✅ Audit every `unsafe` block — verify soundness
- ✅ Audit every `Command::new` call — verify sanitization
- ✅ Create `docs/production-audit/02-security/SECURITY-POSTURE.md`

---

## Approach

### Phase 1: Dependency Audit

1. Run `cargo audit` and capture the full output
2. For each vulnerability:
   - Determine if it's reachable in the default feature configuration
   - Determine if it's reachable in the `wasm-sandbox` feature
   - Determine if it's reachable at all (dev-dependency only?)
3. Classify: **REACHABLE** / **UNREACHABLE** / **CONDITIONAL**
4. Write classification to `docs/production-audit/02-security/AUDIT-DEPS.md`

### Phase 2: wasmtime Upgrade Assessment

1. Check what version of wasmtime is currently used: `wasmtime = "22.0.1"`
2. Check the latest stable wasmtime release
3. Read the wasmtime changelog between 22.0.1 and the target version
4. Identify breaking API changes that affect `crates/oxios-kernel/src/wasm_sandbox.rs`
5. If the upgrade is feasible within the scope of this brief, perform it
6. If the upgrade is too large (major API rewrite), write a separate
   upgrade plan in `docs/production-audit/02-security/WASMTIME-UPGRADE-PLAN.md`
   with step-by-step migration instructions

**CRITICAL:** If wasmtime upgrade requires extensive changes, do NOT
attempt it here. Instead, add the mitigation: ensure `wasm-sandbox`
feature is **off by default** and clearly documented as unsafe until
upgraded. Check if the default features include it.

### Phase 3: unsafe & Command Audit

For each `unsafe` block:
1. Read the surrounding code
2. Verify the safety invariant is documented
3. If not documented, add a `// SAFETY:` comment
4. If the invariant is unclear or potentially violated, flag it

For each `Command::new` call:
1. Trace the input — is it user-controlled?
2. Verify it goes through AccessManager/ExecTool sanitization
3. Verify the allowlist is enforced
4. Flag any path where user input reaches `Command::new` without
   going through the structured execution gate

### Phase 4: Security Posture Document

Write `docs/production-audit/02-security/SECURITY-POSTURE.md`:

```markdown
# Oxios Security Posture — 2026-05-31

## Vulnerability Summary
| ID | Severity | Status | Action Required |
|----|----------|--------|-----------------|

## Attack Surface
- Exec tool (structured vs shell mode)
- WASM sandbox (feature-gated)
- MCP client (stdio transport)
- Web API (localhost only, CORS configured)
- Telegram bot token (env var)

## Security Controls
- RBAC (AccessManager + AccessGate)
- Path sandboxing
- Command allowlist
- Audit trail (Merkle chain)
- Budget enforcement

## Recommendations
...
```

---

## Constraints

- **Do not** downgrade or remove features to avoid vulnerabilities
- **Do not** add new security dependencies (no new crates)
- **Do not** change the default feature set unless the vulnerability
  is reachable in default configuration
- **Do not** modify the AccessManager architecture — it was recently
  redesigned (RFC-015)
- **Preserve** the principle that shell mode is off by default

## Verification

1. `cargo audit` — verify critical findings are resolved or documented
2. `cargo test --workspace` — all tests pass
3. `cargo check -p oxios-kernel --features wasm-sandbox` — WASM still compiles
