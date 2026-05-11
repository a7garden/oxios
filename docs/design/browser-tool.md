# Browser Tool Design

> Headless browser integration via Lightpanda as a host dependency.

## Overview

Oxios agents need web browsing capability — navigate, click, type, extract content, take screenshots.
Instead of embedding a browser engine (which breaks the single-binary purity), we treat the browser
as a **host dependency**: Lightpanda must be installed on the host system, and Oxios connects to it
via CDP (Chrome DevTools Protocol).

```
Oxios (single binary, pure Rust)
  │
  ├── BrowserTool (agent-facing tool)
  │     │
  │     └── BrowserBackend trait
  │           │
  │           └── CdpBackend (chromiumoxide crate)
  │                 │
  │                 └── WebSocket → lightpanda serve (host process)
  │
  └── Program: lightpanda
        ├── program.toml  (host_requirements: lightpanda)
        ├── SKILL.md      (agent instructions)
        └── bin/           (lightpanda binary, host-installed)
```

## Why This Design

| Concern | Decision | Reason |
|---------|----------|--------|
| Single binary purity | ✅ Preserved | Lightpanda is a host tool, not embedded |
| CDP protocol | `chromiumoxide` crate | Mature, async-first, tokio-native, type-safe CDP |
| Dependency scope | Feature gate | `browser = ["chromiumoxide"]` — no cost when unused |
| Agent abstraction | `BrowserTool` trait | Agents don't know about CDP; just `navigate`, `click`, etc. |
| Lifecycle | Lazy start | Lightpanda process spawned on first use, cached |

## Architecture

### 1. BrowserBackend Trait

```rust
// tools/browser/mod.rs

/// Backend-agnostic browser operations that agents can perform.
#[async_trait]
pub trait BrowserBackend: Send + Sync {
    /// Navigate to a URL and wait for page load.
    async fn navigate(&self, url: &str) -> Result<PageInfo>;

    /// Click an element matching the CSS selector.
    async fn click(&self, selector: &str) -> Result<()>;

    /// Type text into an element matching the CSS selector.
    async fn r#type(&self, selector: &str, text: &str) -> Result<()>;

    /// Execute JavaScript and return the result as JSON.
    async fn evaluate(&self, js: &str) -> Result<serde_json::Value>;

    /// Get the page's HTML content.
    async fn html(&self) -> Result<String>;

    /// Get the page's text content (extracted from DOM).
    async fn text(&self) -> Result<String>;

    /// Take a screenshot and return PNG bytes.
    async fn screenshot(&self) -> Result<Vec<u8>>;

    /// Get the current page title.
    async fn title(&self) -> Result<String>;

    /// Get all elements matching a selector, returning their text content.
    async fn query_all(&self, selector: &str) -> Result<Vec<String>>;

    /// Close the current page.
    async fn close(&self) -> Result<()>;
}
```

### 2. CdpBackend (chromiumoxide)

```rust
// tools/browser/cdp_backend.rs

use chromiumoxide::{Browser, BrowserConfig, Page};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;

pub struct CdpBackend {
    browser: Browser,
    page: Option<Page>,
}
```

### 3. BrowserTool (AgentTool)

Wraps `BrowserBackend` behind the `AgentTool` interface so agents can call it
in their tool-calling loop.

```rust
// tools/browser/browser_tool.rs

pub struct BrowserTool {
    backend: Arc<dyn BrowserBackend>,
}

// AgentTool implementation:
// - name: "browser"
// - operations: navigate, click, type, evaluate, html, text, screenshot, query_all
```

### 4. Lightpanda Process Manager

Spawns and manages the Lightpanda CDP server as a child process:

```rust
// tools/browser/process.rs

pub struct LightpandaProcess {
    child: Option<Child>,
    ws_endpoint: String,  // e.g. "ws://127.0.0.1:9222"
}
```

## File Structure

```
crates/oxios-kernel/src/tools/
├── mod.rs                    # add: pub mod browser;
├── browser/
│   ├── mod.rs                # BrowserBackend trait, BrowserTool, re-exports
│   ├── browser_tool.rs       # AgentTool impl (agent-facing)
│   ├── cdp_backend.rs        # CdpBackend using chromiumoxide
│   └── process.rs            # LightpandaProcess lifecycle management
```

## Program Definition

```
.programs/lightpanda/
├── program.toml
└── SKILL.md
```

## Configuration

```toml
# ~/.oxios/config.toml

[browser]
enabled = true
# Path to lightpanda binary (default: "lightpanda" from PATH)
binary_path = "lightpanda"
# Host for the CDP server
host = "127.0.0.1"
# Port for the CDP server
port = 9222
# Default page load timeout in seconds
timeout_secs = 30
# Maximum number of browser sessions per agent
max_sessions = 3
```

## Dependency Changes

```toml
# crates/oxios-kernel/Cargo.toml

[features]
default = []
browser = ["chromiumoxide", "futures"]

[dependencies]
chromiumoxide = { version = "0.7", features = ["tokio-runtime"], optional = true }
```

## Integration Points

### agent_runtime.rs

```rust
// In run_agent_loop(), after Tier 3 program tools:
// Tier 4: Browser tool (if feature enabled and lightpanda available)
#[cfg(feature = "browser")]
{
    if let Some(ref config) = oxios_config {
        if config.browser.enabled {
            if let Some(ref exec) = exec_tool {
                let browser_tool = BrowserTool::new(/* ... */);
                registry.register(browser_tool);
            }
        }
    }
}
```

### lib.rs

```rust
#[cfg(feature = "browser")]
pub mod browser;
```

## Lifecycle

```
1. Agent calls browser tool
2. BrowserTool checks if LightpandaProcess is running
3. If not: spawn `lightpanda serve --host 127.0.0.1 --port 9222`
4. Connect via chromiumoxide to ws://127.0.0.1:9222
5. Execute requested operation
6. Keep connection alive for subsequent calls
7. On agent shutdown: kill LightpandaProcess
```

## Security

- URL validation via AccessManager (same pattern as ExecTool)
- Lightpanda runs with minimal environment (same env clearing as ExecTool)
- CDP port bound to 127.0.0.1 only (no external access)
- Screenshots stored in agent workspace, not arbitrary paths
- JavaScript execution is full-access within the browser sandbox

## Alternatives Considered

| Alternative | Rejected Because |
|------------|-----------------|
| Embed Servo | Massive dependency, slow compile, not a full browser |
| servo-fetch | No DOM manipulation (scraping only) |
| Bundled Chromium | Breaks single-binary purity, 200MB+ |
| Headless Chrome | Same as above, plus Google dependency |
| kalamari | Security-testing focused, not general-purpose, proprietary license |
