//! Session list with expand-to-detail and delete.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[component]
pub fn SessionsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::SessionListItem>("/api/sessions").await
    });

    let mut expanded_id = use_signal(|| Option::<String>::None);
    let mut detail_resource = use_signal(|| None::<serde_json::Value>);
    let mut loading_detail = use_signal(|| false);

    fn truncate_id(id: &str) -> String {
        if id.len() >= 8 {
            format!("{}…", &id[..8])
        } else {
            id.to_string()
        }
    }

    let list_content: Element = match &(resource.value())() {
        Some(Ok(sessions)) if sessions.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconDatabase { size: 40 } }
                p { "No sessions recorded." }
            }
        },
        Some(Ok(sessions)) => {
            let rows: Vec<Element> = sessions.iter().map(|session| {
                let id = session.id.clone();
                let short_id = truncate_id(&id);
                let is_expanded = expanded_id().as_ref() == Some(&id);

                rsx! {
                    div { class: "agent-card", key: "{id}",
                        div {
                            class: "session-row",
                            style: "display:flex;align-items:center;justify-content:space-between;cursor:pointer",
                            onclick: move |_| {
                                if expanded_id().as_ref() == Some(&id) {
                                    expanded_id.set(None);
                                } else {
                                    expanded_id.set(Some(id.clone()));
                                    loading_detail.set(true);
                                    let target_id = id.clone();
                                    spawn(async move {
                                        let detail =
                                            api::fetch_json::<serde_json::Value>(&format!("/api/sessions/{target_id}")).await;
                                        detail_resource.set(Some(detail.unwrap_or_default()));
                                        loading_detail.set(false);
                                    });
                                }
                            }
                        }
                        div { class: "agent-info",
                            div { class: "agent-name",
                                "{short_id}"
                            }
                            div { class: "agent-id", "User: {session.user_id} · Messages: {session.message_count}" }
                            div { class: "agent-id", "Created: {session.created_at} · Updated: {session.updated_at}" }
                            if let Some(ref seed) = session.active_seed_id {
                                div { class: "agent-id", "Active Seed: {seed}" }
                            }
                        }
                        div { class: "card-actions",
                            button {
                                class: "btn btn-danger btn-sm",
                                title: "Delete this session",
                                onclick: move |_| {
                                    let sid = id.clone();
                                    spawn(async move {
                                        let _ = api::delete_action(&format!("/api/sessions/{sid}")).await;
                                        resource.restart();
                                    });
                                },
                                IconTrash { size: 14 }
                            }
                        }
                        if is_expanded {
                            div { class: "session-detail",
                                if loading_detail() {
                                    div { class: "empty-state",
                                        div { class: "empty-icon", IconLoading { size: 20 } }
                                        p { "Loading details..." }
                                    }
                                } else if let Some(ref detail) = detail_resource() {
                                    pre {
                                        style: "font-size:11px;font-family:var(--font-mono);background:var(--bg-2);padding:8px;border-radius:var(--radius-sm);overflow:auto;max-height:200px",
                                        "{serde_json::to_string_pretty(detail).unwrap_or_default()}"
                                    }
                                } else {
                                    div { class: "empty-state", p { "No detail available." } }
                                }
                            }
                        }
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
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading sessions..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconDatabase { size: 20 } " Sessions" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {list_content}
            }
        }
    }
}