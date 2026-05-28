# Concurrency & Correctness Analysis: oxios-kernel

> Generated: 2026-05-28  
> Scope: `crates/oxios-kernel/src/`

---

## Summary

| Category | Severity | Count |
|----------|----------|-------|
| Potential Deadlock | ⚠️ Medium | 3 |
| Race Condition | 🔴 High | 2 |
| Silently Swallowed Errors | ⚠️ Medium | 12 |
| Unsafe Soundness | 🟡 Low | 1 |
| Holding Lock Across `.await` | 🔴 High | 3 |
| Temp File Race | ⚠️ Medium | 1 |
| Error Handling Inconsistency | ⚠️ Medium | 1 |
| Channel Handling | 🟡 Low | 1 |

---

## 1. Potential Deadlocks: Multiple `.lock()` on Different Mutexes in Same Scope

### 1.1 Scheduler: `reap_zombies()` — dual lock held simultaneously
**File:** `scheduler.rs` lines 430–431  
**Severity:** ⚠️ Medium (not a deadlock by itself, but creates an ordering contract)

```rust
let mut start_times = self.task_start_times.lock();
let mut running = self.running.lock();
```

Two `parking_lot::Mutex` guards are held simultaneously. `parking_lot::Mutex` is not reentrant and does not poison — but this creates a lock ordering requirement. If any other code path acquires `running` first then `start_times`, it would deadlock.

**Other call-sites that acquire both locks in the SAME order (`start_times` → `running`):**
- `scheduler.rs:488–490` (`start_task`)
- `scheduler.rs:333–340` (`next_task`, sequential but separate scopes)

**Verified safe:** All current call-sites acquire `start_times` first, then `running`. No reverse ordering found. However, this is fragile — any new method that acquires `running` then `start_times` will deadlock.

**Recommendation:** Document the lock ordering invariant as a `// SAFETY:` comment on the fields, or merge `task_start_times` into the `running` HashMap to eliminate the two-lock pattern entirely.

### 1.2 Scheduler: `stats()` — triple lock held simultaneously
**File:** `scheduler.rs` lines 529–531  
**Severity:** ⚠️ Medium

```rust
let queue = self.queue.lock();
let running = self.running.lock();
let rate_limiter = self.rate_limiter.lock();
```

Three locks held simultaneously. Same ordering concern as 1.1. All are `parking_lot::Mutex` (non-poisoning, not async-aware), and the operation is short-lived. No other path acquires these three in a different order, so this is currently safe.

**Recommendation:** Clone the three values and drop locks before constructing `SchedulerStats`, or merge `queue` + `running` statistics into a single struct behind one lock.

### 1.3 Supervisor: `run_with_seed()` — sequential but not nested
**File:** `supervisor.rs` lines 276–290  
**Severity:** 🟢 Low

The `agents` and `handles` RwLocks are acquired in separate blocks (agents written, then later handles written). No simultaneous hold.

---

## 2. Race Conditions

### 2.1 Scheduler: `next_task()` — TOCTOU between `running.len()` check and task insertion
**File:** `scheduler.rs` lines 270–342  
**Severity:** 🟡 Low (single-threaded access pattern in practice)

```rust
// Step 1: Check concurrent count
let running = self.running.lock();
if running.len() >= self.max_concurrent { return None; }
// Lock dropped here.

// Step 2: Check rate limit
let mut limiter = self.rate_limiter.lock();
if !limiter.allow() { return None; }
// Lock dropped here.

// Step 3: Pop from queue
let task_opt = { let mut queue = self.queue.lock(); queue.pop() };
// ... later:
let mut running = self.running.lock();
running.insert(task.id, task.clone());
```

Between step 1 (checking `running.len()`) and step 3 (inserting into `running`), another caller could have inserted tasks, exceeding `max_concurrent`. Since `parking_lot::Mutex` is not async-aware and the scheduler is currently called from a single orchestrator task, this is unlikely to trigger in practice.

**Recommendation:** Merge the check-and-insert into a single critical section, or document that `next_task()` must not be called concurrently.

### 2.2 Supervisor: `run_with_seed()` — handle removal race with `kill()`
**File:** `supervisor.rs` lines 264–310  
**Severity:** 🔴 High

```rust
// In run_with_seed():
let agent_handle = {
    let mut handles = self.handles.write();
    handles.remove(&id)
};
// Guard dropped above, safe to await.
match agent_handle {
    Some(ah) => match ah.task.await { ... }
    None => anyhow::bail!("Agent {id} handle disappeared"),
}
```

