//! OxiBrowser-based browser backend.
//!
//! Uses the `oxibrowser-core` crate directly — a pure Rust headless browser
//! with html5ever (HTML parsing) and boa_engine (JavaScript). No external
//! process or CDP overhead required.

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use super::{BrowserBackend, PageInfo};

/// OxiBrowser backend using the embedded `oxibrowser-core` engine.
///
/// Owns an `oxibrowser_core::Browser` instance and creates sessions on demand.
/// No external process is needed — everything runs in-process.
pub struct OxibrowserBackend {
    browser: Arc<Mutex<Option<oxibrowser_core::Browser>>>,
    session: Arc<Mutex<Option<Arc<tokio::sync::RwLock<oxibrowser_core::session::Session>>>>>,
    config: OxibrowserConfig,
}

/// Configuration for the embedded OxiBrowser backend.
#[derive(Debug, Clone)]
pub struct OxibrowserConfig {
    /// User-Agent string.
    pub user_agent: Option<String>,
    /// Default navigation timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum concurrent sessions.
    pub max_sessions: usize,
    /// Cookie persistence file path.
    pub cookie_file: Option<String>,
}

impl Default for OxibrowserConfig {
    fn default() -> Self {
        Self {
            user_agent: None,
            timeout_secs: 30,
            max_sessions: 10,
            cookie_file: None,
        }
    }
}

impl OxibrowserBackend {
    /// Create a new OxiBrowser backend with the given configuration.
    pub fn new(config: OxibrowserConfig) -> Self {
        Self {
            browser: Arc::new(Mutex::new(None)),
            session: Arc::new(Mutex::new(None)),
            config,
        }
    }

    /// Ensure the browser is initialized and a session exists.
    ///
    /// If the current session is closed (e.g. after a fatal navigation error),
    /// a fresh one is created automatically.
    async fn ensure_session(
        &self,
    ) -> Result<Arc<tokio::sync::RwLock<oxibrowser_core::session::Session>>> {
        // 1. Ensure browser exists.
        {
            let mut browser_guard = self.browser.lock().await;
            if browser_guard.is_none() {
                tracing::info!("Initializing OxiBrowser engine");

                let mut config = oxibrowser_core::BrowserConfig::headless();
                if let Some(ref ua) = self.config.user_agent {
                    config.user_agent = ua.clone();
                }
                config.max_sessions = self.config.max_sessions;
                config.navigation_timeout_ms = self.config.timeout_secs * 1000;
                if let Some(ref path) = self.config.cookie_file {
                    config.cookie_file = Some(std::path::PathBuf::from(path));
                }

                let browser = oxibrowser_core::Browser::new(config)
                    .await
                    .context("Failed to initialize OxiBrowser engine")?;

                *browser_guard = Some(browser);
            }
        }

        // 2. Ensure a live session exists. Recreate if the previous one died.
        {
            let mut session_guard = self.session.lock().await;
            let needs_new = match session_guard.as_ref() {
                None => true,
                Some(s) => s.read().await.is_closed(),
            };

            if needs_new {
                let browser_guard = self.browser.lock().await;
                let browser = browser_guard
                    .as_ref()
                    .context("Browser not initialized")?;

                let session = browser
                    .new_session()
                    .await
                    .context("Failed to create OxiBrowser session")?;

                *session_guard = Some(session);
            }

            session_guard
                .clone()
                .context("Session not initialized")
        }
    }

    /// Shut down the browser engine.
    pub async fn shutdown(&self) -> Result<()> {
        let mut session_guard = self.session.lock().await;
        *session_guard = None;

        let mut browser_guard = self.browser.lock().await;
        if let Some(browser) = browser_guard.take() {
            browser.close().await?;
            tracing::info!("OxiBrowser engine shut down");
        }
        Ok(())
    }
}

#[async_trait]
impl BrowserBackend for OxibrowserBackend {
    async fn navigate(&self, url: &str) -> Result<PageInfo> {
        let session_arc = self.ensure_session().await?;
        let mut session = session_arc.write().await;

        session
            .navigate_with_retry(url, 2)
            .await
            .with_context(|| format!("Failed to navigate to '{}'", url))?;

        let page = session.page().context("No page loaded after navigation")?;
        Ok(PageInfo {
            title: page.title().unwrap_or("").to_string(),
            url: page.url().to_string(),
        })
    }

