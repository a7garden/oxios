//! Resource monitoring view — current snapshot, overload status, and history.

use crate::api::{self, OverloadStatus};
use crate::components::icons::*;
use dioxus::prelude::*;

/// Response wrapper for resource history endpoint.
#[derive(Debug, Clone, serde::Deserialize)]
struct ResourceHistoryResponse {
    snapshots: Vec<serde_json::Value>,
    count: usize,
}

/// Resource monitoring view with snapshot, overload status, and history.
#[component]
pub fn ResourcesView() -> Element {
    let mut snapshot_resource = use_resource(|| async move {
        api::fetch_json::<serde_json::Value>("/api/resources").await
    });

    let mut overload_resource = use_resource(|| async move {
        api::fetch_json::<OverloadStatus>("/api/resources/overload").await
    });

    let mut history_resource = use_resource(|| async move {
        api::fetch_json::<ResourceHistoryResponse>("/api/resources/history?last_n=30").await
    });

    let snapshot_data = (snapshot_resource.value())();
    let overload_data = (overload_resource.value())();
    let history_data = (history_resource.value())();

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconCpu { size: 20 } " Resources" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        snapshot_resource.restart();
                        overload_resource.restart();
                        history_resource.restart();
                    },
                    "Refresh"
                }
            }

            // Current snapshot
            div { class: "panel-body",
                h3 { "Current Snapshot" }
                match &snapshot_data {
                    Some(Ok(snapshot)) => {
                        // Extract CPU, memory, load values
                        let cpu = snapshot
                            .get("cpu_percent")
                            .or_else(|| snapshot.get("cpu_usage"))
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        let memory = snapshot
                            .get("memory_percent")
                            .or_else(|| snapshot.get("memory_usage"))
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        let load = snapshot
                            .get("load_avg")
                            .or_else(|| snapshot.get("load_average"))
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);

                        let cpu_color = if cpu > 90.0 { "var(--danger)" } else if cpu > 70.0 { "var(--warning)" } else { "var(--accent)" };
                        let mem_color = if memory > 90.0 { "var(--danger)" } else if memory > 70.0 { "var(--warning)" } else { "var(--accent)" };
                        let load_color = if load > 8.0 { "var(--danger)" } else if load > 4.0 { "var(--warning)" } else { "var(--accent)" };

                        rsx! {
                            div { class: "stats-grid",
                                div { class: "stat-card",
                                    div { class: "stat-label", "CPU" }
                                    div { class: "progress-bar-container",
                                        div {
                                            class: "progress-bar-fill",
                                            style: "width: {cpu.min(100.0)}%; background: {cpu_color}",
                                        }
                                    }
                                    div { class: "stat-value", "{cpu:.1}%" }
                                }
                                div { class: "stat-card",
                                    div { class: "stat-label", "Memory" }
                                    div { class: "progress-bar-container",
                                        div {
                                            class: "progress-bar-fill",
                                            style: "width: {memory.min(100.0)}%; background: {mem_color}",
                                        }
                                    }
                                    div { class: "stat-value", "{memory:.1}%" }
                                }
                                div { class: "stat-card",
                                    div { class: "stat-label", "Load Avg" }
                                    div { class: "progress-bar-container",
                                        div {
                                            class: "progress-bar-fill",
                                            style: "width: {(load * 12.5).min(100.0)}%; background: {load_color}",
                                        }
                                    }
                                    div { class: "stat-value", "{load:.2}" }
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
                            p { "Loading snapshot..." }
                        }
                    },
                }
            }

            // Overload status
            div { class: "panel-body",
                h3 { "Overload Status" }
                match &overload_data {
                    Some(Ok(status)) => {
                        let badge_class = if status.overloaded {
                            "status-badge status-badge-inactive"
                        } else {
                            "status-badge status-badge-active"
                        };
                        let badge_text = if status.overloaded { "OVERLOADED" } else { "Normal" };

                        rsx! {
                            div { class: "agent-card",
                                div { class: "agent-info",
                                    div { class: "agent-name",
                                        if status.overloaded {
                                            rsx! { IconAlertTriangle { size: 16 } }
                                        }
                                        "System Load"
                                    }
                                    div { class: "agent-id",
                                        "CPU threshold: {status.threshold.cpu_percent}% · Memory threshold: {status.threshold.memory_percent}% · Load threshold: {status.threshold.load_avg}"
                                    }
                                }
                                span { class: "{badge_class}", "{badge_text}" }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "empty-state", p { { format!("Error: {e}") } } }
                    },
                    None => rsx! {
                        div { class: "text-muted", "Loading overload status..." }
                    },
                }
            }

            // History
            div { class: "panel-body",
                h3 { "History (last 30 snapshots)" }
                match &history_data {
                    Some(Ok(history)) if history.snapshots.is_empty() => rsx! {
                        div { class: "empty-state",
                            p { "No history available." }
                        }
                    },
                    Some(Ok(history)) => {
                        let rows: Vec<Element> = history.snapshots.iter().enumerate().map(|(i, snap)| {
                            let formatted = serde_json::to_string_pretty(snap)
                                .unwrap_or_else(|_| snap.to_string());
                            let idx = i + 1;
                            rsx! {
                                div { class: "agent-card", key: "{idx}",
                                    div { class: "agent-info",
                                        div { class: "agent-name", "Snapshot #{idx}" }
                                        pre { class: "code-block",
                                            code { "{formatted}" }
                                        }
                                    }
                                }
                            }
                        }).collect();
                        rsx! {
                            div {
                                div { class: "agent-monitor-header",
                                    div { class: "agent-monitor-title", "Snapshots" }
                                    span { class: "agent-monitor-count", "{history.count}" }
                                }
                                {rows.into_iter()}
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "empty-state", p { { format!("Error: {e}") } } }
                    },
                    None => rsx! {
                        div { class: "empty-state",
                            div { class: "empty-icon", IconLoading { size: 40 } }
                            p { "Loading history..." }
                        }
                    },
                }
            }
        }
    }
}
