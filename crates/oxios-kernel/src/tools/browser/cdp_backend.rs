//! CDP-based browser backend using chromiumoxide.
//!
//! Connects to a Lightpanda CDP server via WebSocket and provides
//! the full `BrowserBackend` trait implementation.

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Browser;
use chromiumoxide::Page;
use futures::StreamExt;
use tokio::sync::Mutex;

use super::{BrowserBackend, PageInfo};
use crate::tools::browser::process::LightpandaProcess;

/// CDP-based browser backend.
///
/// Manages a connection to a Lightpanda CDP server and provides
/// browser operations through the `BrowserBackend` trait.
pub struct CdpBackend {
    /// The Lightpanda process manager.
    process: Arc<LightpandaProcess>,
    /// The chromiumoxide browser instance (lazy-initialized).
    browser: Arc<Mutex<Option<Browser>>>,
    /// The active page (lazy-initialized, one page at a time).
    page: Arc<Mutex<Option<Page>>>,
}

impl CdpBackend {
    /// Create a new CDP backend connected to the given Lightpanda process.
    pub fn new(process: Arc<LightpandaProcess>) -> Self {
        Self {
            process,
            browser: Arc::new(Mutex::new(None)),
            page: Arc::new(Mutex::new(None)),
        }
    }

    /// Ensure the browser is connected and a page is available.
    async fn ensure_page(&self) -> Result<Page> {
        // 1. Ensure the Lightpanda process is running.
        self.process.ensure_running().await?;

        // 2. Ensure the Browser instance exists.
        {
            let mut browser_guard = self.browser.lock().await;
            if browser_guard.is_none() {
                let ws_endpoint = self.process.config().ws_endpoint();
                tracing::info!(endpoint = %ws_endpoint, "Connecting to Lightpanda CDP");

                let (browser, mut handler) = Browser::connect(ws_endpoint)
                    .await
                    .context("Failed to connect to Lightpanda CDP server")?;

                // Spawn the handler task to process CDP events.
                tokio::spawn(async move {
                    while let Some(event) = handler.next().await {
                        // Process CDP events — required for the browser to work.
                        let _ = event;
                    }
                    tracing::debug!("CDP handler task finished");
                });

                *browser_guard = Some(browser);
            }
        }

        // 3. Ensure a page is available.
        {
            let mut page_guard = self.page.lock().await;
            if page_guard.is_none() {
                let browser_guard = self.browser.lock().await;
                let browser = browser_guard
                    .as_ref()
                    .context("Browser not initialized")?;

                let page = browser
                    .new_page("about:blank")
                    .await
                    .context("Failed to create new page")?;

                *page_guard = Some(page);
            }

            page_guard
                .clone()
                .context("Page not initialized")
        }
    }
}

#[async_trait]
impl BrowserBackend for CdpBackend {
    async fn navigate(&self, url: &str) -> Result<PageInfo> {
        let page = self.ensure_page().await?;

        tracing::debug!(url = %url, "Navigating to URL");

        page.goto(url)
            .await
            .with_context(|| format!("Failed to navigate to '{}'", url))?;

        let title = page.get_title().await?.unwrap_or_default();

        let current_url: String = page
            .evaluate("window.location.href")
            .await
            .ok()
            .and_then(|r| r.into_value().ok())
            .unwrap_or_else(|| url.to_string());

        Ok(PageInfo {
            title,
            url: current_url,
        })
    }

    async fn click(&self, selector: &str) -> Result<()> {
        let page = self.ensure_page().await?;

        tracing::debug!(selector = %selector, "Clicking element");

        page.find_element(selector)
            .await
            .with_context(|| format!("Element '{}' not found", selector))?
            .click()
            .await
            .with_context(|| format!("Failed to click '{}'", selector))?;

        Ok(())
    }

    async fn r#type(&self, selector: &str, text: &str) -> Result<()> {
        let page = self.ensure_page().await?;

        tracing::debug!(selector = %selector, text_len = text.len(), "Typing into element");

        page.find_element(selector)
            .await
            .with_context(|| format!("Element '{}' not found", selector))?
            .type_str(text)
            .await
            .with_context(|| format!("Failed to type into '{}'", selector))?;

        Ok(())
    }

    async fn evaluate(&self, js: &str) -> Result<serde_json::Value> {
        let page = self.ensure_page().await?;

        tracing::debug!(js_len = js.len(), "Evaluating JavaScript");

        let result = page
            .evaluate(js)
            .await
            .context("JavaScript evaluation failed")?
            .into_value()
            .context("Failed to deserialize JS result")?;

        Ok(result)
    }

    async fn html(&self) -> Result<String> {
        let page = self.ensure_page().await?;

        let html: String = page
            .evaluate("document.documentElement.outerHTML")
            .await
            .context("Failed to get page HTML")?
            .into_value()
            .unwrap_or_default();

        Ok(html)
    }

    async fn text(&self) -> Result<String> {
        let page = self.ensure_page().await?;

        let text: String = page
            .evaluate("document.body.innerText")
            .await
            .context("Failed to get page text")?
            .into_value()
            .unwrap_or_default();

        Ok(text)
    }

    async fn screenshot(&self) -> Result<Vec<u8>> {
        let page = self.ensure_page().await?;

        tracing::debug!("Taking screenshot");

        let png_bytes = page
            .screenshot(ScreenshotParams::builder().build())
            .await
            .context("Screenshot failed")?;

        Ok(png_bytes)
    }

    async fn title(&self) -> Result<String> {
        let page = self.ensure_page().await?;

        let title = page.get_title().await?.unwrap_or_default();

        Ok(title)
    }

    async fn query_all(&self, selector: &str) -> Result<Vec<String>> {
        let page = self.ensure_page().await?;

        let js = format!(
            "JSON.stringify(Array.from(document.querySelectorAll('{}')).map(el => el.textContent.trim()))",
            selector.replace('\'', "\\'")
        );

        let json_str: String = page
            .evaluate(js.as_str())
            .await
            .with_context(|| format!("Query '{}' failed", selector))?
            .into_value()
            .unwrap_or_default();

        let texts: Vec<String> = serde_json::from_str(&json_str).unwrap_or_default();
        Ok(texts)
    }

    async fn close(&self) -> Result<()> {
        let mut page_guard = self.page.lock().await;
        if let Some(page) = page_guard.take() {
            // close() takes ownership (self), so it consumes the Page.
            let _ = page.close().await;
        }
        Ok(())
    }
}

impl std::fmt::Debug for CdpBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpBackend")
            .field("ws_endpoint", &self.process.config().ws_endpoint())
            .finish()
    }
}
