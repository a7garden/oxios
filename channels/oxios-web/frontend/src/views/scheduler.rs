//! Stats grid + progress bar. Compute percentage OUTSIDE rsx!

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn SchedulerView() -> Element {
    let mut stats_resource = use_resource(|| async move {
        api::fetch_json::<api::SchedulerStatsResponse>("/api/scheduler/stats").await
    });

    let mut tasks_resource = use_resource(|| async move {
        api::fetch_json::<api::SchedulerTasks>("/api/scheduler/tasks").await
    });

    let stats_content: Element = match &(stats_resource.value())() {
        Some(Ok(stats)) => {
            // Compute percentages OUTSIDE rsx
            let capacity_pct = if stats.max_concurrent > 0 {
                (stats.running as f64 / stats.max_concurrent as f64 * 100.0) as u32
            } else {
                0
            };
            let rate_pct = if stats.rate_limit_per_minute > 0 {
                (stats.rate_remaining as f64 / stats.rate_limit_per_minute as f64 * 100.0) as u32
            } else {
                100
            };

            rsx! {
                div { class: "config-grid",
                    div { class: "config-section",
                        h3 { "Capacity" }
                        div { class: "config-row",
                            span { class: "key", "Queued" }
                            span { class: "val", "{stats.queued}" }
                        }
                        div { class: "config-row",
                            span { class: "key", "Running" }
                            span { class: "val", "{stats.running} / {stats.max_concurrent}" }
                        }
                        div { class: "phase-progress-bar",
                            div { class: "phase-progress-fill", style: "width:{capacity_pct}%" }
                        }
                    }
                    div { class: "config-section",
                        h3 { "Rate Limit" }
                        div { class: "config-row",
                            span { class: "key", "Limit" }
                            span { class: "val", "{stats.rate_limit_per_minute}/min" }
                        }
                        div { class: "config-row",
                            span { class: "key", "Remaining" }
                            span { class: "val", "{stats.rate_remaining}" }
                        }
                        div { class: "phase-progress-bar",
                            div { class: "phase-progress-fill", style: "width:{rate_pct}%" }
                        }
                    }
                }
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "icon", "⏳" }
                p { "Loading scheduler stats..." }
            }
        },
    };

    let tasks_content: Element = match &(tasks_resource.value())() {
        Some(Ok(tasks)) => {
            let queued_empty = tasks.queued.is_empty();
            let running_empty = tasks.running.is_empty();
            if queued_empty && running_empty {
                rsx! {
                    div { class: "empty-state",
                        p { "No active tasks." }
                    }
                }
            } else {
                let all_tasks: Vec<Element> = tasks.running.iter().chain(tasks.queued.iter()).map(|task| {
                    let desc = task.description.clone();
                    let status = task.status.clone();
                    let priority = task.priority.clone();
                    let id = task.id.clone();
                    let short_id = if id.len() >= 8 { id[..8].to_string() } else { id };
                    rsx! {
                        div { class: "agent-card", key: "{task.id}",
                            div { class: "agent-info",
                                div { class: "agent-name", "{desc}" }
                                div { class: "agent-id", "{short_id} · {priority} · {status}" }
                            }
                        }
                    }
                }).collect();
                rsx! {
                    div {
                        h3 { style: "font-family:var(--font-mono);font-size:13px;color:var(--accent);margin:12px 0;",
                            "Tasks"
                        }
                        {all_tasks.into_iter()}
                    }
                }
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! { div {} },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "📋 Scheduler" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        stats_resource.restart();
                        tasks_resource.restart();
                    },
                    "Refresh"
                }
            }
            div { class: "panel-body",
                {stats_content}
                {tasks_content}
            }
        }
    }
}
