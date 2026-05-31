//! Browser API — browser backend facade.
//!
//! When the `browser` feature is enabled, lazily initializes an
//! `Arc<oxibrowser_core::Browser>` on first use from an async context.
//! This avoids the `block_on` panic that occurs when constructing
//! inside an existing tokio runtime.

#[cfg(feature = "browser")]
use std::sync::Arc;

/// Browser management system calls.
///
/// Wraps the embedded OxiBrowser engine. `BrowserTool` borrows the `Browser`
/// via `browser()` for agent tool calls.
///
/// Initialization is **lazy**: the browser engine is only created on the
/// first call to [`browser()`](Self::browser) from an async context.
/// [`from_config()`](Self::from_config) and [`Default::default()`] are
/// cheap and never panic.
#[cfg(feature = "browser")]
pub struct BrowserApi {
    inner: tokio::sync::OnceCell<Arc<oxibrowser_core::Browser>>,
    config: Option<oxibrowser_core::BrowserConfig>,
}

#[cfg(feature = "browser")]
impl Clone for BrowserApi {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(feature = "browser")]
impl BrowserApi {
    /// Create a new BrowserApi that will lazily initialize a Browser from config.
    ///
    /// This is cheap — no I/O or runtime calls happen here.
    pub fn from_config(config: &oxibrowser_core::BrowserConfig) -> Self {
        Self {
            inner: tokio::sync::OnceCell::new(),
            config: Some(config.clone()),
        }
    }

    /// Create a new BrowserApi from an already-initialized Browser.
    pub fn new(browser: Arc<oxibrowser_core::Browser>) -> Self {
        let cell = tokio::sync::OnceCell::new();
        // SAFETY: cell was just created, it's empty.
        match cell.set(browser) {
            Ok(()) => {}
            Err(_) => unreachable!("OnceCell was just created"),
        }
        Self {
            inner: cell,
            config: None,
        }
    }

    /// Get the browser engine, initializing it lazily if needed.
    ///
    /// Must be called from an async context (tokio runtime active).
    /// Returns `Err` if no config was provided and the browser was not
    /// pre-initialized.
    pub async fn browser(&self) -> anyhow::Result<&Arc<oxibrowser_core::Browser>> {
        self.inner
            .get_or_try_init(|| async {
                let config = self.config.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("BrowserApi has no config and was not pre-initialized")
                })?;
                let browser = oxibrowser_core::Browser::new(config.clone())
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to initialize browser engine: {e}"))?;
                Ok(Arc::new(browser))
            })
            .await
    }

    /// Shut down the browser engine (if it was initialized).
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        if let Some(browser) = self.inner.get() {
            browser.close().await?;
        }
        Ok(())
    }
}

/// Default — creates an uninitialized BrowserApi.
///
/// This is used by `from_subsystems` and when `browser.enabled = false`.
/// It will return an error on [`browser()`](Self::browser) if no config
/// is ever set, but will **not panic**.
#[cfg(feature = "browser")]
impl Default for BrowserApi {
    fn default() -> Self {
        Self {
            inner: tokio::sync::OnceCell::new(),
            config: None,
        }
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
