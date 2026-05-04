//! Display TOML config with pre block.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn ConfigView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<serde_json::Value>("/api/config").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(value)) => {
            let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| "Failed to format".to_string());
            rsx! {
                div { class: "config-grid",
                    div { class: "config-section",
                        h3 { "Configuration" }
                        pre {
                            style: "font-family:var(--font-mono);font-size:13px;line-height:1.6;white-space:pre-wrap;word-break:break-word;color:var(--text-primary);",
                            "{pretty}"
                        }
                    }
                }
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "icon", "⏳" }
                p { "Loading configuration..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "⚙️ Config" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
