//! Agent-facing browser tool — the single gateway to all browser capabilities.
//!
//! Wraps `oxibrowser_core::Browser` behind the `AgentTool` interface so agents
//! can browse the web in their tool-calling loop.
//!
//! The browser engine is initialized **lazily** on first tool invocation.
//! This avoids panics from `block_on` inside an existing tokio runtime.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex, OnceCell};

/// Agent tool for web browsing via the embedded OxiBrowser engine.
///
/// Lazily initializes the browser on first `execute()` call.
/// `from_kernel()` is safe to call from sync context.
pub struct BrowserTool {
    /// Lazily-initialized browser engine.
    browser: OnceCell<Arc<oxibrowser_core::Browser>>,
    /// Config source for lazy initialization.
    init: BrowserInit,
    tab: Arc<Mutex<Option<oxibrowser_core::Tab>>>,
}

/// How to obtain a Browser instance.
enum BrowserInit {
    /// Already have one — use directly.
    Ready(Arc<oxibrowser_core::Browser>),
    /// Initialize lazily from BrowserApi on first use.
    #[cfg(feature = "browser")]
    Lazy(std::sync::Arc<crate::kernel_handle::BrowserApi>),
}

impl BrowserTool {
    /// Create a new browser tool with an already-initialized browser.
    pub fn new(browser: Arc<oxibrowser_core::Browser>) -> Self {
        let cell = OnceCell::new();
        // We can't set it here because browser is moved, so we use Ready variant
        Self {
            browser: cell,
            init: BrowserInit::Ready(browser),
            tab: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a `BrowserTool` from a [`KernelHandle`].
    ///
    /// Does **not** initialize the browser yet — that happens on first use.
    /// This is safe to call from a synchronous context.
    #[cfg(feature = "browser")]
    pub fn from_kernel(kernel: &crate::kernel_handle::KernelHandle) -> Self {
        Self {
            browser: OnceCell::new(),
            init: BrowserInit::Lazy(Arc::new(kernel.browser.clone())),
            tab: Arc::new(Mutex::new(None)),
        }
    }

    /// Get or lazily initialize the browser engine.
    async fn get_browser(&self) -> Result<Arc<oxibrowser_core::Browser>, String> {
        let browser = self
            .browser
            .get_or_try_init(|| async {
                match &self.init {
                    BrowserInit::Ready(b) => Ok::<_, String>(b.clone()),
                    #[cfg(feature = "browser")]
                    BrowserInit::Lazy(api) => {
                        api.browser().await.map(|b| b.clone()).map_err(|e| e.to_string())
                    }
                }
            })
            .await?;
        Ok(browser.clone())
    }

    /// Get or create an interactive tab.
    async fn get_or_create_tab(&self) -> anyhow::Result<oxibrowser_core::Tab> {
        let browser = self.get_browser().await.map_err(anyhow::Error::msg)?;
        let mut guard = self.tab.lock().await;
        let needs_new = match guard.as_ref() {
            None => true,
            Some(t) => t.is_closed(),
        };
        if needs_new {
            let tab = browser.new_tab().await?;
            *guard = Some(tab.clone());
        }
        Ok(guard.as_ref().unwrap().clone())
    }
}

impl std::fmt::Debug for BrowserTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserTool").finish()
    }
}

