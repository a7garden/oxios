//! Browser API — browser backend facade.
//!
//! When the `browser` feature is enabled, holds an `Arc<oxibrowser_core::Browser>`
//! that can be shared with `BrowserTool` instances.

use std::sync::Arc;

/// Browser management system calls.
///
/// Wraps the embedded OxiBrowser engine. `BrowserTool` borrows the `Browser`
/// via `browser()` for agent tool calls.
#[cfg(feature = "browser")]
pub struct BrowserApi {
    browser: Arc<oxibrowser_core::Browser>,
}

#[cfg(feature = "browser")]
impl BrowserApi {
    /// Create a new BrowserApi by initializing a Browser from config.
    pub fn from_config(config: &oxibrowser_core::BrowserConfig) -> Self {
        let rt = tokio::runtime::Handle::current();
        let engine = config.clone();
        let browser = rt
            .block_on(oxibrowser_core::Browser::new(engine))
            .expect("Failed to initialize browser engine");
        Self {
            browser: Arc::new(browser),
        }
    }

    /// Create a new BrowserApi from an already-initialized Browser.
    pub fn new(browser: Arc<oxibrowser_core::Browser>) -> Self {
        Self { browser }
    }

    /// Browser engine reference.
    pub fn browser(&self) -> &Arc<oxibrowser_core::Browser> {
        &self.browser
    }

    /// Shut down the browser engine.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.browser.close().await?;
        Ok(())
    }
}

/// Default (no-op) placeholder for `from_subsystems` without browser.
#[cfg(feature = "browser")]
impl Default for BrowserApi {
    fn default() -> Self {
        panic!("BrowserApi::default() called with browser feature enabled — use KernelHandle::new() with a real BrowserApi");
    }
}

/// Zero-sized browser placeholder when the `browser` feature is disabled.
#[cfg(not(feature = "browser"))]
pub struct BrowserApi;

#[cfg(not(feature = "browser"))]
impl Default for BrowserApi {
    fn default() -> Self {
        BrowserApi
    }
}
