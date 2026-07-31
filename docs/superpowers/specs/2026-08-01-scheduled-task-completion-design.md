# Scheduled Task (예약 작업) Completion — Design

> Status: approved-for-implementation (autonomous auto-task; no interactive gate).
> Reference: LobeHub Agent Tasks (UI patterns only); oxios RFC-043 (Task module).

## Problem

Oxios has two half-wired scheduling subsystems:

1. **CronScheduler** (`cron.rs`) — fully implemented (`start`, `restore_jobs`,
   `load_from_config`, `add_job`, `trigger_job`, `mark_job_completed`) but the
   tick loop is **never started at boot**. Persisted + config-defined jobs never
   fire. The web manual-trigger path (`cron_jobs.rs`) works only because it
   routes through the gateway bridge on demand.

2. **Task module** (`task/`, RFC-043) — SQLite CRUD + scheduling fields exist
   (`automation_mode`, `schedule_pattern`, `heartbeat_interval_secs`,
   `next_run_at`, `list_due_tasks()`), but:
   - `POST /api/tasks/:id/run` returns a **503 stub**.
   - `handle_task_set_schedule` is **buggy** — sets `next_run_at` but never
     persists the automation fields (acknowledged TODO in the handler).
   - **No execution loop** drives due tasks.

## Design Principles

- **One shared execution primitive.** Both the cron auto-start executor and the
  task run/schedule loops call the same `run_goal(goal) → OrchestrationResult`.
  No duplicated execution paths (per architectural review).
- **Direct orchestrator path, not gateway correlation.** The cron auto-start
  loop is a local background task with no waiting HTTP client. It captures
  `Arc<Orchestrator>` and `await`s `handle_unified` directly — exactly what the
  CLI does in `execute_prompt_with_session` (kernel.rs:544). The gateway's
  response-correlation machinery exists for live HTTP clients, not background
  tasks.
- **Decouple CronJobs from Tasks.** A CronJob is fire-and-forget; a Task is a
  stateful lifecycle (verify, execution count, dependencies). Coupling them via
  a `task_id` on CronJob would leak abstractions. Instead each gets its own
  lightweight tick loop, both backed by `run_goal`.
- **Reuse existing UI.** The cron-jobs page is already well-developed
  (timeline, `CronScheduleEditor`, edit dialog, templates). The tasks page is
  the one needing work; it reuses `CronScheduleEditor` for schedule config.

## Architecture

```
                         ┌─────────────────────────┐
   cmd_serve boot ──────▶│ KernelHandle.run_goal() │  ← shared primitive
                         │  orchestrator.handle_   │
                         │  unified("system",goal) │
                         └───────────┬─────────────┘
                                     │
           ┌─────────────────────────┼─────────────────────────┐
           ▼                         ▼                         ▼
  ┌─────────────────┐    ┌────────────────────┐    ┌────────────────────┐
  │ CronScheduler   │    │ Task auto-run loop │    │ POST /api/tasks/   │
  │ start() loop    │    │ (poll list_due)    │    │   :id/run          │
  │ (restore + cfg) │    │ 60s tick           │    │ (synchronous)      │
  └─────────────────┘    └────────────────────┘    └────────────────────┘
```

## Backend Changes

### B1. `KernelHandle::run_goal` — shared execution primitive

Add an optional `orchestrator: Option<Arc<Orchestrator>>` field to `KernelHandle`
+ builder `.with_orchestrator(Arc<Orchestrator>)`. Wire in `kernel.rs handle()`
via `.with_orchestrator(self.orchestrator.clone())`.

```rust
impl KernelHandle {
    /// Execute a goal through the Ouroboros pipeline (shared by cron auto-start,
    /// task auto-run loop, and POST /api/tasks/:id/run). Returns the full
    /// orchestration result so callers can map success/summary as needed.
    pub async fn run_goal(&self, goal: &str, session_id: Option<&str>)
        -> anyhow::Result<OrchestrationResult>
    {
        let orch = self.orchestrator.as_ref()
            .ok_or_else(|| anyhow::anyhow!("orchestrator not wired"))?;
        let req_id = format!("run-goal-{}", uuid::Uuid::new_v4());
        orch.handle_unified("system", goal, session_id, None, None, None,
                            None, None, &req_id).await
    }
}
```

