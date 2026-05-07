# Progress

## Status
In Progress

## Tasks
- [x] Protocol 8→10: ouroboros crate enhancements
  - [x] Replace `parse_json` with prose-wrapping support
  - [x] Add `llm_json` helper with retry-once logic
  - [x] Create `eval_cache.rs` module
  - [x] Add `MechanicalEvalResult` and `CriterionResult` to `evaluation.rs`
  - [x] Register `eval_cache` module in `lib.rs`
  - [x] Add `eval_cache` field to `OuroborosEngine`
  - [x] Add tests for eval_cache, MechanicalEvalResult, and parse_json
- [x] Created `channels/oxios-cli/` crate
  - [x] `Cargo.toml` with workspace deps
  - [x] `commands.rs` — MetaCommand parsing (.quit, .help, .reset, .model, .persona, .clear)
  - [x] `session.rs` — Session struct with id, label, created_at, last_active, message_count
  - [x] `channel.rs` — CliChannel implementing Channel trait from oxios-gateway
  - [x] `interactive.rs` — InteractiveLoop using reedline 0.38
  - [x] `lib.rs` — Re-exports CliChannel, CliChannelHandle, InteractiveLoop, Session
  - [x] Added `channels/oxios-cli` to workspace members

## Files Changed
- `crates/oxios-ouroboros/src/ouroboros_engine.rs` — replaced `parse_json` with prose-aware version; added `llm_json` retry helper; added `eval_cache` field to struct and `new()`
- `crates/oxios-ouroboros/src/eval_cache.rs` — new file: in-memory EvalCache with FIFO eviction
- `crates/oxios-ouroboros/src/evaluation.rs` — added `MechanicalEvalResult`, `CriterionResult`, and `evaluate()` method
- `crates/oxios-ouroboros/src/lib.rs` — added `pub mod eval_cache`
- `crates/oxios-ouroboros/tests/eval_cache_test.rs` — new test file: 15 tests
- `channels/oxios-cli/Cargo.toml` — new crate manifest
- `channels/oxios-cli/src/lib.rs` — crate root with re-exports
- `channels/oxios-cli/src/commands.rs` — MetaCommand parser with tests
- `channels/oxios-cli/src/session.rs` — Session tracking
- `channels/oxios-cli/src/channel.rs` — CliChannel + CliChannelHandle
- `channels/oxios-cli/src/interactive.rs` — InteractiveLoop with reedline
- `Cargo.toml` — added workspace member

## Notes
- All 37 ouroboros tests pass (15 new + 22 existing), 0 warnings
- No new crate dependencies added to ouroboros
- `llm_json` and `eval_cache` field marked `#[allow(dead_code)]` as they'll be wired in future protocol steps
- reedline 0.38.0 exists on crates.io and resolves fine
- Removed `oxios-kernel` from oxios-cli deps since we don't use it directly (only use oxios-gateway)
- `cargo check -p oxios-cli` fails due to pre-existing errors in `oxios-kernel` (memory.rs, backup.rs, container_manager.rs) — our crate code is valid
- [x] InteractiveLoop.run() is async but uses reedline's blocking read_line — in production, wrap in spawn_blocking

- [x] Kernel: Multi-Agent, Memory, Container, Backup enhancements
  - [x] Add `#[derive(Clone)]` to `AgentLifecycleManager`
  - [x] Replace sequential `delegate_subtasks` with `JoinSet` parallel version
  - [x] Add A2A `deliver_pending_messages` and `send_and_wait` methods
  - [x] Hook A2A delivery into `spawn_and_run` (step 2b)
  - [x] Memory: `content_hash`, `is_duplicate`, `remember_unique`, `VectorIndexSnapshot`, `save/load_index_snapshot`, `MemoryBudget`, `CurationReport`, `effective_importance`, `curate`, `spawn_curation_task`
  - [x] ContainerInfo: `toolchain`, `tools_verified` fields
  - [x] `ToolHealthReport`, `ToolStatus`, `check_tool_health` method
  - [x] New `backup.rs` module with `create_backup` / `restore_backup`
  - [x] Register `backup` module, export all new types in `lib.rs`
  - [x] `cargo check -p oxios-kernel` passes, all 242 tests green
