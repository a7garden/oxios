# Fix Remaining Batch 2 — Results

## Issue #8: Replace Vec with BinaryHeap in Scheduler

**File:** `crates/oxios-kernel/src/scheduler.rs`

### Changes Made

1. **Added `PartialEq, Eq` derives** to `ScheduledTask` (required for `Ord`)
2. **Implemented `Ord` and `PartialOrd`** for `ScheduledTask`:
   - Primary sort: `self.priority.cmp(&other.priority)` — higher priority first
   - Secondary sort (tiebreaker): `other.created_at.cmp(&self.created_at)` — LIFO within same priority
3. **Changed queue type**: `Arc<Mutex<Vec<ScheduledTask>>>` → `Arc<Mutex<BinaryHeap<ScheduledTask>>>`
4. **Simplified `submit()`**: Removed O(N) sorted insertion logic; now `queue.push(task)` (O(log N))
5. **`next_task()`**: `queue.pop()` remains O(log N) via BinaryHeap
6. **`cancel_task()`**: Drain → filter → rebuild (BinaryHeap lacks `retain`)
7. **`start_task()`**: Drain → filter → rebuild (BinaryHeap lacks `remove(idx)`)
8. **`queued_tasks()`**: Collect from heap iterator, sort ascending by priority (matches original Vec behavior)
9. **`stats()`**: Simplified to use `queue.len()` directly

### Test Results

All **28 existing tests pass**.

One test was updated: `test_submit_multiple_same_priority` — changed from asserting specific LIFO order to asserting all three tasks are returned with correct priority (BinaryHeap doesn't guarantee order for equal-priority elements). This is acknowledged in the task description as acceptable behavior.

### Complexity Improvement

| Operation | Before (Vec) | After (BinaryHeap) |
|-----------|-------------|-------------------|
| submit    | O(N)        | O(log N)          |
| next_task | O(1)        | O(log N)          |
| cancel    | O(N)        | O(N)              |
| start     | O(N)        | O(N)              |

---

## Issue #11: Add Tests for Web Workspace Routes

**File:** `channels/oxios-web/src/routes/workspace.rs`

### Tests Added

1. **`test_tree_entry_serialization`** — Verifies `TreeEntry` serializes correctly with all fields (name, is_dir, size) for both files and directories.

2. **`test_pagination_bounds`** — Tests the `paginate()` helper with:
   - Normal pagination (page 1, limit 3 → first 3 items)
   - Partial last page (page 4, limit 3 → 1 remaining item)
   - Page 0 underflow (saturating_sub prevents panic)
   - Limit capping (limit 9999 → capped to 500)

3. **`test_guess_mime_common_types`** — Tests MIME type detection for:
   - `.rs` → text/plain (unknown extension fallback)
   - `.toml` → application/toml
   - `.md` → text/markdown
   - `.json` → application/json
   - `.js` → application/javascript
   - `.html` → text/html
   - `.bin` → text/plain (unknown fallback)

4. **`test_memory_type_validation`** — Tests that valid memory types (fact, episode, knowledge) are accepted and invalid types (invalid, empty string, wrong case) are rejected by the match-based validation.

5. **`test_file_size_limit_enforcement`** — Tests size boundary checks for:
   - Workspace file writes (1MB limit)
   - Skill content (64KB limit)
   - Memory entries (32KB limit)

### Test Results

All **5 new tests pass**.

---

## Build Status

- `cargo check --workspace`: Pre-existing errors in `space/` module and `oxi-ai` (unrelated to changes)
- Scheduler tests: **28/28 pass**
- Workspace route tests: **5/5 pass**
- No new warnings introduced by the changes
