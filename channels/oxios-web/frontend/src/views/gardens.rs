//! Garden cards with start/stop/remove actions.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn GardensView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::GardenSummary>>("/api/gardens").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(gardens)) if gardens.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "icon", "🌿" }
                p { "No gardens yet. Create a garden to set up an isolated execution environment." }
            }
        },
        Some(Ok(gardens)) => {
            let cards: Vec<Element> = gardens.iter().map(|garden| {
                let name = garden.name.clone();
                let status_class = if garden.running {
                    "garden-status-badge garden-status-running"
                } else {
                    "garden-status-badge garden-status-stopped"
                };
                let status_text = if garden.running { "Running" } else { "Stopped" };
                let start_name = name.clone();
                let stop_name = name.clone();
                let remove_name = name.clone();
                rsx! {
                    div { class: "garden-card", key: "{name}",
                        div { class: "garden-info",
                            div { class: "garden-name", "{name}" }
                            div { class: "garden-meta", "{garden.image_tag} · {garden.created_at}" }
                        }
                        div { class: "garden-actions",
                            span { class: "{status_class}", "{status_text}" }
                            {
                                let sn = start_name.clone();
                                rsx! {
                                    button {
                                        class: "btn btn-sm",
                                        onclick: move |_| {
                                            let n = sn.clone();
                                            spawn(async move {
                                                let _ = api::post_action(&format!("/api/gardens/{n}/start")).await;
                                                resource.restart();
                                            });
                                        },
                                        "Start"
                                    }
                                }
                            }
                            {
                                let sn = stop_name.clone();
                                rsx! {
                                    button {
                                        class: "btn btn-sm",
                                        onclick: move |_| {
                                            let n = sn.clone();
                                            spawn(async move {
                                                let _ = api::post_action(&format!("/api/gardens/{n}/stop")).await;
                                                resource.restart();
                                            });
                                        },
                                        "Stop"
                                    }
                                }
                            }
                            {
                                let rn = remove_name.clone();
                                rsx! {
                                    button {
                                        class: "btn btn-danger btn-sm",
                                        onclick: move |_| {
                                            let n = rn.clone();
                                            spawn(async move {
                                                let _ = api::delete_action(&format!("/api/gardens/{n}")).await;
                                                resource.restart();
                                            });
                                        },
                                        "Remove"
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
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "icon", "⏳" }
                p { "Loading gardens..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "🌿 Gardens" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
