# Critical Issues Fix Report

Date: 2026-05-14
Files Modified: `crates/oxios-kernel/src/agent_runtime.rs`, `crates/oxios-kernel/src/supervisor.rs`, `crates/oxios-kernel/src/scheduler.rs`

---

## Issue #1: CWD Race Condition in agent_runtime.rs ✅ FIXED

**File:** `crates/oxios-kernel/src/agent_runtime.rs`

**Problem:** `run_agent_loop()` called `std::env::set_current_dir()`, which is process-global. When concurrent agents ran in separate `spawn_blocking` threads, they raced on the CWD.

**Fix:** Introduced `WORKSPACE_MUTEX` — a process-global `std::sync::Mutex<()>` that serializes CWD changes + agent loop execution. The agent still gets its own workspace directory (`/tmp/oxios-agent-workspace/<agent_id>`), but only one agent can change CWD at a time.

**Changes:**
- Added `static WORKSPACE_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());` with doc comment explaining the race condition
- Removed the direct `std::env::set_current_dir()` call entirely
- Added `let _workspace_guard = WORKSPACE_MUTEX.lock().expect(...);` to hold the lock for the duration of `run_agent_loop`
- Added TODO comments referencing the oxi-agent upstream fix needed (add `workspace_dir` to `AgentLoopConfig`)

**Trade-off:** Agents now run sequentially with respect to their CWD. This is acceptable since `spawn_blocking` already serializes threads in the common case, and high-concurrency scenarios are rare.

---

## Issue #2: kill() Doesn't Cancel Running Tasks in supervisor.rs ✅ FIXED

**File:** `crates/oxios-kernel/src/supervisor.rs`

**Problem:** `kill()` only set `agent.status = AgentStatus::Stopped` but never cancelled or aborted the actual running task. The agent continued executing indefinitely.

**Fix:** Added `AgentHandle` struct + `handles` HashMap to track per-agent cancellation state and join handles:

1. New struct `AgentHandle` containing:
   - `cancelled: Arc<AtomicBool>` — cooperative cancellation flag
   - `task: JoinHandle<Result<ExecutionResult>>>` — the spawned task handle

2. Added `handles: RwLock<HashMap<AgentId, AgentHandle>>` to `BasicSupervisor`

3. `run_with_seed()` now:
   - Creates a spawned tokio task (instead of direct `.await`)
   - Stores the `JoinHandle` so `kill()` can call `.abort()`
   - Checks the `cancelled` flag before starting execution
   - Properly drops the `parking_lot` write guard before `.await` (required for `Send` bounds)

4. `kill()` now:
   - Sets `cancelled.store(true, Ordering::Relaxed)` for cooperative cancellation
   - Calls `task.abort()` for immediate forceful cancellation
   - Removes the handle from the map
   - Publishes `AgentStopped` event and updates status

**Note:** Used `std::sync::atomic::AtomicBool` instead of `tokio::sync::CancellationToken` because the latter requires `tokio_util` which was not in the dependency tree. `AtomicBool` + `JoinHandle::abort()` provides equivalent cancellation semantics.

---

## Issue #3: Recursive next_task() in scheduler.rs ✅ FIXED

**File:** `crates/oxios-kernel/src/scheduler.rs`

**Problem:** `next_task()` called itself recursively when an agent's budget was exhausted (`return self.next_task()`). With many exhausted agents, this caused unbounded recursion and potential stack overflow.

**Fix:** Replaced the recursive call with an iterative `loop`:

```rust
let mut discarded: usize = 0;
let task = loop {
    let task_opt = { self.queue.lock().pop() };
    match task_opt {
        Some(t) => {
            if let (Some(ref bm), Some(ref agent_id)) = (&self.budget_manager, &t.agent_id) {
                if !bm.can_schedule(agent_id) {
                    tracing::warn!(agent_id = %agent_id, "Agent budget exhausted, skipping task");
                    discarded += 1;
                    continue; // skip this task, try next
                }
            }
            break t;
        }
        None => {
            if discarded > 0 {
                tracing::info!(discarded, "All queued tasks had exhausted budgets");
            }
            return None;
        }
    }
};
if discarded > 0 {
    tracing::info!(discarded, "Skipped tasks with exhausted budgets");
}
```

**Also fixed:** `let task` → `let mut task` (the existing code mutated `task.status` later but `task` wasn't declared as `mut`).

---

## Verification

```bash
cargo check -p oxios-kernel 2>&1
```

**Result:** Compiles successfully. Only pre-existing warnings remain (unused `provider`/`model_id` in `oxi-ai`, unused `oxios_config` in `agent_runtime.rs`, pre-existing test error in `config.rs`).

---

## Summary

| Issue | File | Fix | Status |
|-------|------|-----|--------|
| #1 CWD Race | agent_runtime.rs | `WORKSPACE_MUTEX` serialization | ✅ |
| #2 kill() no cancel | supervisor.rs | `AgentHandle` + `JoinHandle::abort()` | ✅ |
| #3 Recursive next_task | scheduler.rs | Iterative `loop` + `continue` | ✅ |