**Why on KernelHandle, not `Kernel`:** the web task-run handler only has
`Arc<KernelHandle>`; placing `run_goal` here gives both the background loops
and the HTTP handler one call site.

### B2. CronScheduler auto-start wiring (cmd_serve)

In `cmd_serve`, after the gateway is spawned and channels registered, spawn the
cron loop:

```rust
let cron_handle = spawn_cron_loop(kernel);   // returns JoinHandle
supervisor.track_critical("cron", cron_handle);
```

`spawn_cron_loop`:
1. `cron_scheduler.restore_jobs().await` — reload persisted jobs.
2. `cron_scheduler.load_from_config(&config.cron).await` — config-defined jobs.
3. `cron_scheduler.clone().start(executor)` where executor captures the
   KernelHandle and maps `OrchestrationResult → (bool, String)`:
   `success = evaluation_passed.unwrap_or(false)`,
   `summary = output.unwrap_or(response)`.

Spawned **after** the intent engine is wired (build() completes → handle()
cached → cmd_serve runs), so `handle_unified`'s `expect("IntentEngine not
wired")` is safe. The 60s default tick also lands well post-boot.

### B3. `POST /api/tasks/:id/run` — real implementation

Replace the 503 stub:
1. Load task; 404 if missing.
2. `task_store.mark_running(&id)` — status `Running`, `started_at`/`last_run_at`.
3. `result = state.kernel.run_goal(&task.instruction, None)`.
4. `task_store.mark_finished(&id, &result)` — on success: `Completed` +
   `execution_count += 1` + `completed_at` + `last_result`; on err: `Failed` +
   `last_error` + `consecutive_failures += 1`.
5. Recompute `next_run_at` if automation is set.
6. Return `{ success, summary, session_id }`.

### B4. Fix `set_schedule` + add TaskStore lifecycle methods

`TaskStore` gains:
- `set_automation(id, SetScheduleParams)` — persists `automation_mode`,
  `schedule_pattern`, `schedule_timezone`, `heartbeat_interval_secs`,
  `max_executions`, and computes + sets `next_run_at`, sets status `Scheduled`.
- `mark_running(id)` — status `Running`, `started_at`, `last_run_at`.
- `mark_finished(id, success, summary, error)` — terminal status, counts,
  timestamps, `last_result`.
- Add `last_result TEXT` column (schema migration in `init_schema`).

`handle_task_set_schedule` calls `set_automation` (replacing the buggy no-op).

### B5. Task auto-run tick loop

Spawned in the web plugin (where `task_store` is constructed), a 60s loop:
```
loop { sleep(60s); for task in list_due_tasks(): run + mark_finished + recompute next_run }
```
- `next_run_at` recompute: schedule → next cron fire; heartbeat → now + interval.
- Respects `max_executions` (`is_exhausted()` → skip + mark Completed).
- Spawns each run on its own task (don't block the tick on a long LLM call).

## Frontend Changes

### F1. `CreateTaskDialog` — schedule config

Add an optional **Automation** section (collapsed by default):
- Mode toggle: None / Schedule (cron) / Heartbeat (interval).
- Schedule mode → reuse `CronScheduleEditor` (already built for cron-jobs).
- Heartbeat mode → interval (minutes) input.
- On submit: `createTask` then `setTaskSchedule` if a mode was chosen.

### F2. `TaskCard` — schedule + result display

- Show `next_run_at` (relative time) when scheduled.
- Show `last_result` snippet (line-clamp) after a run.
- Add a **Schedule** edit action (opens a schedule dialog reusing the editor).
- "Run" already exists; wire it to the now-working endpoint + show toast.

### F3. Task detail drawer

A `Dialog`/`Sheet` opened from the card showing: full instruction, schedule
config, execution history (`execution_count`, `last_run_at`, `last_result`,
`last_error`, `consecutive_failures`), and a Run button.

## Verification

- `cargo check` + `cargo clippy -D warnings` + `cargo test --workspace`.
- `cd web && bun run build`.
- Smoke: start daemon, create a cron job (manual trigger fires), create a task
  + run it (status transitions), set a schedule (next_run computed).

## Out of Scope

- Refactoring TaskStore out of the web plugin into the kernel (kept web-local).
- CronJob↔Task cross-registration (decoupled by design).
- TaskRun history table (single `last_result` column suffices for v1).
- LobeHub cloud deps (QStash) — N/A, oxios runs on-host.
