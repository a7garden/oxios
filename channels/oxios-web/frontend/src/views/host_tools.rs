//! Tool status showing required/optional availability.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn HostToolsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<api::HostToolsStatusResponse>("/api/host-tools").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(status)) => {
            let all_ok = status.all_required_present;
            let status_icon = if all_ok { "✅" } else { "⚠️" };
            let status_text = if all_ok {
                "All required tools available"
            } else {
                "Some required tools are missing"
            };

            let missing_text = if status.missing_required.is_empty() {
                "None".to_string()
            } else {
                status.missing_required.join(", ")
            };

            let optional_items: Vec<(String, bool)> = status.optional_available
                .iter()
                .map(|(k, &v)| (k.clone(), v))
                .collect();

            let optional_rows: Vec<Element> = optional_items.iter().map(|(name, available)| {
                let icon = if *available { "✅" } else { "❌" };
                let cls = if *available { "host-tool-item tool-available" } else { "host-tool-item tool-missing" };
                rsx! {
                    div { class: "{cls}", key: "{name}",
                        span { class: "tool-icon", "{icon}" }
                        span { class: "tool-name", "{name}" }
                    }
                }
            }).collect();

            rsx! {
                div {
                    div { class: "host-tools-summary",
                        span { class: "tool-icon", "{status_icon}" }
                        span { style: "font-weight:600", "{status_text}" }
                    }
                    div { class: "host-tools-section",
                        h3 { "Missing Required" }
                        div { class: "host-tool-item",
                            span { class: if status.missing_required.is_empty() { "tool-available" } else { "tool-missing" },
                                "{missing_text}"
                            }
                        }
                    }
                    div { class: "host-tools-section",
                        h3 { "Optional Tools" }
                        {optional_rows.into_iter()}
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
                p { "Loading host tools..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "🔧 Host Tools" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
