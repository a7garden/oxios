# OxiBrowser Documentation Generation — Findings

## Files Read

All 20 source files were read in full:

### Workspace
- `Cargo.toml` — Workspace definition with 4 crates, Rust edition 2021

### oxibrowser-core (12 files)
- `lib.rs` — Module re-exports (browser, config, frame, page, session, network, js, error)
- `browser.rs` — `Browser` struct: owns sessions, HTTP client, cookie jar; atomic BrowserId
- `session.rs` — `Session` struct: navigation, history (back/forward/reload), local storage, JS runtime
- `page.rs` — `Page` struct: URL, status, content-type, root Frame, resources, title
- `frame.rs` — `Frame` struct: parsed Document, raw HTML, child frames, DOM version counter
- `config.rs` — `BrowserConfig`: user-agent, timeout, viewport, pool size, TLS, robots.txt
- `error.rs` — `CoreError` enum with 9 variants (thiserror), `Result<T>` alias, `From` impls
- `js/mod.rs` — Module declaration, re-exports `JsRuntime`
- `js/runtime.rs` — `JsRuntime`: stub evaluator (literals, console.log, globals) + servo mode stub
- `network/mod.rs` — Module re-exports (HttpClient, CookieJar)
- `network/client.rs` — `HttpClient`: reqwest wrapper with cookie injection, redirect following
- `network/cookie.rs` — `CookieJar`: domain-scoped HashMap, store/cookies_for_url
- `network/resource.rs` — `Resource` + `ResourceType` (9 variants)

### oxibrowser-cdp (3 files)
- `lib.rs` — Module re-exports (server, session, protocol, domains), re-exports `CdpServer`
- `protocol.rs` — `CdpRequest`, `CdpResponse`, `CdpEvent`, `JsonVersion`, `JsonTarget`, error codes
- `domains/mod.rs` — `dispatch()` router for 6 domains (Browser, DOM, Network, Page, Runtime, Target)

### oxibrowser-webapi (4 files)
- `lib.rs` — Re-exports `Document`
- `dom.rs` — Module re-exports (Document, Node, NodeData, NodeId, NodeType, Tree)
- `dom/document.rs` — `Document`: HTML parsing via html5ever `DomSink` (TreeSink impl), CSS selectors, Markdown conversion
- `dom/node.rs` — `NodeId(usize)`, `NodeType` (Document, Element, Text, Comment, Doctype), `Node` with attribute access
- `dom/tree.rs` — `Tree`: adjacency-list parent/child, DFS/BFS traversal

## Files Created

### 1. `/Volumes/MERCURY/PROJECTS/oxibrowser/AGENTS.md` (16,328 bytes)
Convention guide for AI agents. Includes:
- Project overview (headless browser, Lightpanda-inspired, Rust-native)
- Architecture diagram and Browser→Session→Page→Frame hierarchy
- Directory structure with all source files mapped
- Crate dependency map
- Code conventions (naming, error handling, async patterns, interior mutability, serialization)
- Testing strategy with key test scenarios
- Commit conventions
- Key principles (7 principles)
- Development guide: how to add CDP domains, WebAPI types, DOM operations, network features, config options
- Build & run instructions
- Implementation status matrix

### 2. `/Volumes/MERCURY/PROJECTS/oxibrowser/docs/ARCHITECTURE.md` (21,438 bytes)
Deep architecture document. Includes:
- ASCII system overview diagram
- Component responsibilities (webapi, core, cdp) with tables
- Full data flow for page load (URL → HTTP fetch → HTML parse → DOM build → JS eval → CDP response)
- CDP protocol message lifecycle (WebSocket connection, request/response/event format, HTTP endpoints, error codes)
- JS runtime abstraction (stub vs servo mode)
- Network layer design (HttpClient, CookieJar, Resource tracking)
- Session/Page/Frame lifecycle state machines (ASCII diagrams)
- Error propagation strategy (webapi → core → cdp)
- Thread/async model (tokio runtime, synchronization strategy)

### 3. `/Volumes/MERCURY/PROJECTS/oxibrowser/docs/CDP.md` (17,158 bytes)
CDP implementation guide. Includes:
- All 6 CDP domains with every method, parameters, and return types
- Browser Domain (5 methods)
- DOM Domain (11 methods) with Node wire format and NodeType mapping
- Network Domain (8 methods + 4 events) with Cookie wire format
- Page Domain (10 methods + 5 events) with Frame tree wire format
- Runtime Domain (6 methods) with RemoteObject wire format and type mapping
- Target Domain (7 methods + 5 events) with TargetInfo wire format
- HTTP endpoints (/json/version, /json)
- Full WebSocket exchange example
- Puppeteer compatibility matrix (20+ operations mapped to CDP methods)
- Playwright compatibility matrix
- Current compatibility status table
- Implementation notes (dispatch pattern, event emission, session multiplexing)

### 4. `/Volumes/MERCURY/PROJECTS/oxibrowser/docs/DESIGN.md` (12,390 bytes)
Design rationale document. Includes:
- Why OxiBrowser exists (comparison with Chromium, Firefox, Lightpanda, headless-chrome, fantoccini)
- Why Servo instead of Chromium (5 reasons for, 4 reasons against)
- What we actually use from Servo (6 crates mapped)
- Why html5ever (comparison table with 6 parsers)
- Why tokio-tungstenite (comparison table with 5 libraries)
- Comparison with Lightpanda (what we ported, what we changed, what we kept the same)
- JS runtime abstraction strategy (stub mode capabilities, servo mode integration path)
- Network layer design decisions (reqwest vs hyper, rustls vs native-tls, cookie jar design)
- Roadmap (5 phases with detailed checkboxes)

### 5. `/Volumes/MERCURY/PROJECTS/oxibrowser/CONTRIBUTING.md` (9,554 bytes)
Contribution guide. Includes:
- Development setup (prerequisites, clone/build/verify)
- Project structure overview
- Build instructions (debug, release, binary, feature flags)
- Testing (all tests, per-crate, specific test, output, writing tests, categories, coverage areas)
- Code style (formatting, linting, naming conventions table, documentation, error handling, async code, CDP pattern)
- PR process (checklist, commit format, PR template, review criteria)
- Issue reporting template
- License notice

### 6. `/Volumes/MERCURY/PROJECTS/oxibrowser/.gitignore` (468 bytes)
Standard Rust gitignore: /target/, IDE files, environment files, debug artifacts, profiling data, test artifacts, servo build artifacts.

## Key Observations

1. **Well-structured workspace** with clear crate separation (webapi → core → cdp → binary)
2. **CDP domains are declared but not implemented** — `domains/mod.rs` references 6 domain modules but only `mod.rs` exists in the `domains/` directory (the individual `browser.rs`, `dom.rs`, etc. don't exist yet)
3. **CDP server not implemented** — `server.rs` and `session.rs` are referenced in `lib.rs` but don't exist as files
4. **Binary crate is a placeholder** — `crates/oxibrowser/src/` is empty (no `main.rs`)
5. **html5ever integration is solid** — Full `TreeSink` implementation with proper DOM tree building
6. **JS stub is intentionally minimal** — Handles enough for CDP to not crash; real execution requires servo feature
7. **Thread safety is well-designed** — Appropriate use of `parking_lot::RwLock`, `tokio::sync::RwLock`, and atomics
8. **Error handling is consistent** — `thiserror` enums with `From` conversions and `Result` aliases
