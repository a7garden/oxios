# Clippy Fix Results: oxios-web + oxios binary

## Summary

All MutexGuard-across-await warnings fixed. **0 warnings** remain in `oxios-web` and `oxios` (binary).

### Verification

```
cargo clippy -p oxios-web  → 0 warnings (only oxi-ai dep has 1 unrelated warning)
cargo clippy -p oxios       → 0 warnings
cargo test --workspace      → 245 passed, 0 failed, 7 ignored
```

## Files Changed

### 1. `channels/oxios-web/src/middleware.rs` (lines 56, 62)

**Problem:** `state.auth_manager.lock()` (parking_lot::MutexGuard) held across `next.run(request).await`.

**Fix:** Scoped the guard — validate token in a block, drop guard before the await.

```rust
// BEFORE:
let mut auth = state.auth_manager.lock();
if !auth.validate(token) { ... }
Ok(next.run(request).await)  // guard still held!

// AFTER:
let is_valid = {
    let mut auth = state.auth_manager.lock();
    auth.validate(token)
}; // guard dropped
if !is_valid { ... }
Ok(next.run(request).await)
```

### 2. `channels/oxios-web/src/server.rs`

**Problem:** `mcp_bridge: Arc<parking_lot::Mutex<McpBridge>>` — parking_lot guards are `!Send`, cannot be held across `.await` in Send futures (required by axum).

**Fix:** Changed `parking_lot::Mutex` → `tokio::sync::Mutex` for `mcp_bridge`. The tokio MutexGuard is `Send` and safe to hold across await points.

### 3. `channels/oxios-web/src/routes.rs` (lines 1444, 1452, 1493, 1520, 1521, 1573, 1575)

**Problem:** Five instances of `state.mcp_bridge.lock()` held across `.await` in MCP route handlers.

**Fix:** With `tokio::sync::Mutex`, all `.lock()` calls became `.lock().await`. The guard is now `Send` and safe across await points. No restructuring needed — the lock-across-await is now legal.

Affected handlers:
- `handle_mcp_servers_list` — `bridge` held across `client().await` and `is_initialized().await`
- `handle_mcp_server_register` — guard held across `initialize_server().await`
- `handle_mcp_tools_list` — `bridge` held across `list_tools().await` and `cached_tools().await`
- `handle_mcp_tool_call` — guard held across `call_tool().await`

### 4. `src/main.rs` (lines 836, 943)

**Problem:** `mcp_bridge.lock()` (parking_lot) held across `.await` for `initialize_all()` and `shutdown_all()`.

**Fix:** Changed `mcp_bridge` type to `Arc<tokio::sync::Mutex<McpBridge>>`. Updated all call sites to `.lock().await`. Renamed `parking_lot::Mutex` import to `PLMutex` to avoid collision (still used for `access_manager` and `auth_manager`).

### 5. `oxi-ai/src/providers/openai.rs` (bonus fix)

**Problem:** Pre-existing syntax error: `format("...")` instead of `format!("...")`.

**Fix:** Added missing `!` macro invocation. This was blocking all compilation.

## Why tokio::sync::Mutex for mcp_bridge?

The `McpBridge` has async methods (`initialize_server`, `list_tools`, `cached_tools`, `call_tool`, `shutdown_all`) that take `&self` and internally use `tokio::sync::RwLock`. These methods **must** be called through the MutexGuard, which means the guard is inherently held across `.await` points.

Options considered:
1. **Scope the guard** — impossible for methods that need `&self` from the guard AND do async work internally
2. **Change to `tokio::sync::Mutex`** ✅ — guard is `Send`, legal to hold across `.await`
3. **Suppress with `#[allow]`** — hides a real potential deadlock risk

The `tokio::sync::Mutex` is the correct choice because:
- `McpBridge` async methods hold the guard for the duration of their async operations
- These operations use internal `RwLock`s that are already async-safe
- The mutex protects only the `servers: Vec<McpServer>` field (sync mutations like `register_server`)
- Contention is low (MCP operations are infrequent)
