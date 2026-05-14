# Fix Remaining Batch 1 — Results

## Issue #5: Atomic program upgrade

**File:** `crates/oxios-kernel/src/program/mod.rs`
**Status:** ✅ Fixed

### Problem
The `upgrade()` method did `uninstall → install`, which loses the program if install fails after uninstall succeeds.

### Fix
Replaced the non-atomic `uninstall → install` sequence with a temp-directory swap:

1. Parse source metadata (unchanged)
2. Create a temp directory adjacent to `programs_dir` (`.tmp-upgrade-<name>`)
3. Copy files to temp directory via `copy_dir_all`
4. Parse and validate the new program from the temp directory
5. If validation fails, clean up temp and return error — **old program remains intact**
6. If valid, create state file in temp (preserving enabled state), remove old directory, rename temp to target
7. Update in-memory cache

The `install()` and `uninstall()` methods are unchanged.

### Tests
All 35 program tests pass, including all 4 upgrade tests:
- `test_upgrade_same_version_is_noop`
- `test_upgrade_newer_version`
- `test_upgrade_preserves_enabled_state`
- `test_upgrade_installs_if_not_present`

---

## Issue #18: Audit log uses std::thread::spawn

**File:** `crates/oxios-kernel/src/access_manager/mod.rs`
**Status:** ✅ Fixed

### Problem
`persist_audit_entry` spawned a new OS thread for every audit log write — unbounded thread creation.

### Fix
Replaced with a bounded `tokio::sync::mpsc` channel and single background tokio task:

1. Added `audit_sender: Option<tokio::sync::mpsc::Sender<String>>` to `AccessManager`
2. Added `audit_writer_handle: Option<Arc<tokio::task::JoinHandle<()>>>` (Arc for Clone compatibility)
3. In `with_audit_log_path()`: creates a bounded channel (capacity 1000) and spawns a single background task that reads from the channel and writes to the file
4. `persist_audit_entry` now uses `try_send()` (synchronous) — no thread spawning
5. If channel is full, logs a warning and drops the entry (backpressure)
6. `AccessManager` remains `Clone`-able (both `Sender` and `Arc<JoinHandle>` implement Clone)

### Tests
All 43 access_manager tests pass.

---

## Issue #19: Implement ConfigAction::Set

**File:** `src/main.rs`
**Status:** ✅ Fixed

### Problem
`ConfigAction::Set` was a stub that just bailed with "not yet implemented".

### Fix
Replaced with full implementation:
- Loads config from file (or uses default if file doesn't exist)
- Calls `set_config_value()` to mutate the config in-place
- Serializes back to TOML and writes to disk

Added `set_config_value()` function that mirrors the existing `get_config_value()` structure, supporting all known dotted key paths:
- `kernel.workspace`, `kernel.event_bus_capacity`, `kernel.max_agents`
- `gateway.host`, `gateway.port`
- `exec.default_timeout_secs`, `exec.max_timeout_secs`
- `exec.required_host_tools`, `exec.optional_host_tools` (comma-separated list)

Returns `Option<()>` — `None` for unknown keys (error surfaced via `ok_or_else`).

---

## Issue #20: Implement DaemonAction::Restart

**File:** `src/main.rs`
**Status:** ✅ Fixed

### Problem
`DaemonAction::Restart` was a stub that just printed a manual restart command.

### Fix
Replaced with proper process restart via `exec()`:
1. Resolves current executable path via `std::env::current_exe()`
2. Collects current CLI args (skipping arg0)
3. On Unix: uses `std::os::unix::process::CommandExt::exec()` to replace the current process
4. On non-Unix: spawns a child process and exits the current one

---

## Build Verification

```
cargo check --workspace — ✅ Clean (no new warnings)
cargo test -p oxios-kernel program::tests — ✅ 35/35 passed
cargo test -p oxios-kernel access_manager::tests — ✅ 43/43 passed
```
