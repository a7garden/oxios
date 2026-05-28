# Oxios Dependency & Build Configuration Analysis

**Date:** 2026-05-28  
**Workspace:** 10 crates (9 workspace members + 1 stray)  
**Result:** `cargo check --workspace` passes with 45 warnings (no errors)

---

## 1. Version Inconsistencies

### 1.1 Crate Version Mismatch

| Crate | Version | Expected |
|-------|---------|----------|
| `oxios-mcp` | **0.1.0** | 0.4.0 |
| `oxios-bench` | **0.1.2** | 0.4.0 |

All other crates are aligned at `0.4.0`. `oxios-mcp` and `oxios-bench` are at pre-release versions, which may be intentional (newer/leaf crates). However, this should be documented or aligned if they share the same release cycle.

### 1.2 `dirs` Version Split (⚠️ Actual duplicate at runtime)

| Crate | `dirs` version |
|-------|---------------|
| `oxios` (root) | `5` |
| `oxios-kernel` | **`6`** |
| `oxios-web` | `5` |

`oxios-kernel` uses `dirs = "6"` while the root binary and `oxios-web` use `dirs = "5"`. This pulls in **two different versions** of `dirs` (and transitively `dirs-sys`), increasing compile time and binary size. `dirs v6` is a breaking change from v5 — both should use the same version.

**Recommendation:** Decide on `dirs = "5"` or `dirs = "6"` workspace-wide and add it to `[workspace.dependencies]`.

### 1.3 `zip` Version Disagreement

| Crate | `zip` version |
|-------|--------------|
| `oxios` (root) | `"2"` |
| `oxios-kernel` | `"2.2"` |
| `oxios-web` | `"2"` |

`"2"` and `"2.2"` resolve to the same version currently (both pull `zip 2.4.2`), but `oxios-kernel` pins to `2.2` while others use `2`. Should be unified to workspace dep for consistency.

### 1.4 `serde_json` / `toml` Not Using Workspace in All Crates

| Crate | Issue |
|-------|-------|
| `oxios-mcp` | `serde_json = "1"` instead of `workspace = true` |

The `oxios-mcp` crate specifies `serde_json = "1"` directly instead of using `workspace = true`. While this resolves to the same version, it should use the workspace reference for consistency and single-point-of-update.

---

## 2. Duplicate Dependencies (Different Versions)

From `cargo tree --duplicates`, **94 duplicate version entries** exist. Most are transitive (from different dependency trees). The significant **direct** duplicates:

| Dependency | Versions | Source |
|-----------|----------|--------|
| `dirs` | 5.0.1, 6.0.0 | root+web vs kernel |
| `constant_time_eq` | 0.3.1, 0.4.2 | zip vs blake3 |
| `core-foundation` | 0.9.4, 0.10.1 | system-configuration vs security-framework |
| `crossterm` | 0.28.1, 0.29.0 | reedline vs inquire |
| `derive_more` | 0.99.20, 2.1.1 | selectors/scraper vs crossterm |
| `getrandom` | 0.2.17, 0.3.4, 0.4.2 | multiple transitive paths |
| `thiserror` | 1.0.69, 2.0.18 | workspace pins v1, some transitive deps use v2 |
| `toml_edit` | 0.22.27, 0.25.11 | root uses 0.22, transitive pulls 0.25 |
| `itertools` | 0.10.5, 0.12.1, 0.13.0, 0.14.0 | 4 versions from transitive deps |
| `hashbrown` | 0.13.2, 0.14.5, 0.15.5, 0.16.1, 0.17.1 | 5 versions (all transitive) |
| `zip` | 2.4.2, 3.0.0 | direct vs transitive |
| `rand` | 0.8.6, 0.9.4 | markdown uses 0.8, transitive pulls 0.9 |
| `rustix` | 0.38.44, 1.1.4 | two major versions |
| `flate2` | 1.1.9 (×2 backends) | zlib-rs vs miniz_oxide |

**Transitive duplicates are expected** and largely unavoidable without forcing unified versions (which risks breakage). The key actionable item is the `dirs` split (see §1.2).

---

## 3. Circular Dependencies

**No circular dependencies detected.** The internal dependency graph is a clean DAG:

```
oxios-mcp ──────────────────────────────────────┐
oxios-markdown ──────────────────────────────┐   │
oxios-ouroboros ─────────────────────────┐   │   │
                                          ▼   ▼   ▼
oxios-kernel ──→ oxios-ouroboros + oxios-markdown + oxios-mcp
     ▲
     │
oxios-gateway ──→ oxios-kernel
     ▲
     │
oxios-web ──→ oxios-gateway + oxios-kernel + oxios-markdown + oxios-ouroboros
oxios-cli ──→ oxios-gateway + oxios-kernel
oxios-telegram ──→ oxios-gateway + oxios-kernel

oxios (binary) ──→ oxios-kernel + oxios-markdown + oxios-ouroboros + oxios-gateway
                    + oxios-web* + oxios-cli* + oxios-telegram*
```

All edges point downward. No back-edges.

---

## 4. Feature Flag Issues

### 4.1 `sqlite-memory` — Used in Code but Missing from Binary's Features (⚠️ Bug)

`src/kernel.rs` (lines 492, 625) uses `#[cfg(feature = "sqlite-memory")]` but the **root `Cargo.toml`** does not define `sqlite-memory` as a feature. It only defines: `browser`, `cli`, `default`, `telegram`, `web`.

