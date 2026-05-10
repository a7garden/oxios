//! Audit log table with allow/deny coloring.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn SecurityView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::AuditLogEntry>>("/api/audit").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(entries)) if entries.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "icon", "🔒" }
                p { "No audit log entries yet." }
            }
        },
        Some(Ok(entries)) => {
            let rows: Vec<Element> = entries.iter().map(|entry| {
                let timestamp = entry.timestamp.clone();
                let agent = entry.agent_name.clone();
                let action = entry.action.clone();
                let resource_name = entry.resource.clone();
                let allowed = entry.allowed;
                let reason = entry.reason.clone().unwrap_or_default();

                let status_class = if allowed {
                    "status-badge status-badge-active"
                } else {
                    "status-badge status-badge-inactive"
                };
                let status_text = if allowed { "Allow" } else { "Deny" };

                rsx! {
                    div { class: "agent-card", key: "{timestamp}-{action}-{agent}",
                        div { class: "agent-info",
                            div { class: "agent-name",
                                span { style: "color:var(--text-primary)", "{agent}" }
                                span { style: "color:var(--text-muted);margin:0 8px", "→" }
                                span { style: "color:var(--accent)", "{action}" }
                                span { style: "color:var(--text-muted);margin:0 8px", "on" }
                                span { style: "color:var(--text-primary)", "{resource_name}" }
                            }
                            div { class: "agent-id", "{timestamp} · {reason}" }
                        }
                        span { class: "{status_class}", "{status_text}" }
                    }
                }
            }).collect();
            rsx! { div { {rows.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "icon", "⏳" }
                p { "Loading audit log..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "🔒 Security" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
