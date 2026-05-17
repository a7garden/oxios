//! Agent group view — list groups and view details as formatted JSON.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Agent groups view with list and detail panel.
#[component]
pub fn AgentGroupsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<serde_json::Value>>("/api/agent-groups").await
    });

    // Selected group detail
    let mut selected_id = use_signal(|| None::<String>);
    let mut detail_resource = use_resource(move || {
        let id = selected_id();
        async move {
            match id {
                Some(id) if !id.is_empty() => {
                    Some(
                        api::fetch_json::<serde_json::Value>(&format!("/api/agent-groups/{id}"))
                            .await?,
                    )
                }
                _ => None,
            }
        }
    });

    let groups_data = (resource.value())();
    let detail_data = (detail_resource.value())();

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconLayers { size: 20 } " Agent Groups" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }

            div { class: "panel-body",
                // Groups list
                match &groups_data {
                    Some(Ok(groups)) if groups.is_empty() => rsx! {
                        div { class: "empty-state",
                            div { class: "empty-icon", IconLayers { size: 40 } }
                            p { "No agent groups found." }
                        }
                    },
                    Some(Ok(groups)) => {
                        let rows: Vec<Element> = groups.iter().map(|group| {
                            let id = group
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let name = group
                                .get("name")
                                .or_else(|| group.get("goal"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string();
                            let short_id = if id.len() >= 8 { id[..8].to_string() } else { id.clone() };
                            let sel_id = id.clone();

                            rsx! {
                                div { class: "agent-card",
                                    key: "{id}",
                                    onclick: move |_| {
                                        selected_id.set(sel_id.clone());
                                        detail_resource.restart();
                                    },
                                    div { class: "agent-info",
                                        div { class: "agent-name", "{name}" }
                                        div { class: "agent-id", "{short_id}" }
                                    }
                                    IconChevronRight { size: 16 }
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
                            p { "Loading agent groups..." }
                        }
                    },
                }
            }

            // Detail panel
            match &detail_data {
                Some(Ok(Some(detail))) => {
                    let formatted = serde_json::to_string_pretty(detail).unwrap_or_else(|_| detail.to_string());
                    rsx! {
                        div { class: "panel-body",
                            h3 { "Group Details" }
                            pre { class: "code-block",
                                code { "{formatted}" }
                            }
                        }
                    }
                },
                Some(Ok(None)) => rsx! {
                    div { class: "panel-body",
                        div { class: "empty-state",
                            p { "Select a group to view details." }
                        }
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "panel-body",
                        div { class: "empty-state", p { { format!("Detail error: {e}") } } }
                    }
                },
                None => rsx! {
                    div { class: "panel-body",
                        div { class: "text-muted", "Loading detail..." }
                    }
                },
            }
        }
    }
}
