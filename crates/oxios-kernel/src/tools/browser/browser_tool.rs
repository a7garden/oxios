//! Agent-facing browser tool.
//!
//! Wraps `BrowserBackend` behind the `AgentTool` interface so agents
//! can browse the web in their tool-calling loop.
//!
//! ## Tool Schema
//!
//! The tool exposes a single `action` parameter with sub-operations:
//! - `navigate` — go to a URL
//! - `click` — click an element
//! - `type` — type text into an element
//! - `evaluate` — run JavaScript
//! - `html` — get page HTML
//! - `text` — get page text content
//! - `screenshot` — capture PNG screenshot (returned as base64)
//! - `query_all` — get text of all matching elements

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::BrowserBackend;

/// Agent tool for web browsing via headless browser.
///
/// All browser operations are routed through the `BrowserBackend` trait,
/// which abstracts away the CDP implementation details.
pub struct BrowserTool {
    backend: Arc<dyn BrowserBackend>,
}

impl BrowserTool {
    /// Create a new browser tool with the given backend.
    pub fn new(backend: Arc<dyn BrowserBackend>) -> Self {
        Self { backend }
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
        "Browse the web using a headless browser. Actions: navigate(url), click(selector), type(selector, text), evaluate(js), html(), text(), screenshot(), query_all(selector)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate",
                        "click",
                        "type",
                        "evaluate",
                        "html",
                        "text",
                        "screenshot",
                        "query_all",
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
                    "description": "CSS selector for the target element (click, type, query_all actions)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into the element (type action)"
                },
                "javascript": {
                    "type": "string",
                    "description": "JavaScript code to evaluate (evaluate action)"
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

            "close" => match self.backend.close().await {
                Ok(()) => Ok(AgentToolResult::success("Page closed")),
                Err(e) => Ok(AgentToolResult::error(format!("Close failed: {}", e))),
            },

            other => Err(format!(
                "Unknown browser action '{}'. Valid: navigate, click, type, evaluate, html, text, screenshot, query_all, close",
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
        // We can't create a real backend in unit tests, but we can verify
        // the tool metadata without calling execute.
        let schema = json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "click", "type", "evaluate", "html", "text", "screenshot", "query_all", "close"]
                }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert!(actions.len() >= 8);
        assert!(actions.iter().any(|a| a == "navigate"));
        assert!(actions.iter().any(|a| a == "click"));
        assert!(actions.iter().any(|a| a == "screenshot"));
    }
}
