//! Seed cards showing goal, constraints, criteria, creation date.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[component]
pub fn SeedsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::SeedSummary>>("/api/seeds").await
    });

    let mut selected_id = use_signal(|| None::<String>);
    let mut detail = use_signal(|| String::new());

    let content: Element = match &(resource.value())() {
        Some(Ok(seeds)) if seeds.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconSeeds { size: 40 } }
                p { "No seeds yet. Seeds are created through the Ouroboros interview process." }
            }
        },
        Some(Ok(seeds)) => {
            let cards: Vec<Element> = seeds.iter().map(|seed| {
                let id = seed.id.clone();
                let goal = seed.goal.clone();
                let count = seed.constraints_count;
                let date = seed.created_at.clone();
                let click_id = id.clone();
                rsx! {
                    div {
                        class: "item-card",
                        key: "{id}",
                        onclick: move |_| {
                            let cid = click_id.clone();
                            selected_id.set(Some(cid.clone()));
                            spawn(async move {
                                match api::fetch_json::<serde_json::Value>(&format!("/api/seeds/{cid}")).await {
                                    Ok(val) => {
                                        let pretty = serde_json::to_string_pretty(&val).unwrap_or_default();
                                        detail.set(pretty);
                                    }
                                    Err(e) => detail.set(format!("Error: {e}")),
                                }
                            });
                        },
                        div { class: "item-title", "{goal}" }
                        div { class: "item-subtitle", "Constraints: {count} · {date}" }
                    }
                }
            }).collect();
            rsx! {
                div { class: "item-list",
                    {cards.into_iter()}
                }
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading seeds..." }
            }
        },
    };

    let detail_view: Element = {
        let det = detail().clone();
        if det.is_empty() {
            rsx! { div {} }
        } else {
            rsx! {
                div { class: "detail-view",
                    h3 { "Seed Details" }
                    pre { "{det}" }
                }
            }
        }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconSeeds { size: 20 } " Seeds" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
                {detail_view}
            }
        }
    }
}