    async fn click(&self, selector: &str) -> Result<()> {
        let session_arc = self.ensure_session().await?;
        let mut session = session_arc.write().await;

        // Use JSON-serialized selector to prevent JS injection.
        // Returns {ok: true} on success or {error: "..."} on failure so the
        // agent gets a clear error rather than a silent no-op.
        let js = format!(
            r#"(function() {{
                var el = document.querySelector({selector});
                if (!el) return JSON.stringify({{error: 'element not found'}});
                el.click();
                return JSON.stringify({{ok: true}});
            }})()"#,
            selector = serde_json::to_string(selector)?
        );
        let result = session
            .evaluate_js(&js)
            .await
            .with_context(|| format!("Failed to click '{}'", selector))?;

        if let Some(exception) = &result.exception {
            anyhow::bail!("Click failed: {}", exception);
        }

        // Check the returned value for application-level errors.
        if let Some(Value::String(s)) = &result.value {
            if let Ok(obj) = serde_json::from_str::<serde_json::Map<String, Value>>(s) {
                if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
                    anyhow::bail!("Click failed: {}", err);
                }
            }
        }

        tracing::debug!(selector = %selector, "Clicked element");
        Ok(())
    }

    async fn r#type(&self, selector: &str, text: &str) -> Result<()> {
        let session_arc = self.ensure_session().await?;
        let mut session = session_arc.write().await;

        let js = format!(
            r#"(function() {{
                var el = document.querySelector({selector});
                if (!el) return JSON.stringify({{error: 'element not found'}});
                el.value = {value};
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return JSON.stringify({{ok: true}});
            }})()"#,
            selector = serde_json::to_string(selector)
                .context("Failed to serialize selector")?,
            value = serde_json::to_string(text)
                .context("Failed to serialize text")?,
        );
        let result = session
            .evaluate_js(&js)
            .await
            .with_context(|| format!("Failed to type into '{}'", selector))?;

        if let Some(exception) = &result.exception {
            anyhow::bail!("Type failed: {}", exception);
        }

        // Check the returned value for application-level errors.
        if let Some(Value::String(s)) = &result.value {
            if let Ok(obj) = serde_json::from_str::<serde_json::Map<String, Value>>(s) {
                if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
                    anyhow::bail!("Type failed: {}", err);
                }
            }
        }

        tracing::debug!(selector = %selector, text_len = text.len(), "Typed into element");
        Ok(())
    }

    async fn evaluate(&self, js: &str) -> Result<serde_json::Value> {
        let session_arc = self.ensure_session().await?;
        let mut session = session_arc.write().await;

        let result = session
            .evaluate_js(js)
            .await
            .context("JavaScript evaluation failed")?;

        if let Some(exception) = &result.exception {
            anyhow::bail!("JS exception: {}", exception);
        }

        Ok(result.value.unwrap_or(serde_json::Value::Null))
    }

    async fn html(&self) -> Result<String> {
        let session_arc = self.ensure_session().await?;
        let session = session_arc.read().await;

        let page = session
            .page()
            .context("No page loaded — navigate first")?;

        Ok(page.content().to_string())
    }

    async fn text(&self) -> Result<String> {
        let session_arc = self.ensure_session().await?;
        let session = session_arc.read().await;

        let page = session
            .page()
            .context("No page loaded — navigate first")?;

        let frame = page.root_frame();
        let doc = frame.document();
        let text = doc.query_text("body").unwrap_or_default();

        Ok(text)
    }

    async fn screenshot(&self) -> Result<Vec<u8>> {
        // OxiBrowser is a headless DOM-only engine — no rendering pipeline.
        anyhow::bail!("Screenshots are not supported: OxiBrowser is a DOM-only engine without a rendering pipeline");
    }

    async fn title(&self) -> Result<String> {
        let session_arc = self.ensure_session().await?;
        let session = session_arc.read().await;

        let page = session
            .page()
            .context("No page loaded — navigate first")?;

        Ok(page.title().unwrap_or("").to_string())
    }

    async fn query_all(&self, selector: &str) -> Result<Vec<String>> {
        let session_arc = self.ensure_session().await?;
        let session = session_arc.read().await;

        let page = session
            .page()
            .context("No page loaded — navigate first")?;

        let frame = page.root_frame();
        let doc = frame.document();

        let node_ids = doc.query_selector_all(selector);
        let texts: Vec<String> = node_ids
            .iter()
            .filter_map(|id| doc.text_content(*id))
            .map(|t: String| t.trim().to_string())
            .filter(|t: &String| !t.is_empty())
            .collect();

        Ok(texts)
    }

    async fn close(&self) -> Result<()> {
        let mut session_guard = self.session.lock().await;
        *session_guard = None;
        Ok(())
    }
}

impl std::fmt::Debug for OxibrowserBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxibrowserBackend")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OxibrowserConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_sessions, 10);
        assert!(config.user_agent.is_none());
        assert!(config.cookie_file.is_none());
    }

    #[test]
    fn test_custom_config() {
        let config = OxibrowserConfig {
            user_agent: Some("test-agent".to_string()),
            timeout_secs: 60,
            max_sessions: 5,
            cookie_file: Some("/tmp/cookies.json".to_string()),
        };
        assert_eq!(config.user_agent.as_deref(), Some("test-agent"));
        assert_eq!(config.timeout_secs, 60);
    }
}
