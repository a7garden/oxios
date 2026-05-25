# Progress

## Status
In Progress — RFC-010 Phase 1 complete

## Tasks

### RFC-010: ClawHub Marketplace Integration

- [x] Phase 1: ClawHub client, types, and installer
  - [x] `crates/oxios-kernel/src/clawhub/types.rs` — API types + origin/lockfile types
  - [x] `crates/oxios-kernel/src/clawhub/client.rs` — HTTP client (search, get_skill, download)
  - [x] `crates/oxios-kernel/src/clawhub/installer.rs` — install, update, update_all, check_updates
  - [x] `crates/oxios-kernel/src/clawhub/mod.rs` — module declaration + re-exports
  - [x] `lib.rs` — added clawhub module + re-exports
  - [x] `Cargo.toml` — added reqwest, zip, url deps
  - [x] `kernel_handle/mod.rs` — wired MarketplaceApi with ClawHubInstaller/ClawHubClient
  - [x] `kernel_bridge.rs` — fixed duplicate MarketplaceApi call
  - [x] Unit tests (7 passing)

- [ ] Phase 2: Kernel integration (MarketplaceApi → KernelHandle)
- [ ] Phase 3: Backend API endpoints (`/api/marketplace/*`)
- [ ] Phase 4: Web UI — marketplace tab
- [ ] Phase 5: CLI commands (`oxios marketplace *`)

## Files Changed

### Created
- `crates/oxios-kernel/src/clawhub/mod.rs`
- `crates/oxios-kernel/src/clawhub/types.rs`
- `crates/oxios-kernel/src/clawhub/client.rs`
- `crates/oxios-kernel/src/clawhub/installer.rs`

### Modified
- `crates/oxios-kernel/src/lib.rs` — added clawhub module + re-exports
- `crates/oxios-kernel/Cargo.toml` — added reqwest, zip, url
- `crates/oxios-kernel/src/kernel_handle/mod.rs` — MarketplaceApi wiring + state_store fix
- `crates/oxios-kernel/src/tools/kernel_bridge.rs` — removed duplicate MarketplaceApi call

## Notes

- Lockfile location: `{workspace_dir}/.clawhub/lock.json`
- Origin file: `{skill_dir}/.clawhub/origin.json`
- `find_skill_root` is generic over `R: Read + Seek` for testability
- Pre-existing `MarketplaceApi` was already in codebase — updated wiring only
- 7 unit tests passing for clawhub module