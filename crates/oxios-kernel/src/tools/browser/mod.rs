//! Headless browser integration for Oxios agents.
//!
//! Uses the embedded OxiBrowser engine (pure Rust) for web navigation,
//! content extraction, JavaScript evaluation, and DOM queries.
//!
//! ## Architecture
//!
//! ```text
//! Agent → BrowserTool (AgentTool) → BrowserBackend trait → OxibrowserBackend → oxibrowser-core
//! ```
//!
//! No external process is needed — OxiBrowser runs entirely in-process.
//!
//! ## Feature Gate
//!
//! This module is only available with the `browser` feature:
//! ```toml
//! oxios-kernel = { features = ["browser"] }
//! ```

mod browser_tool;
mod oxibrowser_backend;

pub use browser_tool::BrowserTool;
pub use oxibrowser_backend::{OxibrowserBackend, OxibrowserConfig};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Information about a loaded web page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// Page title.
    pub title: String,
    /// Final URL after redirects.
    pub url: String,
}

/// Backend-agnostic browser operations that agents can perform.
///
/// This trait abstracts away the browser implementation details so that
/// agents interact with a clean, high-level API.
#[async_trait]
pub trait BrowserBackend: Send + Sync {
    /// Navigate to a URL and wait for the page to load.
    async fn navigate(&self, url: &str) -> Result<PageInfo>;

    /// Navigate back in history.
    async fn go_back(&self) -> Result<()>;

    /// Navigate forward in history.
    async fn go_forward(&self) -> Result<()>;

    /// Reload the current page.
    async fn reload(&self) -> Result<()>;

    /// Click an element matching the CSS selector.
    async fn click(&self, selector: &str) -> Result<()>;

    /// Type text into an element matching the CSS selector.
    async fn r#type(&self, selector: &str, text: &str) -> Result<()>;

    /// Execute JavaScript and return the result as JSON.
    async fn evaluate(&self, js: &str) -> Result<serde_json::Value>;

    /// Execute JavaScript, optionally awaiting Promise resolution.
    ///
    /// When `await_promise` is true and the result is a Promise,
    /// the runtime drains microtasks and returns the settled value.
    async fn evaluate_with_await(&self, js: &str, await_promise: bool) -> Result<serde_json::Value>;

    /// Get the page's full HTML content.
    async fn html(&self) -> Result<String>;

    /// Get the page's visible text content (extracted from DOM).
    async fn text(&self) -> Result<String>;

    /// Render the page as proper Markdown (headings, bold, links, etc.).
    async fn markdown(&self) -> Result<String>;

    /// Take a screenshot and return PNG bytes.
    async fn screenshot(&self) -> Result<Vec<u8>>;

    /// Get the current page title.
    async fn title(&self) -> Result<String>;

    /// Get text content of all elements matching a CSS selector.
    async fn query_all(&self, selector: &str) -> Result<Vec<String>>;

    /// Wait for an element matching the CSS selector to appear.
    async fn wait_for(&self, selector: &str, timeout_ms: u64) -> Result<()>;

    /// Load sub-resources (JS, CSS, images) referenced by the current page.
    async fn load_sub_resources(&self) -> Result<usize>;

    /// Close the current page (creates a new blank page for next operation).
    async fn close(&self) -> Result<()>;
}