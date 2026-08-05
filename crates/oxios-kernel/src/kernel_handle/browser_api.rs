//! Browser API — headless browser engine facade (RFC: browser-migration).
//!
//! Wraps the SDK's pure-Rust `oxibrowser-core` engine behind a lazily
//! initialized [`BrowserEngine`]. The engine is created on first use and
//! shared across every agent run that holds the same [`KernelHandle`].
//!
//! The engine is only available with the `native-browser` feature. Without
//! it, [`BrowserApi::try_engine`] always returns `None` and no browse tools
//! are registered — the capability degrades gracefully to a no-op.
//!
//! [`BrowserEngine`]: oxicode_sdk::BrowserEngine

use std::sync::Arc;

use crate::config::OxiosConfig;

/// Headless browser facade.
///
/// Holds the SDK [`BrowseConfig`] and a lazily-initialized engine cell. The
/// concrete `OxicodeBrowserEngine` (pure-Rust: boa JS + html5ever + wreq) is
/// constructed only when [`engine`](Self::engine) is first awaited.
///
/// [`BrowseConfig`]: oxicode_sdk::BrowseConfig
pub struct BrowserApi {
    /// Lazily-initialized shared engine. `None` until first `engine().await`.
    #[cfg(feature = "native-browser")]
    engine: tokio::sync::OnceCell<Arc<dyn oxicode_sdk::BrowserEngine>>,
    /// Engine configuration (propagated to the backend on init).
    config: oxicode_sdk::BrowseConfig,
}

impl std::fmt::Debug for BrowserApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserApi")
            .field("config", &self.config)
            .finish()
    }
}

impl BrowserApi {
    /// Create a new browser facade with the given configuration.
    pub fn new(config: oxicode_sdk::BrowseConfig) -> Self {
        Self {
            #[cfg(feature = "native-browser")]
            engine: tokio::sync::OnceCell::new(),
            config,
        }
    }

    /// Build a [`BrowserApi`] from the kernel config, honoring `[browser].enabled`.
    ///
    /// Returns `None` when browser integration is disabled, so callers can
    /// short-circuit engine construction entirely.
    pub fn from_config(config: &OxiosConfig) -> Option<Self> {
        if config.browser.enabled {
            Some(Self::new(config.browser.engine.clone()))
        } else {
            None
        }
    }

    /// Lazily initialize and return the shared browser engine.
    ///
    /// The underlying `OxicodeBrowserEngine` (pure-Rust `oxibrowser-core`) is
    /// created exactly once; subsequent calls return the cached handle.
    /// A failed init is retryable — the cell stays empty on error.
    ///
    /// Only available with the `native-browser` feature.
    #[cfg(feature = "native-browser")]
    pub async fn engine(&self) -> anyhow::Result<Arc<dyn oxicode_sdk::BrowserEngine>> {
        self.engine
            .get_or_try_init(|| async {
                let backend = oxicode_sdk::OxicodeBrowserEngine::with_config(self.config.clone())
                    .await
                    .map_err(|e| anyhow::anyhow!("browser engine init failed: {e}"))?;
                Ok(Arc::new(backend) as Arc<dyn oxicode_sdk::BrowserEngine>)
            })
            .await
            .map(Arc::clone)
    }

    /// Synchronous accessor — returns the engine only if already initialized.
    ///
    /// Used by the (synchronous) tool registration path, which relies on the
    /// agent runtime having awaited [`engine`](Self::engine) first. Returns
    /// `None` if the engine has not been initialized (or the feature is off).
    #[cfg(feature = "native-browser")]
    pub fn try_engine(&self) -> Option<Arc<dyn oxicode_sdk::BrowserEngine>> {
        self.engine.get().cloned()
    }

    /// Without `native-browser`, no engine is ever available.
    #[cfg(not(feature = "native-browser"))]
    pub fn try_engine(&self) -> Option<Arc<dyn oxicode_sdk::BrowserEngine>> {
        None
    }
}
