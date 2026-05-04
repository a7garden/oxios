//! Split layout: list on left, content on right.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn MemoryView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::MemoryListItem>>("/api/memory").await
    });

    let mut selected_name = use_signal(|| None::<String>);
    let mut entry_content = use_signal(String::new);

    let list_content: Element = match &(resource.value())() {
        Some(Ok(entries)) if entries.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "icon", "🧠" }
                p { "No memory entries yet. Agent conversations generate daily memory summaries." }
            }
        },
        Some(Ok(entries)) => {
            let items: Vec<Element> = entries.iter().map(|entry| {
                let name = entry.name.clone();
                let category = entry.category.clone();
                let click_name = name.clone();
                rsx! {
                    div {
                        class: "item-card",
                        key: "{name}",
                        onclick: move |_| {
                            let n = click_name.clone();
                            selected_name.set(Some(n.clone()));
                            spawn(async move {
                                match api::fetch_json::<api::MemoryDetail>(&format!("/api/memory/{n}")).await {
                                    Ok(m) => entry_content.set(m.content),
                                    Err(e) => entry_content.set(format!("Error: {e}")),
                                }
                            });
                        },
                        div { class: "item-title", "{name}" }
                        div { class: "item-subtitle", "{category}" }
                    }
                }
            }).collect();
            rsx! { div { class: "item-list", {items.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "icon", "⏳" }
                p { "Loading memory..." }
            }
        },
    };

    let detail: Element = {
        let sel = selected_name();
        match sel {
            Some(_) => {
                let text = entry_content().clone();
                rsx! {
                    div { class: "detail-view",
                        h3 { "Memory Entry" }
                        pre { "{text}" }
                    }
                }
            }
            None => rsx! {
                div { class: "empty-state",
                    div { class: "icon", "📄" }
                    p { "Select an entry from the list to view its content." }
                }
            },
        }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "🧠 Memory" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "workspace-split",
                div { class: "workspace-tree",
                    {list_content}
                }
                div { class: "workspace-viewer",
                    {detail}
                }
            }
        }
    }
}
