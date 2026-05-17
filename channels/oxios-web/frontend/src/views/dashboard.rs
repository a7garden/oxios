//! Dashboard — stat cards and agent table.

use dioxus::prelude::*;

use crate::api::{self, AgentSummary, PaginatedResponse, StatusResponse};
use crate::components::icons::*;

/// Small stat card component.
#[component]
fn StatCard(icon: Element, label: String, value: String, color: Option<String>) -> Element {
    let color_class = match color.as_deref() {
        Some("green")  => "stat-value green",
        Some("blue")   => "stat-value blue",
        Some("orange") => "stat-value orange",
        Some("red")    => "stat-value red",
        Some("purple") => "stat-value purple",
        _              => "stat-value",
    };

    rsx! {
        div { class: "stat-card",
            div { class: "stat-icon", {icon} }
            div { class: "stat-label", "{label}" }
            div { class: "{color_class}", "{value}" }
        }
    }
}

#[component]
pub fn DashboardView() -> Element {
    let status = use_resource(|| async move {
        api::fetch_json::<StatusResponse>("/api/status").await
    });

    let agents = use_resource(|| async move {
        api::fetch_json::<PaginatedResponse<AgentSummary>>("/api/agents")
            .await
            .map(|r| r.items)
    });

    let status_data = (status.value())();
    let agents_data = (agents.value())();

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconDashboard { size: 20 } " Dashboard" }
            }

            div { class: "stats-grid",
                match &status_data {
                    Some(Ok(s)) => {
                        let active_agents = s.components.as_ref()
                            .map(|c| c.agents.active_count.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let total_forked = s.components.as_ref()
                            .map(|c| c.agents.total_forked.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let memory_entries = s.components.as_ref()
                            .map(|c| c.memory.total_entries.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        rsx! {
                            StatCard { icon: rsx! { IconClock { size: 20 } }, label: "Uptime".to_string(), value: s.uptime.clone(), color: Some("blue".to_string()) }
                            StatCard { icon: rsx! { IconAgents { size: 20 } }, label: "Active Agents".to_string(), value: active_agents, color: Some("green".to_string()) }
                            StatCard { icon: rsx! { IconZap { size: 20 } }, label: "Total Forked".to_string(), value: total_forked, color: Some("orange".to_string()) }
                            StatCard { icon: rsx! { IconMemory { size: 20 } }, label: "Memory Entries".to_string(), value: memory_entries, color: Some("purple".to_string()) }
                            StatCard { icon: rsx! { IconActivity { size: 20 } }, label: "Version".to_string(), value: s.version.clone(), color: None }
                        }
                    },
                    Some(Err(e)) => rsx! { div { class: "error-box", "Status error: {e}" } },
                    None => rsx! { div { class: "text-muted", "Loading status..." } },
                }
            }

            h2 { "Agents" }
            match &agents_data {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div { class: "empty-state",
                        div { class: "empty-icon", IconAgents { size: 40 } }
                        p { "No agents running" }
                    }
                },
                Some(Ok(list)) => rsx! {
                    table { class: "data-table",
                        thead { tr {
                            th { "ID" }
                            th { "Name" }
                            th { "Status" }
                        }}
                        tbody {
                            for a in list {
                                tr {
                                    td { class: "text-mono", "{a.id}" }
                                    td { "{a.name}" }
                                    td { "{a.status}" }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { div { class: "error-box", "Agents error: {e}" } },
                None => rsx! { div { class: "text-muted", "Loading agents..." } },
            }
        }
    }
}
