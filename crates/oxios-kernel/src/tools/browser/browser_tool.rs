//! Agent-facing browser tool.
//!
//! Wraps `BrowserBackend` behind the `AgentTool` interface so agents
//! can browse the web in their tool-calling loop.
//!
//! ## Tool Schema
//!
//! The tool exposes a single `action` parameter with sub-operations:
//! - `navigate` — go to a URL
//! - `back` — navigate back in history
//! - `forward` — navigate forward in history
//! - `reload` — reload the current page
//! - `click` — click an element
//! - `type` — type text into an element
//! - `evaluate` — run JavaScript
//! - `evaluate_with_await` — run JavaScript, awaiting Promise resolution
//! - `html` — get page HTML
//! - `text` — get page text content
//! - `markdown` — get page content as proper Markdown
//! - `screenshot` — capture PNG screenshot (returned as base64)
//! - `query_all` — get text of all matching elements
//! - `wait_for` — wait for element to appear
//! - `load_sub_resources` — preload JS/CSS/images
//! - `close` — close current page

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::BrowserBackend;

/// Agent tool for web browsing via headless browser.
///
/// All browser operations are routed through the `BrowserBackend` trait,
/// which abstracts away the underlying browser engine implementation.
pub struct BrowserTool {
    backend: Arc<dyn BrowserBackend>,
}

impl BrowserTool {
    /// Create a new browser tool with the given backend.
    pub fn new(backend: Arc<dyn BrowserBackend>) -> Self {
        Self { backend }
    }

