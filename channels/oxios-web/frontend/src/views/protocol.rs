//! Ouroboros 5-phase display with dynamic phase tracking and recent seeds list.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Mapping of phase strings to step indices (0-indexed).
fn phase_to_index(phase: &str) -> usize {
    match phase {
        "Interview" => 0,
        "Seed" => 1,
        "Execute" => 2,
        "Evaluate" => 3,
        "Evolve" => 4,
        _ => 2, // Default to Execute if unknown
    }
}

/// Fetch the current phase from the most recent seed.
async fn fetch_current_phase() -> Option<(usize, String)> {
    let seeds: Vec<api::SeedSummary> = match api::fetch_paginated("/api/seeds").await {
        Ok(s) => s,
        Err(_) => return None,
    };
    let most_recent = seeds.first()?;
    let details: serde_json::Value = match api::fetch_json(&format!("/api/seeds/{}", most_recent.id)).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let phase = details
        .get("phase")
        .and_then(|v| v.as_str())
        .unwrap_or("Execute")
        .to_string();
    let idx = phase_to_index(&phase);
    Some((idx, phase))
}

#[derive(Debug, Clone)]
struct ProtocolData {
    current_index: usize,
    current_phase: String,
    seeds: Vec<api::SeedSummary>,
}

#[component]
pub fn ProtocolView() -> Element {
    let mut resource = use_resource(|| async move {
        let phase_data = fetch_current_phase().await;
        let seeds: Vec<api::SeedSummary> = api::fetch_paginated("/api/seeds").await.unwrap_or_default();
        
        let (current_index, current_phase) = phase_data.unwrap_or((2, "Execute".to_string()));
        
        ProtocolData {
            current_index,
            current_phase,
            seeds,
        }
    });

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconProtocol { size: 20 } " Protocol" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                match &(resource.value())() {
                    Some(Ok(data)) if data.seeds.is_empty() => rsx! {
                        div { class: "empty-state",
                            div { class: "empty-icon", IconSeeds { size: 40 } }
                            p { "No seeds yet. Seeds are created through the Ouroboros interview process." }
                        }
                    },
                    Some(Ok(data)) => {
                        let active_step = data.current_index;
                        let progress_pct = if data.current_index == 0 {
                            0.0
                        } else {
                            (data.current_index as f64 / 5.0) * 100.0
                        };

                        rsx! {
                            div { class: "ouroboros-panel",
                                div { class: "phase-indicator",
                                    div { class: "phase-current", "Ouroboros Lifecycle" }
                                    div { class: "phase-phase-label", style: "font-size:11px;color:var(--text-3);margin-bottom:8px",
                                        "Current: {data.current_phase}"
                                    }
                                    div { class: "phase-progress-bar",
                                        div {
                                            class: "phase-progress-fill",
                                            style: "width:{progress_pct}%"
                                        }
                                    }
                                    div { class: "phase-steps",
                                        if active_step == 0 {
                                            rsx! {
                                                div { class: "phase-step active",
                                                    div { class: "phase-step-icon", IconChat { size: 16 } }
                                                    div { class: "phase-step-label", "Interview" }
                                                }
                                                div { class: "phase-step",
                                                    div { class: "phase-step-icon", IconFile { size: 16 } }
                                                    div { class: "phase-step-label", "Seed" }
                                                }
                                                div { class: "phase-step",
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
                                        } else if active_step == 1 {
                                            rsx! {
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconChat { size: 16 } }
                                                    div { class: "phase-step-label", "Interview" }
                                                }
                                                div { class: "phase-step active",
                                                    div { class: "phase-step-icon", IconFile { size: 16 } }
                                                    div { class: "phase-step-label", "Seed" }
                                                }
                                                div { class: "phase-step",
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
                                        } else if active_step == 2 {
                                            rsx! {
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
                                        } else if active_step == 3 {
                                            rsx! {
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconChat { size: 16 } }
                                                    div { class: "phase-step-label", "Interview" }
                                                }
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconFile { size: 16 } }
                                                    div { class: "phase-step-label", "Seed" }
                                                }
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconPlay { size: 16 } }
                                                    div { class: "phase-step-label", "Execute" }
                                                }
                                                div { class: "phase-step active",
                                                    div { class: "phase-step-icon", IconCheckSquare { size: 16 } }
                                                    div { class: "phase-step-label", "Evaluate" }
                                                }
                                                div { class: "phase-step",
                                                    div { class: "phase-step-icon", IconRefresh { size: 16 } }
                                                    div { class: "phase-step-label", "Evolve" }
                                                }
                                            }
                                        } else {
                                            rsx! {
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconChat { size: 16 } }
                                                    div { class: "phase-step-label", "Interview" }
                                                }
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconFile { size: 16 } }
                                                    div { class: "phase-step-label", "Seed" }
                                                }
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconPlay { size: 16 } }
                                                    div { class: "phase-step-label", "Execute" }
                                                }
                                                div { class: "phase-step completed",
                                                    div { class: "phase-step-icon", IconCheckSquare { size: 16 } }
                                                    div { class: "phase-step-label", "Evaluate" }
                                                }
                                                div { class: "phase-step active",
                                                    div { class: "phase-step-icon", IconRefresh { size: 16 } }
                                                    div { class: "phase-step-label", "Evolve" }
                                                }
                                            }
                                        }
                                    }
                                }
                                h3 { style: "font-family:var(--font-mono);font-size:13px;color:var(--accent);margin-top:16px",
                                    "Recent Seeds"
                                }
                                div { class: "item-list",
                                    {data.seeds.iter().take(5).map(|seed| {
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
                                    }).collect::<Vec<_>>()}
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
                }
            }
        }
    }
}