The feature **is** defined in `crates/oxios-kernel/Cargo.toml`:
```toml
sqlite-memory = ["dep:rusqlite", "dep:sqlite-vec"]
```

And it's in the kernel's default features: `default = ["browser", "sqlite-memory"]`.

However, the root binary depends on the kernel with `default-features = false`:
```toml
oxios-kernel = { version = "0.4.0", path = "crates/oxios-kernel", default-features = false }
```

This means **the `sqlite-memory` feature is never enabled** and the SQLite code in `kernel.rs` is dead code. The `cfg` condition silently evaluates to false.

**Recommendation:** Either:
- Add `sqlite-memory` as a feature in the root `Cargo.toml` and forward it: `sqlite-memory = ["oxios-kernel/sqlite-memory"]`
- Or remove the `default-features = false` and let the kernel's defaults apply
- Or remove the `sqlite-memory` code if it's intentionally disabled

### 4.2 `native-browser` — Used in Code but Missing from Kernel Features (⚠️ Warning)

`crates/oxios-kernel/src/lib.rs:277` uses `#[cfg(feature = "native-browser")]` but this feature is not defined in `oxios-kernel/Cargo.toml`. The kernel only defines: `browser`, `sqlite-memory`, `otel`, `wasm-sandbox`, `embedding-gguf`.

**Recommendation:** Either add `native-browser` as a feature in `oxios-kernel/Cargo.toml` or remove the dead code.

### 4.3 `browser` Feature is Empty

Both `oxios-kernel/Cargo.toml` and root `Cargo.toml` define `browser = []` as a feature with no dependencies. It's used only as a cfg gate. This is fine but worth noting — it could be removed if no longer needed, or should gate actual browser dependencies.

---

## 5. Unused/Missing Dependencies

### 5.1 Unused Imports (from `cargo check`)

| File | Line | Unused Item |
|------|------|-------------|
| `crates/oxios-kernel/src/agent_runtime.rs` | 36 | `register_tools_from_cspace` |
| `crates/oxios-kernel/src/agent_runtime.rs` | 38 | `crate::config::ExecConfig` |
| `crates/oxios-kernel/src/orchestrator.rs` | 735 | variable `session_id` |

### 5.2 Dead Code (fields never read)

| File | Item |
|------|------|
| Unknown struct | field `session_context` is never read |
| Unknown struct | field `eval_cache_enabled` is never read |

### 5.3 Non-Workspace Dependencies That Should Be Unified

The following dependencies are repeated across crates without using `[workspace.dependencies]`:

| Dependency | Crates specifying directly | Recommendation |
|-----------|---------------------------|----------------|
| `reqwest` | root, kernel, telegram, web, bench (5 crates!) | Add to workspace deps |
| `zip` | root, kernel, web | Add to workspace deps |
| `dirs` | root, kernel, web | Add to workspace deps |
| `inquire` | root, kernel | Add to workspace deps |
| `console` | root, kernel | Add to workspace deps |
| `libc` | root, kernel | Add to workspace deps |
| `clap` | root, bench | Add to workspace deps |
| `tempfile` | root, gateway, kernel (×2), markdown, bench (6 places!) | Add to workspace deps |
| `once_cell` | kernel, markdown | Add to workspace deps |
| `glob` | kernel | Low priority (single use) |
| `toml_edit` | root | Low priority (single use) |
| `serde_yaml` | kernel | Low priority (single use) |
| `serde_json` | mcp (uses `"1"` instead of workspace) | Fix to workspace ref |

### 5.4 Stray `hello_world/` Crate

A `hello_world/` directory exists at the workspace root containing a trivial Rust binary:
```rust
fn main() {
    println!("Hello, World!");
}
```

It is **not** a workspace member (not listed in `members = [...]`), so it doesn't affect builds. However, it appears to be a leftover artifact that should be cleaned up.

---

## 6. Missing Documentation Warnings

41 of 45 warnings are `missing documentation` from `oxios-kernel`, primarily:
- 34 × missing documentation for struct fields (mostly in `access_manager/audit_sink.rs`)
- 1 × missing documentation for a constant (`onboarding.rs:23`)
- 1 × missing documentation for a method

These are non-blocking but should be addressed since the crate has `#![warn(missing_docs)]`.

---

## Summary of Actionable Items

| Priority | Issue | Impact |
|----------|-------|--------|
| 🔴 High | `sqlite-memory` feature disabled by `default-features = false` | SQLite memory storage is dead code |
| 🔴 High | `dirs` version split (5 vs 6) | Binary bloat, duplicate runtime deps |
| 🟡 Medium | `native-browser` feature undefined | Dead code path in kernel |
| 🟡 Medium | `reqwest` not in workspace deps (5 separate specs) | Maintenance burden, feature drift risk |
| 🟡 Medium | Unused imports in `agent_runtime.rs` | Code hygiene |
| 🟡 Medium | `oxios-mcp` serde_json not using workspace | Consistency |
| 🟢 Low | `hello_world/` stray directory | Cleanup |
| 🟢 Low | 34 missing-doc warnings in `audit_sink.rs` | Documentation debt |
| 🟢 Low | Transitive version duplicates (hashbrown, itertools, etc.) | Unavoidable; only fix if problematic |
| ℹ️ Info | `oxios-mcp` (0.1.0) and `oxios-bench` (0.1.2) version mismatch | Possibly intentional |
