# Middleware & Health Endpoint — Implementation Results

## Summary

All changes compile cleanly and all 240 workspace tests pass.

## Files Created

### `/crates/oxios-kernel/src/auth.rs` (new)
- **`AuthManager`** — Bearer token validator backed by a `HashSet<String>`.
- **`KeyMeta`** — Struct for key metadata (label, active flag).
- **`load_from_file()`** — Loads keys from JSON array or line-delimited file.
- **`validate()`** — Checks a token against loaded keys.
- Unit tests for empty validation, key validation.

> Note: The `pub mod auth` and `pub use auth::{AuthManager, KeyMeta}` were already present in `lib.rs`; the module simply didn't exist yet.

### `/channels/oxios-web/src/middleware.rs` (new)
- **`require_auth`** — Axum middleware function (adapted for Axum 0.8 `Next` API).
- Skips auth when `config.security.auth_enabled == false`.
- Allows `/health` without auth.
- Extracts `Bearer <token>` from `Authorization` header.
- Validates via `AuthManager::validate()`.
- Logs failed attempts via `tracing::warn`.

## Files Modified

### `/channels/oxios-web/src/lib.rs`
- Added `pub mod middleware;`.

### `/channels/oxios-web/src/server.rs`
- **AppState**: Added `auth_manager: Arc<parking_lot::Mutex<oxios_kernel::auth::AuthManager>>`.
- **WebServer::new()**: Added `auth_manager` parameter, wired into state.
- **serve()**: Replaced `CorsLayer::permissive()` with restricted CORS (`localhost:4200` only).
- Removed unused `CorsLayer` import.

### `/channels/oxios-web/src/routes.rs`
- Added `GET /health` route at the top of `build_routes()` (before all `/api` routes).
- Added `handle_health()` handler returning `{ status, version, backend: { container } }`.

### `/src/main.rs`
- Added `AuthManager` import.
- **`init_kernel()`**: Returns 16-element tuple (added `Arc<parking_lot::Mutex<AuthManager>>` as 16th element). Creates `AuthManager`, loads API keys from `config.security.api_keys_path`.
- Updated all 5 tuple destructuring sites (`cmd_run`, `cmd_garden`, `cmd_pkg`, `cmd_status`, interactive mode).
- **Interactive mode**: Passes `auth_manager` to `WebServer::new()`.
- **CORS fix**: Replaced `CorsLayer::permissive()` with restricted CORS allowing only `http://localhost:4200`.

## Key Design Decisions

1. **AuthManager in kernel** — Lives in `oxios-kernel` so it can be shared across channels (not just web).
2. **Mutex wrapping** — `parking_lot::Mutex` for interior mutability (`validate` takes `&mut self` for future extensibility).
3. **Health endpoint at `/health`** — Root-level, not under `/api`, matching standard practice.
4. **CORS restricted** — Only `localhost:4200` allowed. Can be replaced with `config.security.cors_origins` when that config is wired into the router.
5. **Axum 0.8 compat** — `Next` no longer takes a generic param; middleware uses `Request<Body>` directly.

## Build & Test

```
cargo build --workspace  ✅  (only pre-existing warnings)
cargo test --workspace   ✅  (240 passed, 1 ignored)
```
