# Progress

## Status
Complete — RFC-010 Phase 3 Backend API

## Tasks

### RFC-010 Phase 3: Backend API (DONE ✅)
- [x] Create `crates/oxios-kernel/src/kernel_handle/marketplace_api.rs` — `MarketplaceApi` facade
- [x] Add `pub mod marketplace_api` + `pub use MarketplaceApi` to `kernel_handle/mod.rs`
- [x] Add `pub marketplace_api: MarketplaceApi` field to `KernelHandle` struct
- [x] Update `KernelHandle::new()` to accept `marketplace_api` parameter
- [x] Update `KernelHandle::from_subsystems()` to construct `MarketplaceApi` using `config.marketplace.base_url`
- [x] Add `serde::Serialize` to ClawHub API types (`types.rs`) — required for axum `Json<T>`
- [x] Add `ClawHubClient::base_url()` accessor to `client.rs`
- [x] Add `ClawHubInstaller::client()` accessor to `installer.rs`
- [x] Update all `KernelHandle::new()` call sites (kernel_bridge test, supervisor test)
- [x] Create `channels/oxios-web/src/routes/marketplace.rs` with 4 handlers
- [x] Update `channels/oxios-web/src/routes/mod.rs` — add module, re-exports, route registrations
- [x] Add `MarketplaceApi` re-export to `crates/oxios-kernel/src/lib.rs`

## Files Changed

### New files
- `crates/oxios-kernel/src/kernel_handle/marketplace_api.rs`
- `channels/oxios-web/src/routes/marketplace.rs`

### Modified files
- `crates/oxios-kernel/src/kernel_handle/mod.rs`
- `crates/oxios-kernel/src/clawhub/types.rs`
- `crates/oxios-kernel/src/clawhub/client.rs`
- `crates/oxios-kernel/src/clawhub/installer.rs`
- `crates/oxios-kernel/src/lib.rs`
- `crates/oxios-kernel/src/tools/kernel_bridge.rs`
- `crates/oxios-kernel/src/supervisor.rs`
- `channels/oxios-web/src/routes/mod.rs`

## Routes Added

| Method | Path | Handler |
|--------|------|---------|
| GET | `/api/marketplace/search?q=...&limit=N` | `handle_marketplace_search` |
| GET | `/api/marketplace/skills/{slug}` | `handle_marketplace_skill_detail` |
| POST | `/api/marketplace/skills/{slug}/install` | `handle_marketplace_install` |
| GET | `/api/marketplace/updates` | `handle_marketplace_updates` |

## Notes

- `cargo check -p oxios-kernel` ✅ (pre-existing warnings only)
- `cargo check -p oxios-web` ✅ (1 pre-existing unused variable warning)
- `cargo build -p oxios-kernel -p oxios-web` ✅
- Key fix: Added `serde::Serialize` to ClawHub API types (was `Deserialize` only) — this was the root cause of the "Handler trait not satisfied" error
- Implementation notes written to: `/tmp/clawhub-api-impl.md`