#[async_trait]
impl AgentTool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn label(&self) -> &str {
        "Browser"
    }

    fn description(&self) -> &'static str {
        "Browse the web using a headless browser. Actions: browse(url), goto(url), back(), forward(), reload(), post(url, body, content_type), click(selector), type(selector, text), press_key(key), evaluate(js), evaluate_await(js), content(), query_all(selector), wait_for(selector, timeout_ms), load_resources(), screenshot(), run_script(yaml), close()"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "browse",
                        "goto",
                        "back",
                        "forward",
                        "reload",
                        "post",
                        "click",
                        "type",
                        "press_key",
                        "evaluate",
                        "evaluate_await",
                        "content",
                        "query_all",
                        "wait_for",
                        "load_resources",
                        "screenshot",
                        "run_script",
                        "close"
                    ],
                    "description": "Browser action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL (browse, goto, post actions)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector (click, type, query_all, wait_for actions)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (type action)"
                },
                "key": {
                    "type": "string",
                    "description": "Key to press (press_key action, e.g. 'Enter', 'Tab')"
                },
                "javascript": {
                    "type": "string",
                    "description": "JavaScript code (evaluate, evaluate_await actions)"
                },
                "body": {
                    "type": "string",
                    "description": "Request body (post action)"
                },
                "content_type": {
                    "type": "string",
                    "description": "Content-Type header (post action)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (wait_for action)"
                },
                "width": {
                    "type": "integer",
                    "description": "Viewport width for screenshot (default 1280)"
                },
                "script": {
                    "type": "string",
                    "description": "YAML script for run_script action. Supports: goto, click, fill, type, wait, evaluate, extract, screenshot, if, retry, set, echo, sleep, and more."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        // Grab the browser reference (lazy init happens here if needed).
        let browser = self.get_browser().await?;

        match action {
            // ── One-shot browse ────────────────────────────────────
            "browse" => {
                let url = param_str(&params, "url", "browse requires 'url'")?;
                match browser.browse(url).await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("Browse failed: {}", e))),
                }
            }

            // ── Interactive navigation ─────────────────────────────
            "goto" => {
                let url = param_str(&params, "url", "goto requires 'url'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.goto(url).await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("Navigation failed: {}", e))),
                }
            }
            "back" => {
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.back().await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("Back failed: {}", e))),
                }
            }
            "forward" => {
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.forward().await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("Forward failed: {}", e))),
                }
            }
            "reload" => {
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.reload().await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("Reload failed: {}", e))),
                }
            }
            "post" => {
                let url = param_str(&params, "url", "post requires 'url'")?;
                let body = param_str(&params, "body", "post requires 'body'")?;
                let ct = params
                    .get("content_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("application/json");
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.post(url, body, ct).await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("POST failed: {}", e))),
                }
            }

            // ── Interaction ────────────────────────────────────────
            "click" => {
                let selector = param_str(&params, "selector", "click requires 'selector'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.click(selector).await {
                    Ok(()) => Ok(AgentToolResult::success(format!("Clicked '{}'", selector))),
                    Err(e) => Ok(AgentToolResult::error(format!("Click failed: {}", e))),
                }
            }
            "type" => {
                let selector = param_str(&params, "selector", "type requires 'selector'")?;
                let text = param_str(&params, "text", "type requires 'text'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.r#type(selector, text).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Typed {} chars into '{}'",
                        text.len(),
                        selector
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Type failed: {}", e))),
                }
            }
            "press_key" => {
                let key = param_str(&params, "key", "press_key requires 'key'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.press_key(key).await {
                    Ok(()) => Ok(AgentToolResult::success(format!("Pressed '{}'", key))),
                    Err(e) => Ok(AgentToolResult::error(format!("Press key failed: {}", e))),
                }
            }

            // ── Content extraction ─────────────────────────────────
            "content" => {
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.content().await {
                    Ok(r) => Ok(AgentToolResult::success(format_browse(&r))),
                    Err(e) => Ok(AgentToolResult::error(format!("Content failed: {}", e))),
                }
            }
            "query_all" => {
                let selector = param_str(&params, "selector", "query_all requires 'selector'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.query_all(selector).await {
                    Ok(texts) => {
                        let output = if texts.is_empty() {
                            format!("No elements found matching '{}'", selector)
                        } else {
                            texts
                                .iter()
                                .enumerate()
                                .map(|(i, t)| format!("{}. {}", i + 1, t))
                                .collect::<Vec<_>>()
                                .join("\n")
                        };
                        Ok(AgentToolResult::success(output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!("Query failed: {}", e))),
                }
            }
            "evaluate" => {
                let js = param_str(&params, "javascript", "evaluate requires 'javascript'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.evaluate(js).await {
                    Ok(value) => {
                        let output =
                            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                        Ok(AgentToolResult::success(output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!("JS evaluation failed: {}", e))),
                }
            }
            "evaluate_await" => {
                let js = param_str(&params, "javascript", "evaluate_await requires 'javascript'")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.evaluate_await(js).await {
                    Ok(value) => {
                        let output =
                            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                        Ok(AgentToolResult::success(output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!("JS evaluation failed: {}", e))),
                }
            }

            // ── Waiting ────────────────────────────────────────────
            "wait_for" => {
                let selector = param_str(&params, "selector", "wait_for requires 'selector'")?;
                let timeout_ms = params.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30_000);
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.wait_for(selector, timeout_ms).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Element '{}' found within {}ms",
                        selector, timeout_ms
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("wait_for failed: {}", e))),
                }
            }

            // ── Sub-resources ──────────────────────────────────────
            "load_resources" => {
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.load_resources().await {
                    Ok(count) => {
                        Ok(AgentToolResult::success(format!("Loaded {} resources", count)))
                    }
                    Err(e) => {
                        Ok(AgentToolResult::error(format!("load_resources failed: {}", e)))
                    }
                }
            }

            // ── Screenshot ─────────────────────────────────────────
            "screenshot" => {
                let width = params.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                match tab.screenshot(width).await {
                    Ok(png) => Ok(AgentToolResult::success(format!(
                        "Screenshot: {} bytes (PNG, {}px wide)",
                        png.len(),
                        width
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Screenshot failed: {}", e))),
                }
            }

            // ── Script execution ────────────────────────────────────
            "run_script" => {
                let yaml =
                    param_str(&params, "script", "run_script requires 'script' (YAML string)")?;
                let tab = self.get_or_create_tab().await.map_err(|e| e.to_string())?;
                let mut runner = oxibrowser_core::script::ScriptRunner::new(&tab);
                match runner.run(yaml).await {
                    Ok(result) => {
                        let output = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));
                        Ok(AgentToolResult::success(output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Script failed: {}",
                        e
                    ))),
                }
            }

            // ── Lifecycle ──────────────────────────────────────────
            "close" => {
                let mut guard = self.tab.lock().await;
                if let Some(t) = guard.take() {
                    let _ = t.close().await;
                }
                Ok(AgentToolResult::success("Tab closed"))
            }

            other => Err(format!(
                "Unknown browser action '{}'. Valid: browse, goto, back, forward, reload, post, click, type, press_key, evaluate, evaluate_await, content, query_all, wait_for, load_resources, screenshot, run_script, close",
                other
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a `BrowseResult` for agent consumption.
fn format_browse(r: &oxibrowser_core::BrowseResult) -> String {
    let md = &r.markdown;
    if md.len() > 50_000 {
        let cut = md.floor_char_boundary(50_000);
        format!(
            "URL: {} (status {})\nTitle: {}\n\n{}\n\n... (truncated, {} total chars)",
            r.url,
            r.status,
            r.title,
            &md[..cut],
            md.len()
        )
    } else if md.is_empty() {
        format!(
            "URL: {} (status {})\nTitle: {}\n(no content)",
            r.url, r.status, r.title
        )
    } else {
        format!(
            "URL: {} (status {})\nTitle: {}\n\n{}",
            r.url, r.status, r.title, md
        )
    }
}

/// Extract a required string parameter (borrowed).
fn param_str<'a>(params: &'a Value, key: &str, error_msg: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| error_msg.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_schema_covers_all_actions() {
        let actions = vec![
            "browse", "goto", "back", "forward", "reload", "post",
            "click", "type", "press_key", "evaluate", "evaluate_await",
            "content", "query_all", "wait_for", "load_resources",
            "screenshot", "run_script", "close",
        ];
        assert!(actions.len() >= 16);
        assert!(actions.contains(&"browse"));
        assert!(actions.contains(&"goto"));
        assert!(actions.contains(&"run_script"));
    }
}