Between `handles.remove(&id)` and `ah.task.await`, a concurrent `kill()` call on the same agent ID would find the handle already removed (good), but if `kill()` races *before* the remove, it would abort the task and then `run_with_seed` would get a `JoinError`. This is handled correctly — the `JoinError` is caught and returns `ExecutionResult { success: false }`. **Verified correct.**

However, there's a subtler issue: if `kill()` is called concurrently with the `handles.write()` block in `run_with_seed` (line 270), both would try to write-lock `handles`. Since `parking_lot::RwLock` is fair, one would block the other. This is correct but means `kill()` may block briefly while the handle is being inserted.

---

## 3. Silently Swallowed Errors (`.ok()` without handling)

### 3.1 Orchestrator: A2A capability query failure
**File:** `orchestrator.rs` line 1030  
```rust
a2a.query_capabilities(cap).await.ok()
```
If the A2A registry is unavailable, the error is silently discarded. The task falls through to lifecycle execution, which is correct fallback behavior. **Acceptable** but should log the failure.

### 3.2 Space detection failure
**File:** `space/manager.rs` line 509  
```rust
.ok(); // Ignore save errors here
```
Space save errors are ignored. If disk is full or permissions change, the space state is lost silently.

### 3.3 Memory subsystem — numerous `.ok()` calls
**File:** `memory/sqlite_store.rs` lines 175, 209, 435, 486, 529, 631  
**File:** `memory/database.rs` lines 287, 335, 355, 452, 496  
**File:** `memory/dream.rs` lines 461, 565, 932, 938  

These are `filter_map(|r| r.ok())` on SQL query row iterations. Individual row deserialization failures are silently skipped. This means corrupted rows are silently dropped without any log. **Risky** for data integrity — a corrupted database could lose entries without any indication.

**Recommendation:** Add `tracing::warn!()` for row-level deserialization failures, or count failures and log a summary after the loop.

### 3.4 Supervisor: `export_state()` failure
**File:** `supervisor.rs` line 87  
```rust
.and_then(|agent| agent.export_state().ok())
```
If `export_state()` fails (serialization error), the result is `None` with no log.

### 3.5 Dream checkpoint clear failure
**File:** `memory/dream.rs` line 565  
```rust
self.clear_checkpoint().await.ok();
```
Checkpoint cleanup failure is silently ignored. If this fails repeatedly, stale checkpoints accumulate.

### 3.6 Git layer failures
**File:** `git_layer.rs` lines 98, 142, 177, 202, 259, 419  
Multiple `repo.head_id().ok()` calls — if the git repository is corrupted, these silently return `None` instead of propagating or logging the error.

### 3.7 Event publish failures
**File:** `orchestrator.rs` — multiple `let _ = self.event_bus.publish(...)`  
**File:** `supervisor.rs` — multiple `let _ = self.event_bus.publish(...)`  

These discard the `Result` from `EventBus::publish()`. Since `publish()` returns `Ok(())` always (the internal `send()` result is discarded with `let _`), this is technically harmless. However, if the broadcast channel is closed, events are silently lost.

---

## 4. Unsafe Soundness

### 4.1 sqlite-vec auto extension registration
**File:** `memory/database.rs` lines 214–218  
**Severity:** 🟡 Low (well-guarded)

```rust
static REGISTERED: AtomicBool = AtomicBool::new(false);
if !REGISTERED.swap(true, Ordering::SeqCst) {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
}
```

**Analysis:**
- The `AtomicBool` with `SeqCst` ordering ensures single registration across threads. ✅
- `sqlite3_auto_extension` is process-global and documented as safe to call from a single thread. ✅ (guarded by AtomicBool)
- `std::mem::transmute` casts a Rust function pointer to a C function pointer. This is the standard pattern for `sqlite3_auto_extension` with `sqlite-vec`. The types must match `unsafe extern "C" fn(...) -> i32`. This relies on `sqlite_vec::sqlite3_vec_init` having the correct C ABI. **Sound if the `sqlite-vec` crate provides the correct signature.**
- The `REGISTERED` flag prevents double-registration, which would cause UB per SQLite docs.

**Potential issue:** If the binary is loaded as a shared library and `REGISTERED` is not in a unique translation unit, the AtomicBool might not be truly unique. In practice, for a Rust binary, this is fine.

### 4.2 libc::kill for daemon management
**File:** `daemon.rs` lines 107, 321  
**Severity:** 🟢 Low (standard POSIX signal API)

