//! Agent groups view — list and detail display.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Agent groups view with list and detail panels.
#[component]
pub fn AgentGroupsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<serde_json::Value>>("/api/agent-groups").await.ok()
    });

    // Selected group detail
    let mut selected_id = use_signal(|| None::<String>);
    let detail_resource = use_resource(move || {
        let id = selected_id();
        async move {
            match id {
                Some(id) if !id.is_empty() => {
                    api::fetch_json::<serde_json::Value>(&format!("/api/agent-groups/{id}")).await.ok()
                }
                _ => None,
            }
        }
    });

    let groups_data = (resource.value())();
    let detail_data = (detail_resource.value())();

    let groups_content: Element = match &groups_data {
        Some(Some(groups)) if groups.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLayers { size: 40 } }
                p { "No agent groups found." }
            }
        },
        Some(Some(groups)) => {
            let rows: Vec<Element> = groups.iter().map(|group| {
                let id = group
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let display_id = if id.len() >= 8 { id[..8].to_string() } else { id.clone() };
                let name = group.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed").to_string();
                let id_for_click = id.clone();

                rsx! {
                    div {
                        class: "item-card",
                        key: "{id}",
                        onclick: move |_| selected_id.set(Some(id_for_click.clone())),
                        div { class: "item-title", "{name}" }
                        div { class: "item-subtitle", "{display_id}" }
                    }
                }
            }).collect();

            rsx! { div { class: "item-list", {rows.into_iter()} } }
        },
        Some(None) => rsx! {
            div { class: "empty-state", p { "Failed to load groups." } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading..." }
            }
        },
    };

    let detail_content: Element = match &detail_data {
        Some(Some(val)) => {
            let pretty = serde_json::to_string_pretty(val).unwrap_or_else(|_| "Invalid JSON".to_string());
            rsx! {
                div { class: "code-block",
                    pre { code { "{pretty}" } }
                }
            }
        },
        _ => rsx! {
            p { class: "text-muted", style: "padding:16px", "Select a group to view details." }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconLayers { size: 20 } " Agent Groups" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {groups_content}
                h3 { style: "margin-top:16px", "Details" }
                {detail_content}
            }
        }
    }
}
