# Progress

## Status
In Progress

## Tasks

## Files Changed

## Notes

## L11: Cron Scheduler Core Implementation (2026-05-08)

### Status: ✅ Complete

### What was done
Implemented the full `CronScheduler` module for `oxios-kernel`:
- Created `crates/oxios-kernel/src/cron.rs` with `CronScheduler`, `CronJob`, `CronJobResult`, `CronJobUpdate`, `JobSource`
- Added `cron = "0.16"` to `Cargo.toml` dependencies
- Added `CronConfig` and `InlineCronJob` to `config.rs` with `use crate::scheduler::Priority`
- Registered `pub mod cron` in `lib.rs` and exported public types
- Removed old `cron_scheduler` module that caused duplicate type errors

Key fixes applied:
- **C1**: `normalize_expr()` auto-prepends `"0 "` for 5-field Linux cron expressions
- **C2**: `running_jobs: Arc<Mutex<HashSet<Uuid>>>` guard prevents duplicate execution
- **L2**: `dirty: Arc<AtomicBool>` flag avoids unnecessary persistence writes
- **Send fix**: Scoped `{ }` block drops `RwLockWriteGuard` before `.await`
- **Arc<Self>**: `start()` takes `Arc<Self>` so spawned tasks get `'static` lifetime

### Verification
- `cargo check -p oxios-kernel` ✅ (pre-existing warnings only)
- `cargo test -p oxios-kernel --lib -- cron` ✅ **16/16 tests pass**

### Files Changed
- `crates/oxios-kernel/Cargo.toml`
- `crates/oxios-kernel/src/lib.rs`
- `crates/oxios-kernel/src/config.rs`
- `crates/oxios-kernel/src/cron.rs` (new)

## L11-CRON-API: Cron Scheduler API Routes (2026-05-08)

### Status: ✅ Complete

### What was done
Created API routes for cron job management and wired them into the web server:

1. **Created `channels/oxios-web/src/routes/cron_jobs.rs`** — 6 API handlers:
   - `GET /api/cron-jobs` — List all cron jobs
   - `POST /api/cron-jobs` — Create a new cron job
   - `GET /api/cron-jobs/{id}` — Get a specific cron job
   - `DELETE /api/cron-jobs/{id}` — Delete a cron job
   - `POST /api/cron-jobs/{id}/edit` — Update a cron job
   - `POST /api/cron-jobs/{id}/trigger` — Manually trigger a cron job

2. **Registered routes in `channels/oxios-web/src/routes/mod.rs`**:
   - Added `mod cron_jobs;` module
   - Re-exported all handlers
   - Added routes to `build_routes`

3. **Added `cron_scheduler` to `AppState` and `Kernel`**:
   - `server.rs`: Added `cron_scheduler: Arc<CronScheduler>` field to `AppState`
   - `server.rs`: Updated `WebServer::new()` to accept `cron_scheduler` parameter
   - `src/kernel.rs`: Added `cron_scheduler: Arc<CronScheduler>` field to `Kernel`
   - `src/main.rs`: Pass `kernel.cron_scheduler.clone()` to `WebServer::new()`

4. **Fixed `cron.rs` Send issues**:
   - `update_job()`: Scoped `RwLockWriteGuard` in block to drop before `.await`
   - `mark_job_completed()`: Scoped `RwLockWriteGuard` in block to drop before `.await`
   - `persist_jobs()`: Clone data out of read lock before `.await`
   - `start()`: Added `'static` bounds to `Fut` parameter
   - `tick_inner()`: Added `'static` bounds to `Fut` parameter

5. **Fixed `agent_runtime.rs` import**: Changed `ToolExecutionMode` import to use `oxi_agent::agent_loop::config::ToolExecutionMode`

6. **Config**: Added `cron: CronConfig` field to `OxiosConfig` in `config.rs`

### Verification
- `cargo check -p oxios-web` ✅
- `cargo check` (full workspace) ✅

### Files Changed
- `channels/oxios-web/src/routes/cron_jobs.rs` (new)
- `channels/oxios-web/src/routes/mod.rs`
- `channels/oxios-web/src/server.rs`
- `src/kernel.rs`
- `src/main.rs`
- `crates/oxios-kernel/src/cron.rs` (Send fixes)
- `crates/oxios-kernel/src/agent_runtime.rs` (import fix)
- `crates/oxios-kernel/src/config.rs` (CronConfig in OxiosConfig)
