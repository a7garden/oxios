# Fix Medium Issues — Findings & Changes

## Issue #6: Audit trail rehashes entire chain on prune ✅

**File:** `crates/oxios-kernel/src/audit_trail.rs`

### Problem
When auto-prune triggers in `append_with_meta()` and `restore_from()`, it drains excess entries and then recomputes hashes for ALL remaining entries in O(N), cascading because each recomputed hash changes the prev_hash reference for the next entry.

### Root Cause
The old code did:
1. Set `first.prev_hash = "pruned"` 
2. Recompute `first.hash` (which changes it)
3. For each subsequent entry, check if `prev_hash` changed → if so, recompute (which cascades)

This is O(N) with N hash computations.

### Fix Applied
The key insight: **we don't need to recompute any hashes**. When we drain old entries:
1. Entry[0] loses its predecessor — we just set `prev_hash = "pruned"` 
2. Entry[0]'s stored hash was computed with its original `prev_hash`, but we DON'T change it
3. Entry[1]'s `prev_hash` still points to Entry[0]'s unchanged hash — still valid
4. No cascade needed

**Changes:**
- `append_with_meta()`: Replaced O(N) rehash loop with single `first.prev_hash = "pruned"` assignment
- `restore_from()`: Same simplification
- `verify()`: Updated to skip hash recomputation for pruned first entry (since its stored hash was computed with the original prev_hash, not "pruned"). The first pruned entry is trusted with `continue` after setting `prev_hash` to its stored hash for subsequent chain validation.

**Complexity:** O(1) instead of O(N).

---

## Issue #7: Budget persistence ✅

**File:** `crates/oxios-kernel/src/budget.rs`

### Problem
Budgets are in-memory only — lost on restart. The `Instant` type is not serializable, preventing persistence.

### Fix Applied
1. **Replaced `Instant` with `DateTime<Utc>`** for `Usage.window_start` — fully serializable
2. **Added `Serialize, Deserialize` derives** to both `BudgetLimit` and `Usage` structs
3. **Updated all time comparisons:**
   - `Instant::now()` → `Utc::now()` (6 call sites)
   - `Instant::elapsed()` → `Utc::now().signed_duration_since(entry.window_start).to_std()`
   - `reset_if_expired()` now uses `chrono::Duration::seconds()` comparison
4. **Added `persist()` method** — serializes budgets + usage to JSON via a given path
5. **Added `restore()` method** — loads from JSON; returns `Ok(())` if file doesn't exist
6. **Added `PersistedBudgets` helper struct** for clean JSON serialization of the two HashMaps

---

## Issue #10: 13 parameters in run_agent_loop() ✅

**File:** `crates/oxios-kernel/src/agent_runtime.rs`

### Problem
`run_agent_loop()` had 13+ positional parameters, making it error-prone to call and maintain.

### Fix Applied
1. **Created `AgentLoopContext` struct** bundling all 13 parameters:
   - `provider`, `config`, `system_prompt`, `prompt`, `seed_id`, `agent_id`
   - `program_manager`, `oxios_config`, `mcp_bridge`, `memory_manager`
   - `exec_config`, `exec_access`, `a2a_protocol`
   - `browser_backend` (feature-gated)
2. **Changed `run_agent_loop(params...)` → `run_agent_loop(ctx: AgentLoopContext)`**
3. **Updated call site in `execute()`** to build the struct and pass it
4. **Function body destructures the context** to keep existing variable names unchanged
5. **Removed `#[allow(clippy::too_many_arguments)]`** — no longer needed

---

## Issue #14 (partial): KernelError Timeout and RateLimited variants ✅

**File:** `crates/oxios-kernel/src/error.rs`

### Fix Applied
1. **Added `Timeout { context: String }` variant** — with `#[error("Operation timed out: {context}")]`
2. **Added `RateLimited { context: String }` variant** — with `#[error("Rate limit exceeded: {context}")]`
3. **Added `TooManyRequests = 429` to `HttpStatus` enum**
4. **Added HTTP status mappings:**
   - `Timeout` → `HttpStatus::ServiceUnavailable` (503)
   - `RateLimited` → `HttpStatus::TooManyRequests` (429)
5. **Added two tests:** `test_timeout_error_status` and `test_rate_limited_error_status`

---

## Build Status

`cargo check -p oxios-kernel` passes for the 4 modified files. The crate has 2 pre-existing compilation errors in unrelated files (`supervisor.rs`: CancellationToken import, `scheduler.rs`: missing `mut`) that are not caused by these changes.