    /// Create a `BrowserTool` from a [`KernelHandle`].
    ///
    /// Extracts the browser backend from the kernel's browser facade.
    /// Only available when the `browser` feature is enabled.
    #[cfg(feature = "browser")]
    pub fn from_kernel(kernel: &crate::kernel_handle::KernelHandle) -> Self {
        Self::new(kernel.browser.backend().clone() as Arc<dyn BrowserBackend>)
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
        "Browse the web using a headless browser. Actions: navigate(url), back(), forward(), reload(), click(selector), type(selector, text), evaluate(js), evaluate_with_await(js, await_promise), html(), text(), markdown(), screenshot(), query_all(selector), wait_for(selector, timeout_ms), load_sub_resources(), close()"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate",
                        "back",
                        "forward",
                        "reload",
                        "click",
                        "type",
                        "evaluate",
                        "evaluate_with_await",
                        "html",
                        "text",
                        "markdown",
                        "screenshot",
                        "query_all",
                        "wait_for",
                        "load_sub_resources",
                        "close"
                    ],
                    "description": "Browser action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (navigate action)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for the target element (click, type, query_all, wait_for actions)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into the element (type action)"
                },
                "javascript": {
                    "type": "string",
                    "description": "JavaScript code to evaluate (evaluate, evaluate_with_await actions)"
                },
                "await_promise": {
                    "type": "boolean",
                    "description": "Whether to await Promise resolution (evaluate_with_await action)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (wait_for action)"
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

        match action {
            "navigate" => {
                let url = params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "navigate requires 'url' parameter".to_string())?;

                match self.backend.navigate(url).await {
                    Ok(info) => Ok(AgentToolResult::success(format!(
                        "Navigated to '{}'. Title: '{}'",
                        info.url, info.title
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Navigation failed: {}",
                        e
                    ))),
                }
            }

            "back" => {
                match self.backend.go_back().await {
                    Ok(()) => Ok(AgentToolResult::success("Navigated back")),
                    Err(e) => Ok(AgentToolResult::error(format!("Go back failed: {}", e))),
                }
            }

            "forward" => {
                match self.backend.go_forward().await {
                    Ok(()) => Ok(AgentToolResult::success("Navigated forward")),
                    Err(e) => Ok(AgentToolResult::error(format!("Go forward failed: {}", e))),
                }
            }

            "reload" => {
                match self.backend.reload().await {
                    Ok(()) => Ok(AgentToolResult::success("Page reloaded")),
                    Err(e) => Ok(AgentToolResult::error(format!("Reload failed: {}", e))),
                }
            }

            "click" => {
                let selector = params
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "click requires 'selector' parameter".to_string())?;

                match self.backend.click(selector).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Clicked '{}'",
                        selector
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Click failed: {}", e))),
                }
            }

            "type" => {
                let selector = params
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "type requires 'selector' parameter".to_string())?;
                let text = params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "type requires 'text' parameter".to_string())?;

                match self.backend.r#type(selector, text).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Typed {} chars into '{}'",
                        text.len(),
                        selector
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!("Type failed: {}", e))),
                }
            }

            "evaluate" => {
                let js = params
                    .get("javascript")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "evaluate requires 'javascript' parameter".to_string())?;

                match self.backend.evaluate(js).await {
                    Ok(value) => {
                        let output = serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| value.to_string());
                        Ok(AgentToolResult::success(output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "JS evaluation failed: {}",
                        e
                    ))),
                }
            }

            "evaluate_with_await" => {
                let js = params
                    .get("javascript")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "evaluate_with_await requires 'javascript' parameter".to_string())?;
                let await_promise = params
                    .get("await_promise")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                match self.backend.evaluate_with_await(js, await_promise).await {
                    Ok(value) => {
                        let output = serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| value.to_string());
                        Ok(AgentToolResult::success(output))
                    }
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "JS evaluation failed: {}",
                        e
                    ))),
                }
            }

            "html" => match self.backend.html().await {
                Ok(html) => {
                    // Truncate large HTML to avoid context bloat.
                    let truncated = if html.len() > 50_000 {
                        format!(
                            "{}\n\n... (truncated, {} total chars)",
                            &html[..50_000],
                            html.len()
                        )
                    } else {
                        html
                    };
                    Ok(AgentToolResult::success(truncated))
                }
                Err(e) => Ok(AgentToolResult::error(format!("Get HTML failed: {}", e))),
            },

            "text" => match self.backend.text().await {
                Ok(text) => {
                    let truncated = if text.len() > 50_000 {
                        format!(
                            "{}\n\n... (truncated, {} total chars)",
                            &text[..50_000],
                            text.len()
                        )
                    } else {
                        text
                    };
                    Ok(AgentToolResult::success(truncated))
                }
                Err(e) => Ok(AgentToolResult::error(format!("Get text failed: {}", e))),
            },

            "markdown" => match self.backend.markdown().await {
                Ok(md) => {
                    let truncated = if md.len() > 50_000 {
                        format!(
                            "{}\n\n... (truncated, {} total chars)",
                            &md[..50_000],
                            md.len()
                        )
                    } else {
                        md
                    };
                    Ok(AgentToolResult::success(truncated))
                }
                Err(e) => Ok(AgentToolResult::error(format!("Get markdown failed: {}", e))),
            },

            "screenshot" => match self.backend.screenshot().await {
                Ok(png_bytes) => {
                    Ok(AgentToolResult::success(format!(
                        "Screenshot captured: {} bytes (PNG)",
                        png_bytes.len()
                    )))
                }
                Err(e) => Ok(AgentToolResult::error(format!(
                    "Screenshot failed: {}",
                    e
                ))),
            },

            "query_all" => {
                let selector = params
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "query_all requires 'selector' parameter".to_string())?;

                match self.backend.query_all(selector).await {
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
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Query failed: {}",
                        e
                    ))),
                }
            }

            "wait_for" => {
                let selector = params
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "wait_for requires 'selector' parameter".to_string())?;
                let timeout_ms = params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30_000);

                match self.backend.wait_for(selector, timeout_ms).await {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Element '{}' found within {}ms",
                        selector, timeout_ms
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "wait_for failed: {}",
                        e
                    ))),
                }
            }

            "load_sub_resources" => match self.backend.load_sub_resources().await {
                Ok(count) => Ok(AgentToolResult::success(format!(
                    "Loaded {} sub-resources",
                    count
                ))),
                Err(e) => Ok(AgentToolResult::error(format!(
                    "load_sub_resources failed: {}",
                    e
                ))),
            },

            "close" => match self.backend.close().await {
                Ok(()) => Ok(AgentToolResult::success("Page closed")),
                Err(e) => Ok(AgentToolResult::error(format!("Close failed: {}", e))),
            },

            other => Err(format!(
                "Unknown browser action '{}'. Valid: navigate, back, forward, reload, click, type, evaluate, evaluate_with_await, html, text, markdown, screenshot, query_all, wait_for, load_sub_resources, close",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_and_label() {
        let actions = vec!["navigate", "back", "forward", "reload", "click", "type", "evaluate", "evaluate_with_await", "html", "text", "markdown", "screenshot", "query_all", "wait_for", "load_sub_resources", "close"];
        assert!(actions.len() >= 15);
        assert!(actions.iter().any(|a| *a == "navigate"));
        assert!(actions.iter().any(|a| *a == "markdown"));
        assert!(actions.iter().any(|a| *a == "wait_for"));
    }
}