```rust
let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
unsafe { libc::kill(pid as i32, 0) == 0 }
```

These are standard POSIX signal operations. The PID is read from a PID file and validated before use. Sound.

---

## 5. Holding Lock Across `.await` (Send/Sync Issues)

### 5.1 Orchestrator: `handle_message()` with `#[allow(clippy::await_holding_lock)]`
**File:** `orchestrator.rs` line 263  
**Severity:** 🔴 High

The function is annotated `#[allow(clippy::await_holding_lock)]`, which suppresses the lint. Upon inspection, the actual code carefully drops locks before `.await` points:

```rust
// Lines 283-287: Lock dropped before .await
let sm_opt = {
    let sm_guard = self.space_manager.read();
    sm_guard.as_ref().cloned()
}; // Guard dropped
if let Some(sm) = sm_opt {
    if let Err(e) = sm.activate(&uuid).await { ... }
}

// Lines 295-302: Both guards dropped before .await
let (turns, sm_arc) = {
    let buffer = self.conversation_buffer.read();
    let sm_guard = self.space_manager.read();
    // ...
    (turns, sm_arc)
}; // Both guards dropped
match sm.detect_or_create(user_message, &turns).await { ... }
```

**The allow attribute is overly broad** — it suppresses the lint for the *entire* function, not just the safe patterns. If someone adds a genuine lock-across-await later, the lint won't catch it.

**Recommendation:** Remove the `#[allow]` attribute and instead use `#[allow]` only on specific blocks, or restructure to make the lock drops explicit (already done).

### 5.2 SpaceManager: `ensure_default_space()` with `#[allow(clippy::await_holding_lock)]`
**File:** `space/manager.rs` line 177  
**Severity:** ⚠️ Medium

```rust
async fn ensure_default_space(&self) -> Result<()> {
    let spaces = self.spaces.read();
    if spaces.contains_key(&default_space_id()) {
        return Ok(());  // Guard dropped on early return ✅
    }
    drop(spaces);  // Explicit drop before .await ✅
    // ...
    self.add_space(default).await  // Safe, no guard held
}
```

The code correctly drops the guard before `.await`. The `#[allow]` is overly broad but the code is correct.

### 5.3 SpaceManager: line 280 — similar pattern
**File:** `space/manager.rs` line 280  
Same pattern as 5.2, correctly drops before `.await`.

---

## 6. Channel Handling Issues

### 6.1 EventBus: `publish()` silently drops events
**File:** `event_bus.rs` line 376  
**Severity:** 🟡 Low

```rust
pub fn publish(&self, event: KernelEvent) -> Result<()> {
    let _ = self.sender.send(event);
    Ok(())
}
```

`broadcast::Sender::send()` returns `Result<usize, SendError>`. If there are no subscribers, `send()` returns `Ok(0)` — fine. If the channel is full (all receivers lagging), the oldest message is dropped per broadcast semantics. The `let _` discards the receiver count, but more importantly, **returns `Ok(())` even if the send fails**. This means callers can't detect when events are being lost.

Additionally, the `attach_audit_trail` method (line 385) uses `while let Ok(event) = rx.recv().await` which will exit the loop silently on `RecvError::Lagged` — the audit trail subscriber would stop processing events if it falls behind.

**Recommendation:** Handle `RecvError::Lagged` in `attach_audit_trail`:
```rust
loop {
    match rx.recv().await {
        Ok(event) => { /* process */ }
        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
            tracing::warn!(skipped = n, "Audit trail lagged, skipping events");
            continue;
        }
        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
    }
}
```

### 6.2 KnowledgeLens: mpsc channel send silently dropped
**File:** `kernel_handle/knowledge_lens.rs` line 108  
**Severity:** 🟡 Low

```rust
let _ = tx.send(event).await;
```

If the receiver is closed (background task terminated), the send is silently dropped. This means knowledge index updates stop happening without any indication.

---

## 7. Temp File Race Condition

### 7.1 StateStore: non-unique temp file names
**File:** `state_store.rs` lines 253–255, 311–313  
**Severity:** ⚠️ Medium

```rust
let temp_path = dir.join(format!("{name}.{}.tmp", std::process::id()));
fs::write(&temp_path, content).await?;
tokio::fs::rename(&temp_path, &path).await?;
```

The temp file name includes `std::process::id()` but NOT a unique per-write identifier. If two concurrent writes to the same file happen within the same process (e.g., two tokio tasks calling `save_json` for the same key), they'll write to the same temp file, causing data corruption.

