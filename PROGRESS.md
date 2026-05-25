# Progress

## Status
In Progress — RFC-010 Phase 2 kernel integration complete, oxios-web handler error

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

- [x] Phase 2: Kernel integration (MarketplaceApi → KernelHandle)
  - [x] `crates/oxios-kernel/src/kernel_handle/marketplace_api.rs` — new MarketplaceApi facade
  - [x] `crates/oxios-kernel/src/kernel_handle/mod.rs` — MarketplaceApi field + accessor
  - [x] `crates/oxios-kernel/src/tools/kernel/marketplace_tool.rs` — AgentTool for agents
  - [x] `crates/oxios-kernel/src/tools/kernel/mod.rs` — registered MarketplaceTool in bridge
  - [x] `crates/oxios-kernel/src/tools/kernel_bridge.rs` — added marketplace to tool_names
  - [x] `crates/oxios-kernel/src/config.rs` — added MarketplaceConfig
  - [x] `crates/oxios-kernel/src/lib.rs` — re-exports for MarketplaceApi + MarketplaceConfig
  - [x] `crates/oxios-kernel/src/clawhub/client.rs` — added `pub fn base_url()` method
  - [x] `src/kernel.rs` — wired MarketplaceApi with workspace_dir + skills_dir
  - [x] `channels/oxios-web/src/routes/marketplace.rs` — HTTP handlers (needs trait fix)
  - ⚠️ oxios-web: Handler trait error — handlers exist but fail axum Handler bound
  - [x] `cargo check -p oxios-kernel` passes
- [ ] Phase 3: Backend API endpoints (`/api/marketplace/*`)
- [ ] Phase 4: Web UI — marketplace tab
- [x] Phase 5: CLI commands (`oxios marketplace *`)
  - ✅ CLI commands fully implemented in `src/main.rs`
  - ✅ MarketplaceApi wired into KernelHandle
  - ✅ All subcommands: search, install, update, updates
  - ✅ Unit tests passing (7/7)

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