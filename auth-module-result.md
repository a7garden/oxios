# Oxios Auth Module — Implementation Result

## Summary

Successfully created the Oxios Auth module (`auth.rs`) with SHA-256 hashed API key management.

## Files Changed

### New File
- **`crates/oxios-kernel/src/auth.rs`** — API key authentication manager with:
  - SHA-256 hashed key storage (keys never stored in plaintext)
  - `AuthManager` with in-memory or file-persisted modes
  - `generate_key()` — creates `oxios_`-prefixed keys with random 32-byte payloads
  - `validate()` — hashes incoming bearer token and checks against stored hashes
  - `revoke_key()` — removes key by name
  - `list_keys()` — returns metadata only (never exposes keys)
  - Atomic file writes via temp+rename pattern
  - 8 unit tests (all passing)

### Modified Files
1. **`Cargo.toml` (workspace root)** — Added workspace deps: `sha2 = "0.10"`, `hex = "0.4"`, `getrandom = "0.2"`
2. **`crates/oxios-kernel/Cargo.toml`** — Added `sha2`, `hex`, `getrandom` as workspace deps
3. **`crates/oxios-kernel/src/config.rs`** — Added to `SecurityConfig`:
   - `auth_enabled: bool` (default: `false`)
   - `api_keys_path: String` (default: `~/.oxios/api-keys.json`)
   - `cors_origins: Vec<String>` (default: `["http://localhost:4200"]`)
4. **`crates/oxios-kernel/src/lib.rs`** — Added `pub mod auth;` and `pub use auth::{AuthManager, KeyMeta};`

## Build & Test

```
$ cargo build -p oxios-kernel    ✅ compiles (0 new warnings)
$ cargo test -p oxios-kernel -- auth  ✅ 8/8 tests pass
```

### Test Results
| Test | Result |
|------|--------|
| `generate_and_validate_key` | ✅ |
| `invalid_key_rejected` | ✅ |
| `revoke_key` | ✅ |
| `revoke_nonexistent_key_fails` | ✅ |
| `has_keys_reflects_state` | ✅ |
| `list_keys_returns_metadata` | ✅ |
| `persistence_roundtrip` | ✅ |
| `hash_is_deterministic` | ✅ |

## Notes
- Used `getrandom::getrandom()` (v0.2 API) instead of `getrandom::fill()` (v0.3+ API)
- Pre-existing warnings in `program_tool.rs`, `container_exec.rs`, `mcp_tool.rs`, and `program.rs` are unrelated to this change
