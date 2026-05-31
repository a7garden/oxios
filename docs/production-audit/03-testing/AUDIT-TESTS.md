# Test Audit — Ignored, Skipped, and Doc-test Analysis

**Date:** 2026-05-31  
**Scope:** Full workspace  
**Method:** `grep -rn '#\[ignore\]'` + `grep -rn '```ignore'` + `cargo test --doc -p oxios-kernel`

---

## 1. Summary

| Category | Count |
|----------|-------|
| `#[ignore]` integration/unit tests | 5 |
| `#[ignore = "..."]` embedding tests | 2 |
| ` ```ignore` / ` ```rust,ignore` doc-tests (pre-audit) | 11 |
| **Total ignored/skipped** | **18** |

**After Phase 3 fixes:**  
- **Doc-tests fixed:** 11 → 0 ignored (all now `no_run` — compile-checked but not executed)
- **All 1,169 workspace tests pass.** 6 remain `#[ignore]` (LLM-dependent + model-download-dependent).
- **16 doc-tests all compile** (was 5 passing + 11 ignored, now 16 passing + 0 ignored).

---

## 2. `#[ignore]` Tests — Detailed Analysis

### 2.1 `tests/e2e_real_pipeline.rs`

| Test | Line | Reason Ignored | Can Mock? | Recommendation |
|------|------|---------------|-----------|----------------|
| `test_full_interview_to_seed` | 42 | Requires real LLM API key + `OXIOS_E2E=1` env var | No — purpose is real LLM integration validation | **Keep ignored.** Manual-only E2E validation. |
| `test_evaluate_with_cache` | 68 | Requires real LLM API key + `OXIOS_E2E=1` env var | Partially — the cache-hit path could be tested with `EvalCache` directly without LLM | **Split:** Extract cache-roundtrip test into a non-ignored unit test in `oxios-ouroboros`. Keep the real-LLM version ignored. |

### 2.2 `crates/oxios-ouroboros/tests/scenario_test.rs`

| Test | Line | Reason Ignored | Can Mock? | Recommendation |
|------|------|---------------|-----------|----------------|
| `test_interview_scenarios` | 247 | Requires real LLM API key (z.ai provider from `~/.oxi/auth.json`) | No — purpose is LLM classification accuracy benchmark | **Keep ignored.** QA benchmark, not regression test. |

### 2.3 `crates/oxios-kernel/src/embedding/gguf/mod.rs`

| Test | Line | Reason Ignored | Can Mock? | Recommendation |
|------|------|---------------|-----------|----------------|
| `test_embed_produces_dense_vector` | 292 | Requires 329MB model download | No — purpose is real embedding model validation | **Keep ignored.** |
| `test_embed_korean` | 314 | Requires same 329MB model download | No | **Keep ignored.** |

---

## 3. Doc-test Fixes Applied (Phase 3)

### Before → After

| # | File | Before | After | Fix Applied |
|---|------|--------|-------|-------------|
| 1 | `access_manager/gate.rs` | `` ```rust,ignore `` | `` ```no_run`` | Rewrote with correct imports, simplified to show API usage only |
| 2 | `access_manager/mod.rs` | `` ```rust,ignore `` | `` ```no_run`` | Fixed: replaced non-existent methods (`assign_workspace`, `can_access_path_in_workspace`) with actual API (`can_use_tool`, `can_access_path`, `AgentPermissions` builder) |
| 3 | `capability/resolve.rs` | `` ```ignore`` | `` ```no_run`` | Added import + uses real public API |
| 4 | `clawhub/mod.rs` | `` ```ignore`` | `` ```no_run`` | Added imports, `async fn` wrapper for `.await` |
| 5 | `coordination.rs` | `` ```ignore`` | `` ```no_run`` | Fixed: replaced partial `WorkItem` construction with comment |
| 6 | `cron.rs` | `` ```ignore`` | `` ```no_run`` | Added imports, `async fn` wrapper for `.await` |
| 7 | `engine.rs` | `` ```ignore`` | `` ```no_run`` | Added import, kept API key example (no_run = compile only) |
| 8 | `observability.rs` | `` ```ignore`` | `` ```no_run`` | Fixed: replaced non-existent `tool_call` with `tool_execution` (5 args) |
| 9 | `state_store.rs` | `` ```ignore`` | `` ```no_run`` | Added import |
| 10 | `tools/registration.rs` | `` ```ignore`` | `` ```no_run`` | Fixed imports, commented out `register_tools_from_cspace` (needs `Kernel`) |
| 11 | `tools/retrieval.rs` | `` ```ignore`` | `` ```no_run`` | Fixed: removed `.await` in non-async context, added `async fn` wrapper |

### Key API errors found and fixed in doc-tests:
- `AuditEntry::tool_call()` → `AuditEntry::tool_execution()` (5 args, not 3)
- `AccessManager::assign_workspace()` — does not exist (removed from doc)
- `AccessManager::can_access_path_in_workspace()` — does not exist (replaced with `can_access_path()`)
- `WorkItem { .. }` — partial struct construction not possible in doc-test (commented out)
- `.await` in non-async context — wrapped in `async fn example()`

---

## 4. Additional Fix: `browser_tool.rs` Polyfill

The `floor_char_boundary()` method is unstable as of Rust 1.88 (feature `round_char_boundary`).  
**Fix:** Replaced with a stable polyfill function in `browser_tool.rs`:

```rust
fn floor_char_boundary(s: &str, max_len: usize) -> usize {
    if max_len >= s.len() { return s.len(); }
    let mut i = max_len;
    while !s.is_char_boundary(i) { i -= 1; }
    i
}
```

This is a pre-existing bug — `floor_char_boundary` was used without a nightly feature gate.

---

## 5. Gap Analysis — Critical Missing Test Coverage

### 5.1 Kernel Execution Path (ZERO coverage)

The primary user-facing code path:
```
Kernel::execute_prompt_with_session() → Orchestrator → AgentRuntime → Tool calls → Response
```

This has **no automated test**. The existing tests cover subsystems individually but not the full assembly. See INTEGRATION-TEST-DESIGN.md for designed tests.

### 5.2 EvalCache (testable without LLM)

The mechanical_pass evaluation path and EvalCache roundtrip can be tested without any LLM.

### 5.3 AgentRuntime Tool Execution

No test verifies tool registration → tool dispatch → tool result handling with a mock tool.

---

## 6. Test Count Breakdown

| Crate | Unit Tests | Integration Tests | Doc Tests | Ignored |
|-------|-----------|-------------------|-----------|---------|
| oxios-kernel | ~695 | 27 (e2e_test + integration_tests) | 16 (0 ignored) | 2 (embedding) |
| oxios-markdown | ~200 | 0 | 0 | 0 |
| oxios-ouroboros | ~39 | 1 (scenario_test.rs, ignored) | 0 | 1 |
| oxios-gateway | 0 | 1 (gateway_test.rs) | 0 | 0 |
| oxios (binary) | 0 | 15 (e2e_kernel.rs + e2e_real_pipeline.rs) | 0 | 2 |
| **Total** | **~934** | **~44** | **16** | **5** |

**Grand total: 1,169 passing, 0 failing, 6 ignored.**

---

## 7. Verification Results

```
$ cargo test --workspace
→ 1,169 passed; 0 failed; 6 ignored

$ cargo test --doc -p oxios-kernel
→ 16 passed; 0 failed; 0 ignored
```
