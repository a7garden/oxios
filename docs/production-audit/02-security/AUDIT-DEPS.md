# Dependency Vulnerability Audit — 2026-05-31

**Tool:** `cargo audit` (advisory db: 1099 advisories)  
**Scope:** 881 crate dependencies in Cargo.lock  
**Findings:** 18 vulnerabilities + 6 warnings

---

## Classification Legend

| Class | Meaning |
|-------|---------|
| **REACHABLE** | Vulnerable code path is active in default build or a non-optional dependency |
| **CONDITIONAL** | Reachable only when an optional feature is enabled |
| **UNREACHABLE** | Dev-dependency only, or the vulnerable API surface is not used |

---

## Vulnerabilities

### 🔴 Critical

| # | ID | Severity | Crate | Version | Classification |
|---|----|----------|-------|---------|----------------|
| 1 | RUSTSEC-2026-0096 | **9.0 Critical** | wasmtime | 22.0.1 | **CONDITIONAL** |
| 2 | RUSTSEC-2026-0095 | **9.0 Critical** | wasmtime | 22.0.1 | **CONDITIONAL** |

**Details:**

1. **RUSTSEC-2026-0096** — Miscompiled guest heap access enables sandbox escape on aarch64 Cranelift.
   - **Path:** `oxios-kernel → wasmtime (feature: wasm-sandbox)`
   - **Condition:** Only reachable when `--features wasm-sandbox` is enabled. The feature is **NOT** in the default feature set.
   - **Impact:** On ARM64 hosts with `wasm-sandbox` enabled, a malicious WASM module could escape the sandbox.
   - **Fix:** Upgrade to wasmtime ≥ 36.0.7 or ≥ 42.0.2 or ≥ 43.0.1.

2. **RUSTSEC-2026-0095** — Winch compiler backend allows sandbox-escaping memory access.
   - **Path:** Same as above.
   - **Condition:** Only with `wasm-sandbox` feature. Oxios enables `cranelift` feature, not `winch`. However, wasmtime may still use Winch internally for some targets.
   - **Impact:** Memory access outside sandbox bounds.
   - **Fix:** Same as above.

### 🟡 High

| # | ID | Severity | Crate | Version | Classification |
|---|----|----------|-------|---------|----------------|
| 3 | RUSTSEC-2026-0149 | **7.5 High** | wasmtime-wasi | 22.0.1 | **CONDITIONAL** |

**Details:**

3. **RUSTSEC-2026-0149** — WASI `path_open(TRUNCATE)` bypasses `FilePerms::WRITE` host restriction.
   - **Path:** `oxios-kernel → wasmtime-wasi (feature: wasm-sandbox)`
   - **Condition:** Only with `wasm-sandbox` feature.
   - **Impact:** A WASM guest could truncate a file without write permission.
   - **Fix:** Upgrade to wasmtime-wasi ≥ 24.0.9 or ≥ 36.0.10 or ≥ 44.0.2.

### 🟡 Medium

| # | ID | Severity | Crate | Version | Classification |
|---|----|----------|-------|---------|----------------|
| 4 | RUSTSEC-2026-0020 | 6.9 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 5 | RUSTSEC-2026-0021 | 6.9 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 6 | RUSTSEC-2026-0087 | 4.1 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 7 | RUSTSEC-2026-0089 | 5.9 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 8 | RUSTSEC-2026-0091 | 6.1 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 9 | RUSTSEC-2026-0092 | 5.9 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 10 | RUSTSEC-2026-0093 | 6.9 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 11 | RUSTSEC-2026-0094 | 6.1 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 12 | RUSTSEC-2026-0085 | 5.6 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 13 | RUSTSEC-2023-0071 | 5.9 | rsa | 0.9.10 | **REACHABLE** |

**Details:**

- **#4–#12** (wasmtime): All are guest-triggered panics, DoS, OOB reads, or data leakage. All are gated behind `wasm-sandbox` feature. Not reachable in default build.
- **#13 RUSTSEC-2023-0071** — Marvin timing attack on RSA decryption.
  - **Path:** `oxi-ai → rsa 0.9.10`
  - **Reachable:** Yes. `oxi-ai` is a transitive dependency of `oxi-sdk`, which is used in the default build.
  - **Impact:** Side-channel timing attack could recover RSA private keys. However, this is only relevant if the system performs RSA decryption with user-controlled ciphertext. In Oxios, `oxi-ai` uses this for API authentication — the attack surface is minimal (no user-controlled ciphertext flows through RSA decryption).
  - **Fix:** No upstream fix available. Monitor RUSTSEC-2023-0071 for updates.

### 🟢 Low

| # | ID | Severity | Crate | Version | Classification |
|---|----|----------|-------|---------|----------------|
| 14 | RUSTSEC-2026-0086 | 2.3 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 15 | RUSTSEC-2025-0118 | 1.8 | wasmtime | 22.0.1 | **CONDITIONAL** |
| 16 | RUSTSEC-2024-0438 | — | wasmtime | 22.0.1 | **CONDITIONAL** |
| 17 | RUSTSEC-2025-0046 | 3.3 | wasmtime | 22.0.1 | **CONDITIONAL** |

All wasmtime-related. All gated behind `wasm-sandbox` feature. Not reachable in default build.

### ⚠️ Unsound Warnings

| # | ID | Crate | Version | Classification |
|---|----|-------|---------|----------------|
| 18 | RUSTSEC-2026-0002 | lru | 0.10.1 | **UNREACHABLE** |
| 19 | RUSTSEC-2024-0442 | wasmtime-jit-debug | 22.0.1 | **CONDITIONAL** |

- **lru (RUSTSEC-2026-0002):** The unsound `IterMut` API is not used. Oxios only uses `LruCache` basic operations in `memory/embedding_cache.rs`. **Not affected.**
- **wasmtime-jit-debug:** Only linked when `wasm-sandbox` feature is enabled. The dump functionality requires explicit opt-in. **Not affected in production.**

### 📦 Unmaintained Warnings

| # | ID | Crate | Impact |
|---|----|-------|--------|
| 20 | RUSTSEC-2025-0141 | bincode 1.3.3 | Via `llama-gguf`. No known vulnerabilities, just unmaintained. |
| 21 | RUSTSEC-2025-0057 | fxhash 0.2.1 | Via `scraper → oxi-agent` and `wasmtime`. No known vulnerabilities. |
| 22 | RUSTSEC-2025-0119 | number_prefix 0.4.0 | Via `indicatif`. No known vulnerabilities. |
| 23 | RUSTSEC-2024-0436 | paste 1.0.15 | Via `wasmtime` and others. No known vulnerabilities. |

These are informational — no security fix needed, but track for future deprecation.

---

## Summary

| Category | Count | Default-build risk |
|----------|-------|--------------------|
| Critical | 2 | ❌ None (wasm-sandbox gated) |
| High | 1 | ❌ None (wasm-sandbox gated) |
| Medium | 10 | ⚠️ 1 reachable (rsa timing, minimal impact) |
| Low | 4 | ❌ None |
| Unsound | 2 | ❌ None (unused API / feature-gated) |
| Unmaintained | 4 | ℹ️ Informational |

**Default build risk: LOW** — Only RUSTSEC-2023-0071 (rsa) is reachable, with minimal practical impact for this codebase.

**wasm-sandbox build risk: CRITICAL** — Enabling `wasm-sandbox` exposes 16 vulnerabilities including two critical sandbox escapes. **Do not enable in production until wasmtime is upgraded.**
