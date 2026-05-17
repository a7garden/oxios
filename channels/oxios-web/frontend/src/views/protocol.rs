//! Ouroboros 5-phase display with phase arrows and recent seeds list.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[component]
pub fn ProtocolView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::SeedSummary>("/api/seeds").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(seeds)) if seeds.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconSeeds { size: 40 } }
                p { "No seeds yet. Seeds are created through the Ouroboros interview process." }
            }
        },
        Some(Ok(seeds)) => {
            let cards: Vec<Element> = seeds.iter().take(5).map(|seed| {
                let id = seed.id.clone();
                let goal = seed.goal.clone();
                let count = seed.constraints_count;
                let date = seed.created_at.clone();
                let short = if id.len() >= 8 { &id[..8] } else { &id };
                rsx! {
                    div { class: "item-card", key: "{id}",
                        div { class: "item-title", "{goal}" }
                        div { class: "item-subtitle", "{short} · {count} constraints · {date}" }
                    }
                }
            }).collect();
            rsx! {
                div { class: "ouroboros-panel",
                    div { class: "phase-indicator",
                        div { class: "phase-current", "Ouroboros Lifecycle" }
                        div { class: "phase-progress-bar",
                            div { class: "phase-progress-fill", style: "width:60%" }
                        }
                        div { class: "phase-steps",
                            div { class: "phase-step completed",
                                div { class: "phase-step-icon", IconChat { size: 16 } }
                                div { class: "phase-step-label", "Interview" }
                            }
                            div { class: "phase-step completed",
                                div { class: "phase-step-icon", IconFile { size: 16 } }
                                div { class: "phase-step-label", "Seed" }
                            }
                            div { class: "phase-step active",
                                div { class: "phase-step-icon", IconPlay { size: 16 } }
                                div { class: "phase-step-label", "Execute" }
                            }
                            div { class: "phase-step",
                                div { class: "phase-step-icon", IconCheckSquare { size: 16 } }
                                div { class: "phase-step-label", "Evaluate" }
                            }
                            div { class: "phase-step",
                                div { class: "phase-step-icon", IconRefresh { size: 16 } }
                                div { class: "phase-step-label", "Evolve" }
                            }
                        }
                    }
                    h3 { style: "font-family:var(--font-mono);font-size:13px;color:var(--accent);margin-top:16px",
                        "Recent Seeds"
                    }
                    div { class: "item-list",
                        {cards.into_iter()}
                    }
                }
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading protocol status..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconProtocol { size: 20 } " Protocol" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
