# Progress

## Status
Completed

## Tasks
- [x] Add Chat, Tui, Backup, Restore command variants to `Command` enum in main.rs
- [x] Add command handlers for Chat (interactive CLI via CliChannel), Tui (placeholder), Backup, Restore
- [x] Add `oxios-cli` dependency to root Cargo.toml and workspace members
- [x] Add `tracing-appender` to workspace and root dependencies
- [x] Add log rotation with tracing-appender (daily rotation, non-blocking writer)
- [x] Fix pre-existing compilation errors (memory.rs borrow issues, api_docs.rs SecurityScheme import, utoipa-swagger-ui version mismatch with axum 0.8)
- [x] Fix recursive async in backup.rs (Box::pin)
- [x] Verify compilation passes (`cargo check -p oxios` — clean)

## Files Changed
- `Cargo.toml` — Added `oxios-cli` dep, `tracing-appender` workspace + root dep, added `channels/oxios-cli` to workspace members
- `src/main.rs` — Added Chat/Tui/Backup/Restore commands, handlers, tracing-appender log rotation
- `src/kernel.rs` — No changes needed (fields already public)
- `channels/oxios-cli/` — Created new crate with CliChannel, CliChannelHandle, InteractiveLoop (reedline-based)
- `crates/oxios-kernel/src/backup.rs` — Created backup/restore functions with recursive directory copy
- `crates/oxios-kernel/src/lib.rs` — Added `pub mod backup` (already done by other worker)
- `crates/oxios-kernel/src/memory.rs` — Fixed pre-existing borrow-after-move errors
- `channels/oxios-web/src/api_docs.rs` — Fixed SecurityScheme import
- `channels/oxios-web/Cargo.toml` — Upgraded utoipa-swagger-ui 8 → 9 for axum 0.8 compat
- `channels/oxios-web/src/server.rs` — Fixed Router state type mismatch with SwaggerUi
- `channels/oxios-cli/src/interactive.rs` — Fixed reedline API (PromptSegments → DefaultPromptSegment)

## Notes
- The `oxios-cli` crate was created from scratch since it didn't exist. It provides `CliChannel` (implements `Channel` trait), `CliChannelHandle`, and `InteractiveLoop` (reedline-based REPL with meta-commands).
- Log rotation uses `tracing_appender::rolling::daily` with a non-blocking writer. The guard is leaked via `Box::leak` for program lifetime.
- The backup module uses a `Box::pin` recursive async pattern to satisfy Rust's recursion requirements.
- Several pre-existing compilation errors were fixed as blockers: memory.rs borrow issues, utoipa-swagger-ui version incompatibility with axum 0.8, and reedline API mismatches.
