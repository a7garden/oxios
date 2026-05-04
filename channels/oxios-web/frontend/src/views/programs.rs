//! Program list with enable/disable buttons.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn ProgramsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::ProgramSummary>>("/api/programs").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(programs)) if programs.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "icon", "📦" }
                p { "No programs installed. Install a program to extend agent capabilities." }
            }
        },
        Some(Ok(programs)) => {
            let cards: Vec<Element> = programs.iter().map(|prog| {
                let name = prog.name.clone();
                let enabled = prog.enabled;
                let enabled_class = if enabled { "program-enabled yes" } else { "program-enabled no" };
                let enabled_text = if enabled { "Enabled" } else { "Disabled" };
                let action_class = if enabled { "btn btn-danger btn-sm" } else { "btn btn-sm" };
                let action_label = if enabled { "Disable" } else { "Enable" };
                let action_name = name.clone();
                rsx! {
                    div { class: "program-card", key: "{name}",
                        div { style: "display:flex;align-items:center;justify-content:space-between;",
                            div {
                                span { class: "program-name", "{name}" }
                                span { class: "program-version", "v{prog.version}" }
                                span { class: "{enabled_class}", "{enabled_text}" }
                            }
                            {
                                let an = action_name.clone();
                                rsx! {
                                    button {
                                        class: "{action_class}",
                                        onclick: move |_| {
                                            let n = an.clone();
                                            let is_enabled = enabled;
                                            spawn(async move {
                                                if is_enabled {
                                                    let _ = api::post_action(&format!("/api/programs/{n}/disable")).await;
                                                } else {
                                                    let _ = api::post_action(&format!("/api/programs/{n}/enable")).await;
                                                }
                                                resource.restart();
                                            });
                                        },
                                        "{action_label}"
                                    }
                                }
                            }
                        }
                        div { class: "program-desc", "{prog.description}" }
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
                p { "Loading programs..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "📦 Programs" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
