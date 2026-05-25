# Progress

## Status
Completed

## Tasks
- [x] Delete `resources.rs` (all deprecated program/host-tools handlers)
- [x] Remove `mod resources` from `mod.rs`
- [x] Remove `pub(crate) use resources::{...}` re-exports from `mod.rs`
- [x] Remove all `/api/programs/*` route registrations from `mod.rs`
- [x] Remove `/api/host-tools` route registration from `mod.rs`
- [x] Update module doc comment in `mod.rs`
- [x] Verify `workspace.rs` has no program-related references (clean)
- [x] `cargo check -p oxios-web` passes

## Files Changed
- `channels/oxios-web/src/routes/resources.rs` — **deleted entirely**
- `channels/oxios-web/src/routes/mod.rs` — removed `mod resources`, removed `use resources::*` re-exports, removed 8 program/host-tools route registrations, updated doc comment
- `channels/oxios-web/src/routes/workspace.rs` — **no changes needed** (already uses only skill types)

## Notes
- All skill-related routes and handlers remain intact
- All other routes (agents, spaces, scheduler, audit, knowledge, etc.) remain intact
- `resource_routes.rs` (system resource monitoring) is unaffected — separate file
- Build passes cleanly with no new warnings
