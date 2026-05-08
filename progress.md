# Progress

## Status
In Progress

## Loop 11 Post-Implementation Review (2026-05-08)

### Summary

| Area | Status | Notes |
|------|--------|-------|
| Config validation | **FAIL** | No `OxiosConfig::validate()`, `load_config()` doesn't call it |
| Atomic JSON saves | **FAIL** | Direct `fs::write()`, no temp+rename |
| Agent timeout | **FAIL** | No explicit timeout in `spawn_and_run()` |
| README Quick Start | **PASS** | Section exists, well-structured |
| Cron normalize_expr | **PASS** | Correctly prepends "0 " for 5-field |
| Cron running_jobs guard | **PASS** | `HashSet<Uuid>` with full lifecycle |
| Build (cargo check) | **PASS** | No errors, 36 warnings (missing docs) |
| Tests (cargo test) | **PASS** | 316 passed, 0 failed, 1 ignored |

## Loop 11b — Documentation + API Key Environment Variable (2026-05-08)

### Summary

| Area | Status | Notes |
|------|--------|-------|
| `OXIOS_API_KEY` env var | **PASS** | Primary source, falls back to config |
| `security.default_api_key` | **PASS** | New config field for simple deployments |
| `OxiosConfig::api_key()` | **PASS** | Method checks env first, then config |
| Auth middleware | **PASS** | Accepts key from 3 sources |
| README Quick Start | **PASS** | Comprehensive section added |
| Build (cargo check) | **PASS** | No errors |
| Tests (cargo test) | **PASS** | 316 passed, 0 failed |

### Configuration Priority

1. `OXIOS_API_KEY` environment variable (highest)
2. `security.default_api_key` in config.toml
3. Hashed keys in `security.api_keys_path` JSON file

### Files Changed

- `crates/oxios-kernel/src/config.rs` — `api_key()` method, `default_api_key` field, validation warning
- `channels/oxios-web/src/middleware.rs` — Updated auth to check env var and config key
- `README.md` — Added Quick Start section, updated env vars table
- `crates/oxios-kernel/tests/integration_tests.rs` — Fixed missing `max_execution_time_secs` param

### Build & Test Results
- `cargo check --workspace`: **PASS** — No errors
- `cargo test --workspace`: **PASS** — 316 passed, 0 failed, 1 ignored

### Critical Fixes Needed (3)

1. **`crates/oxios-kernel/src/config.rs`** — Add `OxiosConfig::validate()`:
   - Implement structural validation (range checks, required fields, mutual exclusions)
   - Call from `load_config()` before returning

2. **`crates/oxios-kernel/src/state_store.rs`** — Make `save_json()` atomic:
   - Write to temp file `{name}.json.tmp`
   - Atomic rename to `{name}.json` (POSIX rename is atomic)

3. **`crates/oxios-kernel/src/agent_lifecycle.rs`** — Add explicit timeout:
   - Wrap `supervisor.run_with_seed()` in `tokio::time::timeout()`
   - Use `SecurityConfig::max_execution_time_secs` as duration

## Notes
- Review report: `/tmp/oxios-l11-review.md`
- Documentation: `/tmp/oxios-l11-critical2.md`
- 36 warnings in oxios-kernel are all `missing_docs` — can be suppressed or addressed
- oxi external dependencies have 3 warnings (unused import, mutable variables)