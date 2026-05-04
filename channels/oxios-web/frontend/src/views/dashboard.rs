use dioxus::prelude::*;

use crate::api::{self, AgentInfo, SchedulerStats, StatusResponse};

/// Small stat card component.
#[component]
fn StatCard(icon: String, label: String, value: String, color: Option<String>) -> Element {
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
            div { class: "stat-icon", "{icon}" }
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
        api::fetch_json::<Vec<AgentInfo>>("/api/agents").await
    });

    let _scheduler = use_resource(|| async move {
        api::fetch_json::<SchedulerStats>("/api/scheduler/stats").await
    });

    let status_data = (status.value())();
    let agents_data = (agents.value())();

    rsx! {
        div { class: "panel-container",
            h1 { "📊 Dashboard" }

            div { class: "stats-grid",
                match &status_data {
                    Some(Ok(s)) => rsx! {
                        StatCard { icon: "⏱".to_string(), label: "Uptime".to_string(), value: format!("{}s", s.uptime_secs), color: Some("blue".to_string()) }
                        StatCard { icon: "🤖".to_string(), label: "Active Agents".to_string(), value: s.active_agents.to_string(), color: Some("green".to_string()) }
                        StatCard { icon: "🌿".to_string(), label: "Gardens".to_string(), value: s.total_gardens.to_string(), color: Some("purple".to_string()) }
                        StatCard { icon: "🌱".to_string(), label: "Seeds".to_string(), value: s.total_seeds.to_string(), color: Some("orange".to_string()) }
                        StatCard { icon: "📡".to_string(), label: "Version".to_string(), value: s.version.clone(), color: None }
                    },
                    Some(Err(e)) => rsx! { div { class: "error-box", "Status error: {e}" } },
                    None => rsx! { div { class: "text-muted", "Loading status…" } },
                }
            }

            h2 { "Agents" }
            match &agents_data {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div { class: "empty-state",
                        div { class: "empty-icon", "🤖" }
                        p { "No agents running" }
                    }
                },
                Some(Ok(list)) => rsx! {
                    table { class: "data-table",
                        thead { tr {
                            th { "ID" }
                            th { "Name" }
                            th { "Status" }
                            th { "Garden" }
                        }}
                        tbody {
                            for a in list {
                                tr {
                                    td { class: "text-mono", "{a.id}" }
                                    td { "{a.name}" }
                                    td { "{a.status}" }
                                    td { "{a.garden.as_deref().unwrap_or(\"-\")}" }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { div { class: "error-box", "Agents error: {e}" } },
                None => rsx! { div { class: "text-muted", "Loading agents…" } },
            }
        }
    }
}