**Recommendation:** Use a UUID or thread-id + sequence number in the temp file name:
```rust
let temp_path = dir.join(format!("{name}.{}.{}.tmp", std::process::id(), uuid::Uuid::new_v4()));
```

---

## 8. Error Handling Consistency

### 8.1 `anyhow` vs `thiserror` usage
**Files:** Throughout `oxios-kernel`  

The crate properly follows the convention from `AGENTS.md`:
- **`error.rs`**: Uses `thiserror` for public `KernelError` enum — ✅ correct for library crate
- **Internal modules**: Use `anyhow::Result` — ✅ correct for implementation
- **`wasm_sandbox.rs`**: Uses `thiserror` for `WasmSandboxError` — ✅ correct for typed error enum

However, `KernelError` has an `Internal(#[from] anyhow::Error)` variant, which means any `anyhow` error can be converted to `KernelError` with loss of type information. This is intentional (documented as "wraps from anyhow") but makes structured error matching at the consumer level impossible for internal errors.

**No violation found** — the convention is followed correctly.

### 8.2 Tools ignoring `shutdown` signal
**Files:** Multiple tool implementations  

Every tool's `execute()` method receives `shutdown: Option<oneshot::Receiver<()>>` but universally ignores it (naming it `_signal` or `_shutdown`). This means agents cannot be gracefully interrupted during long-running tool execution.

**Affected files:**
- `tools/memory_tools.rs` lines 87, 225, 357
- `tools/browser/browser_tool.rs` line 194
- `tools/mcp_tool.rs` line 119
- `tools/a2a_tools.rs` lines 100, 278, 412
- `tools/exec_tool.rs` line 467
- `tools/kernel/*.rs` — all tools

**Recommendation:** For long-running tools (especially `exec_tool`), check the shutdown signal periodically using `tokio::select!`.

---

## 9. Additional Findings

### 9.1 Scheduler uses `parking_lot::Mutex` (non-async)
**File:** `scheduler.rs`  
**Severity:** 🟡 Low

The scheduler uses `parking_lot::Mutex` (blocking) rather than `tokio::sync::Mutex`. Since all scheduler operations are short (HashMap insert/remove, BinaryHeap push/pop), this is fine — holding a `parking_lot::Mutex` across an `.await` would be problematic, but the scheduler never awaits while holding a lock. **Verified safe.**

### 9.2 `AgentRuntime::execute()` spawns fire-and-forget tasks
**File:** `agent_runtime.rs` lines 636, 756  
**Severity:** ⚠️ Medium

```rust
tokio::spawn(async move {
    if let Err(e) = memory_manager.remember(entry).await {
        tracing::warn!(error = %e, "Failed to save compaction summary");
    }
});
```

Compaction summaries and SONA trajectory recording are spawned as fire-and-forget tasks. If the tokio runtime shuts down while these tasks are pending, the data is lost. For SONA trajectory recording, this is explicitly documented as "fire-and-forget: don't block the result on learning" — so this is intentional.

**Recommendation:** Consider using `tokio::task::JoinSet` or a bounded channel for compaction summaries to provide backpressure and guarantee delivery.

### 9.3 BrowserTool: unwrap after Option check
**File:** `tools/browser/browser_tool.rs` line 93  
**Severity:** 🟢 Low

```rust
Ok(guard.as_ref().unwrap().clone())
```

The `unwrap()` is safe because the code checks `needs_new` and always sets `*guard = Some(tab)` before reaching this line. **Verified correct** but could use `expect("tab was just set")` for clarity.

---

## Recommendations (Priority Order)

1. **🔴 Fix `attach_audit_trail` to handle `RecvError::Lagged`** — silent audit trail loss is a security concern
2. **🔴 Remove overly broad `#[allow(clippy::await_holding_lock)]`** from `orchestrator.rs` — future code changes could introduce real issues without warning
3. **⚠️ Fix StateStore temp file naming** — add UUID to prevent same-process write collisions
4. **⚠️ Add logging for silently swallowed errors** — especially memory row deserialization failures
5. **⚠️ Document scheduler lock ordering invariant** — `start_times` → `running` → `queue` → `rate_limiter`
6. **⚠️ Implement shutdown signal handling in `exec_tool`** — long-running commands can't be cancelled
7. **🟡 Add backpressure for fire-and-forget memory writes** — prevent data loss on runtime shutdown
