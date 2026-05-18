//! Space management — list, activate, archive, merge, and knowledge flow.
//!
//! Spaces are Oxios's context-partitioning concept. Each Space isolates
//! conversations, knowledge, and agent interactions into a separate domain.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

// ---------------------------------------------------------------------------
// Space View
// ---------------------------------------------------------------------------

#[component]
pub fn SpacesView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<SpacesListResponse>("/api/spaces").await
    });

    let mut current_resource = use_resource(|| async move {
        api::fetch_json::<Option<api::SpaceInfo>>("/api/spaces/current").await
    });

    let mut flow_resource = use_resource(|| async move {
        api::fetch_json::<KnowledgeFlowResponse>("/api/spaces/knowledge-flow").await
    });

    let _selected_id = use_signal(|| None::<String>);
    let mut status_msg = use_signal(|| None::<String>);

    let current_space: Option<api::SpaceInfo> = match &(current_resource.value())() {
        Some(Ok(Some(s))) => Some(s.clone()),
        _ => None,
    };

    let content: Element = match &(resource.value())() {
        Some(Ok(resp)) if resp.items.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLayers { size: 40 } }
                p { "No spaces created yet. Spaces are created through conversations." }
            }
        },
        Some(Ok(resp)) => {
            let cards: Vec<Element> = resp.items.iter().map(|space| {
                let id = space.id.clone();
                let name = space.name.clone();
                let is_active = space.active;
                let interaction_count = space.interaction_count;
                let last_active = space.last_active.clone();
                let paths = space.paths.join(", ");

                let activate_id = id.clone();
                let archive_id = id.clone();

                let badge: Element = if is_active {
                    rsx! { span { class: "status-badge status-badge-active", "Active" } }
                } else {
                    rsx! { div {} }
                };

                let activate_btn: Element = if !is_active {
                    let aid = activate_id.clone();
                    rsx! {
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                let sid = aid.clone();
                                status_msg.set(None);
                                spawn(async move {
                                    match api::post_json::<serde_json::Value, ()>(
                                        &format!("/api/spaces/{sid}/activate"), &(),
                                    ).await {
                                        Ok(_) => {
                                            status_msg.set(Some("✓ Space activated".to_string()));
                                            resource.restart();
                                            current_resource.restart();
                                        }
                                        Err(e) => status_msg.set(Some(format!("✗ {e}"))),
                                    }
                                });
                            },
                            "Activate"
                        }
                    }
                } else {
                    rsx! { div {} }
                };

                rsx! {
                    div { class: "agent-card", key: "{id}",
                        div { class: "agent-info",
                            div { class: "agent-name",
                                "{name}"
                                {badge}
                            }
                            div { class: "agent-id",
                                "Interactions: {interaction_count} · Last: {last_active}"
                            }
                            if !paths.is_empty() {
                                div { class: "agent-id", "Paths: {paths}" }
                            }
                        }
                        div { class: "card-actions",
                            {activate_btn}
                            if !is_active {
                                {
                                    let arid = archive_id.clone();
                                    rsx! {
                                        button {
                                            class: "btn btn-danger btn-sm",
                                            onclick: move |_| {
                                                let sid = arid.clone();
                                                status_msg.set(None);
                                                spawn(async move {
                                                    match api::post_json::<serde_json::Value, ()>(
                                                        &format!("/api/spaces/{sid}/archive"), &(),
                                                    ).await {
                                                        Ok(_) => {
                                                            status_msg.set(Some("✓ Space archived".to_string()));
                                                            resource.restart();
                                                            current_resource.restart();
                                                        }
                                                        Err(e) => status_msg.set(Some(format!("✗ {e}"))),
                                                    }
                                                });
                                            },
                                            "Archive"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }).collect();
            rsx! { div { {cards.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "error-box", { format!("Error: {e}") } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading spaces..." }
            }
        },
    };

    // Current space highlight
    let current_highlight: Element = match &current_space {
        Some(cs) => rsx! {
            div { class: "stat-card mb-16",
                div { class: "stat-label", "Active Space" }
                div { class: "stat-value purple", "{cs.name}" }
                div { class: "text-xs text-muted mt-8", "ID: {cs.id}" }
            }
        },
        None => rsx! {
            div { class: "stat-card mb-16",
                div { class: "stat-label", "Active Space" }
                div { class: "text-muted text-sm", "No active space" }
            }
        },
    };

    // Knowledge flow
    let flow_content: Element = match &(flow_resource.value())() {
        Some(Ok(resp)) if resp.items.is_empty() => rsx! {
            div { class: "text-muted text-sm mt-8", "No knowledge flow entries yet." }
        },
        Some(Ok(resp)) => {
            let rows: Vec<Element> = resp.items.iter().map(|flow| {
                rsx! {
                    div { class: "agent-card", key: "{flow.from}-{flow.to}",
                        div { class: "agent-info",
                            div { class: "agent-name",
                                span { class: "text-accent", "{flow.from}" }
                                span { style: "color:var(--text-3);margin:0 8px", "→" }
                                span { class: "text-accent", "{flow.to}" }
                                span { class: "status-badge status-badge-active", style: "margin-left:8px", "{flow.flow_type}" }
                            }
                            div { class: "agent-id", "Entries: {flow.entry_count} · {flow.timestamp}" }
                        }
                    }
                }
            }).collect();
            rsx! {
                div { class: "mt-16",
                    h3 { class: "text-sm text-muted mb-8", "Knowledge Flow" }
                    {rows.into_iter()}
                }
            }
        },
        _ => rsx! { div {} },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconLayers { size: 20 } " Spaces" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        resource.restart();
                        current_resource.restart();
                        flow_resource.restart();
                    },
                    "Refresh"
                }
            }
            {current_highlight}
            if let Some(msg) = status_msg() {
                if msg.starts_with('✓') {
                    div { class: "message-success", "{msg}" }
                } else {
                    div { class: "message-error", "{msg}" }
                }
            }
            div { class: "panel-body",
                {content}
                {flow_content}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
struct SpacesListResponse {
    items: Vec<api::SpaceInfo>,
    #[allow(dead_code)]
    total: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct KnowledgeFlowResponse {
    items: Vec<api::KnowledgeFlowInfo>,
    #[allow(dead_code)]
    total: usize,
}
