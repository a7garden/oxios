# Brief 01: Code Quality — unwrap() & Error Handling

**Area:** Core kernel modules  
**Severity:** 🔴 Critical  
**Estimated scope:** ~160 `unwrap()` calls across 6 critical files  
**Target:** Zero panics in production code paths

---

## Context

Oxios is an Agent Operating System where the kernel must never panic.
A panic in `scheduler.rs` kills the scheduling loop. A panic in
`orchestrator.rs` aborts the Ouroboros protocol mid-execution. These are
**not recoverable** — tokio catches the panic but the task is gone.

Current state:

| File | `unwrap()` count | Role |
|------|-----------------|------|
| `crates/oxios-kernel/src/scheduler.rs` | 58 | Task queue, priority scheduling |
| `crates/oxios-kernel/src/cron.rs` | 34 | Cron job scheduler |
| `crates/oxios-kernel/src/orchestrator.rs` | 29 | Ouroboros protocol brain |
| `crates/oxios-kernel/src/supervisor.rs` | 18 | Agent lifecycle (fork/exec/wait/kill) |
| `crates/oxios-kernel/src/budget.rs` | 7 | Token/cost enforcement |
| `src/kernel.rs` | 4 | Kernel assembler |
| `crates/oxios-kernel/src/access_manager/mod.rs` | 3 | RBAC gate |

Total in critical kernel modules: **~153**

Additionally, `anyhow` is used throughout (375 occurrences across the
codebase), which is correct for the binary crate. Library crates use
`thiserror` for typed errors — this split is intentional and should be
preserved.

---

## Objective

Convert every `unwrap()` in **non-test, production execution paths** to
proper error handling. This does NOT mean:

- ❌ Blindly replacing all `unwrap()` with `expect("reason")`
- ❌ Creating new error enum variants for every single case
- ❌ Splitting files into smaller modules for "cleaner" code
- ❌ Introducing new abstraction layers

It DOES mean:

- ✅ `unwrap()` on infallible operations (e.g., `Mutex::lock()` where
  the lock is never poisoned, or `Arc::downgrade().upgrade()` right
  after creation) can stay — but add a `// SAFETY:` comment explaining
  why it's infallible
- ✅ `unwrap()` on fallible operations (IO, parsing, channel sends,
  HashMap lookups) must become `?`, `ok_or_else?`, or `.unwrap_or_default()`
  depending on context
- ✅ For HashMap/BTreeMap lookups, prefer `.get()` + match over
  direct indexing when the key might be absent
- ✅ For channel operations (`send`, `recv`), handle `Err` explicitly
  (closed channel is a real production scenario)

---

## Approach

### Phase 1: Audit & Classify (read-only)

For each of the 7 files above:

1. Read the entire file
2. For each `unwrap()`, classify it:
   - **SAFE** — Infallible in this context (lock never poisoned, etc.)
   - **BENIGN** — Test-only or startup-only code where panic is acceptable
   - **ACTIONABLE** — Can fail in production and should be converted
3. Write the classification to `docs/production-audit/01-code-quality/AUDIT-RESULTS.md`
   in a table format:

```markdown
| Line | Code | Classification | Replacement |
|------|------|---------------|-------------|
| 142 | `lock().unwrap()` | SAFE | Mutex never poisoned here (single owner) |
| 289 | `map.entry(k).unwrap()` | ACTIONABLE | → `ok_or(KernelError::NotFound)?` |
```

**IMPORTANT:** Actually verify each one. Do not assume. Read the surrounding
context. A classification must be justified by the code, not by pattern
matching.

### Phase 2: Fix ACTIONABLE items

For each ACTIONABLE `unwrap()`:

1. Choose the appropriate replacement:
   - `?` — when the enclosing function already returns `Result`
   - `.ok_or_else(|| ...)?` — when converting Option to Result
   - `.unwrap_or_default()` — when a missing value has a sensible default
   - `if let Some(x) = ... { x } else { return Err(...) }` — when
     additional cleanup is needed
   - `.expect("...")` — ONLY for startup-time invariants that, if
     violated, mean the system is misconfigured and cannot run
2. If the enclosing function doesn't return `Result`, change its
   signature. Propagate the Result upward. Do NOT create intermediate
   error types — use the existing `KernelError` (in
   `crates/oxios-kernel/src/error.rs`) or `anyhow` as appropriate.
3. Run `cargo test -p oxios-kernel` after every file to catch regressions.

### Phase 3: Verify

1. `cargo test --workspace` — all tests pass
2. `cargo clippy --workspace -- -D warnings` — no new warnings
3. Grep for remaining `unwrap()` in the 7 files — only SAFE/BENIGN should remain
4. Write summary to `docs/production-audit/01-code-quality/RESULT.md`

---

## Constraints

- **Do not** restructure module boundaries or move files
- **Do not** add new crate dependencies
- **Do not** change public API signatures unless the return type changes
  from `T` to `Result<T, E>` — in that case, update all call sites
- **Do not** modify test code (tests can keep their `unwrap()`)
- **Preserve** the `anyhow` for binary crate, `thiserror` for library
  crates convention

## Clippy Cleanup (secondary)

Also fix these while you're in the files:
- 30 instances of variables that can be inlined into `format!()` strings
  (e.g., `format!("{}", var)` → `format!("{var}")`)
- 6 unused import/dead code warnings in root `kernel.rs`

These are mechanical fixes. Do them last, file by file.

## References

- `crates/oxios-kernel/src/error.rs` — existing `KernelError` enum
- `docs/ARCHITECTURE.md` — kernel architecture overview
- `AGENTS.md` — project conventions (anyhow for apps, thiserror for libs)
