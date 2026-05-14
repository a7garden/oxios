//! Agent list table with kill button and refresh.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[component]
pub fn AgentsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::AgentSummary>>("/api/agents").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(agents)) if agents.is_empty() => rsx! {
            div { class: "empty-state",
                IconUsers { class: "icon".to_string(), size: 48 }
                p { "No agents running. Start a conversation to spawn agents." }
            }
        },
        Some(Ok(agents)) => {
            let count = agents.len();
            let rows: Vec<Element> = agents.iter().map(|agent| {
                let id = agent.id.clone();
                let name = agent.name.clone();
                let status = agent.status.clone();
                let created = agent.created_at.clone();
                let short_id = if id.len() >= 8 { id[..8].to_string() } else { id.clone() };

                let status_class = match status.to_lowercase().as_str() {
                    "running" => "agent-status status-running",
                    "idle" => "agent-status status-idle",
                    "starting" => "agent-status status-starting",
                    "failed" => "agent-status status-failed",
                    _ => "agent-status status-stopped",
                };

                let kill_id = id.clone();
                rsx! {
                    div { class: "agent-card", key: "{id}",
                        div { class: "agent-info",
                            div { class: "agent-name", "{name}" }
                            div { class: "agent-id", "{short_id} · {created}" }
                        }
                        span { class: "{status_class}", "{status}" }
                        button {
                            class: "btn btn-danger btn-sm",
                            onclick: move |_| {
                                let kid = kill_id.clone();
                                spawn(async move {
                                    let _ = api::post_action(&format!("/api/agents/{kid}/kill")).await;
                                    resource.restart();
                                });
                            },
                            "Kill"
                        }
                    }
                }
            }).collect();
            rsx! {
                div { class: "agent-monitor-grid",
                    div { class: "agent-monitor-card",
                        div { class: "agent-monitor-header",
                            div { class: "agent-monitor-title", "Running Agents" }
                            span { class: "agent-monitor-count", "{count}" }
                        }
                        {rows.into_iter()}
                    }
                }
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                IconLoading { class: "icon".to_string(), size: 48 }
                p { "Loading agents..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                IconZap { class: "panel-icon".to_string(), size: 24 }
                h2 { "Agent Monitor" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(),
                    IconRefresh { class: "btn-icon".to_string(), size: 16 }
                    "Refresh"
                }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}