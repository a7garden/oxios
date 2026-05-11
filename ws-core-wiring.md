# WebSocket Upgrade & CDP Core Wiring — Implementation Report

## Summary

Successfully implemented WebSocket upgrade and wired all CDP domain handlers to the real Browser core in the OxiBrowser project.

## Changes Made

### Task 1: Wire CdpServer to Browser

**File: `crates/oxibrowser-cdp/src/server.rs`**
- `CdpServer::new(addr, browser: Arc<Browser>)` now takes a shared Browser reference
- Browser is stored as `browser: Arc<Browser>` field in CdpServer
- Browser reference is cloned and passed to CdpSession on WebSocket upgrade
- `handle_http_request` changed from `&self` method to a static async function that receives `browser: Arc<Browser>` as a parameter (required for the `service_fn` + spawn pattern)

### Task 2: Implement WebSocket Upgrade

**File: `crates/oxibrowser-cdp/src/server.rs`**
- The `/ws` handler now performs a real WebSocket upgrade using hyper's built-in upgrade mechanism:
  1. Detects `Upgrade: websocket` header
  2. Extracts `Sec-WebSocket-Key` and derives `Sec-WebSocket-Accept` using SHA-1 + Base64
  3. Spawns an async task that:
     - Calls `hyper::upgrade::on(req)` to get the upgraded IO
     - Wraps it with `TokioIo` (hyper→tokio IO adapter from `hyper_util`)
     - Creates `WebSocketStream::from_raw_socket(io, Role::Server, None)`
     - Creates a `CdpSession` and calls `session.run()`
  4. Returns `101 Switching Protocols` with correct WebSocket headers

**Key insight:** `hyper::upgrade::Upgraded` implements `hyper::rt::Read/Write`, NOT `tokio::io::AsyncRead/AsyncWrite`. Must wrap with `hyper_util::rt::TokioIo<Upgraded>` to satisfy `tokio-tungstenite`'s trait bounds. `TokioIo` is bidirectional — it converts both tokio↔hyper IO traits.

**New dependencies:** `sha1`, `base64` added for WebSocket accept-key computation.

### Task 3: Wire CdpSession to Browser

**File: `crates/oxibrowser-cdp/src/session.rs`**
- `CdpSession` now holds:
  - `browser: Arc<Browser>` — for creating sessions and browser-level operations
  - `session: Arc<RwLock<Session>>` — the browsing context (navigation, DOM, JS)
- `CdpSession::new()` is now `async` — creates a Browser Session on construction
- WebSocket stream type changed from `WebSocketStream<MaybeTlsStream<TcpStream>>` to `WebSocketStream<TokioIo<Upgraded>>`
- `handle_text_message()` passes `&self.session` to the async dispatch

### Task 4: Update Domain Handlers to Use Real Data

**File: `crates/oxibrowser-cdp/src/domains/mod.rs`**
- `dispatch()` is now `async` and takes `session: &Arc<RwLock<Session>>`
- Routes session to DOM, Page, and Runtime domains (Browser, Network, Target remain sync)

**File: `crates/oxibrowser-cdp/src/domains/page.rs`**
- `navigate()` → calls `session.write().await.navigate(url)` with real HTTP fetch
- `reload()` → calls `session.write().await.reload()`
- `getFrameTree()` → returns actual frame ID and URL from the parsed page
- All handlers that access session are now `async`

**File: `crates/oxibrowser-cdp/src/domains/dom.rs`**
- `getDocument()` → traverses the real html5ever-parsed DOM tree and builds CDP node JSON
- `getOuterHTML()` → returns actual page HTML via `page.content()`
- `querySelector()` → uses real `Frame::query_selector()` with html5ever CSS matching
- `querySelectorAll()` → uses real `Document::query_selector_all()`
- Added `build_cdp_node()` helper that converts webapi `Document` tree to CDP format:
  - Maps NodeType → CDP nodeType numbers (Document=9, Element=1, Text=3, Comment=8, Doctype=10)
  - Filters whitespace-only text nodes
  - Depth-limited to 10 levels to avoid huge outputs

**File: `crates/oxibrowser-cdp/src/domains/runtime.rs`**
- `evaluate()` → calls `session.write().await.evaluate_js(expression)` using the real JS runtime
- Falls back to stub evaluator if JS runtime returns an error
- Properly maps `JsEvalResult` (value, exception) to CDP response format

### Task 5: Update main.rs

**File: `crates/oxibrowser/src/main.rs`**
- `run_serve()` creates a `Browser` with `BrowserConfig::headless()`
- Wraps in `Arc` and passes to `CdpServer::new(addr, browser)`
- On Ctrl+C: calls `server.shutdown()` then `browser.close().await`

### Dependency Changes

**Workspace `Cargo.toml`:**
- Added `sha1 = "0.10"` and `base64 = "0.22"` to workspace dependencies

**`crates/oxibrowser-cdp/Cargo.toml`:**
- Added `oxibrowser-webapi = { workspace = true }` (for DOM types in domain handlers)
- Added `sha1 = { workspace = true }` (for WebSocket accept-key)
- Added `base64 = { workspace = true }` (for WebSocket accept-key)

### Bug Fix

- Fixed pre-existing bug: `/json` endpoint's `webSocketDebuggerUrl` was `ws://host:port/ws/ws` (double `/ws`). Now correctly returns `ws://host:port/ws`.

## Verification

- `cargo check` — ✅ Zero errors (warnings only, all pre-existing)
- `cargo build` — ✅ Successful
- `serve` command — ✅ Creates Browser, starts CDP server
- `/json/version` — ✅ Returns correct JSON
- `/json` — ✅ Returns target list with correct WS URL
- WebSocket upgrade — ✅ 101 Switching Protocols, CdpSession created with Browser Session

## Architecture After Changes

```
main.rs
  └── Browser (Arc)
        ├── CdpServer (Arc)
        │     └── TCP Listener
        │           ├── HTTP /json/version → JsonVersion
        │           ├── HTTP /json → JsonTarget list
        │           └── WS /ws → 101 → CdpSession
        │                         ├── browser: Arc<Browser>
        │                         ├── session: Arc<RwLock<Session>>
        │                         └── dispatch(method, params, &session)
        │                               ├── Page.navigate → session.navigate()
        │                               ├── DOM.getDocument → document tree → CDP JSON
        │                               ├── DOM.getOuterHTML → page.content()
        │                               ├── Runtime.evaluate → session.evaluate_js()
        │                               └── ...other domains (stubs)
        └── Sessions[] (per WS connection)
              └── Session
                    ├── Page → Frame → Document (html5ever)
                    ├── JsRuntime (stub evaluator)
                    └── History
```
