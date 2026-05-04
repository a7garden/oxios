//! Persona cards with active badge and set-active button.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn PersonasView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::PersonaSummary>>("/api/personas").await
    });

    let mut active_resource = use_resource(|| async move {
        api::fetch_json::<serde_json::Value>("/api/personas/active").await
    });

    let active_id: Option<String> = match &(active_resource.value())() {
        Some(Ok(val)) => val.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    };

    let content: Element = match &(resource.value())() {
        Some(Ok(personas)) if personas.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "icon", "🎭" }
                p { "No personas configured." }
            }
        },
        Some(Ok(personas)) => {
            let cards: Vec<Element> = personas.iter().map(|persona| {
                let id = persona.id.clone();
                let is_active = active_id.as_ref() == Some(&id);
                let traits = persona.personality_traits.join(", ");
                let set_active_id = id.clone();

                let active_badge: Element = if is_active {
                    rsx! { span { class: "garden-status-badge garden-status-running", "Active" } }
                } else {
                    rsx! { div {} }
                };

                let action_btn: Element = if is_active {
                    rsx! { div {} }
                } else {
                    let sid = set_active_id.clone();
                    rsx! {
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                let s = sid.clone();
                                spawn(async move {
                                    let _ = api::put_json::<serde_json::Value, _>(
                                        "/api/personas/active",
                                        &serde_json::json!({ "id": s }),
                                    ).await;
                                    resource.restart();
                                    active_resource.restart();
                                });
                            },
                            "Set Active"
                        }
                    }
                };

                rsx! {
                    div { class: "agent-card", key: "{id}",
                        div { class: "agent-info",
                            div { class: "agent-name",
                                "{persona.name}"
                                {active_badge}
                            }
                            div { class: "agent-id", "{persona.role} · {traits}" }
                            div { class: "agent-id", "{persona.description}" }
                        }
                        div { class: "garden-actions",
                            {action_btn}
                        }
                    }
                }
            }).collect();
            rsx! { div { {cards.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "icon", "⏳" }
                p { "Loading personas..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "🎭 Personas" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        resource.restart();
                        active_resource.restart();
                    },
                    "Refresh"
                }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
