//! Cron job list with create modal, delete, and manual trigger.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Wrapper for the backend list response.
#[derive(Debug, Clone, serde::Deserialize)]
struct CronJobsListResponse {
    jobs: Vec<api::CronJobSummary>,
}

/// Modal form state for creating a new cron job.
#[component]
pub fn CronJobsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<CronJobsListResponse>("/api/cron-jobs").await
    });

    let mut show_modal = use_signal(|| false);
    let mut editing_id = use_signal(|| None::<String>);
    let mut form_name = use_signal(String::new);
    let mut form_schedule = use_signal(String::new);
    let mut form_goal = use_signal(String::new);
    let mut form_constraints = use_signal(String::new);
    let mut form_ac = use_signal(String::new);

    let content: Element = match &(resource.value())() {
        Some(Ok(resp)) if resp.jobs.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconClock { size: 40 } }
                p { "No cron jobs configured." }
            }
        },
        Some(Ok(resp)) => {
            let cards: Vec<Element> = resp.jobs.iter().map(|job| {
                let id = job.id.clone();
                let enabled_text = if job.enabled { "Enabled" } else { "Disabled" };
                let enabled_class = if job.enabled {
                    "status-badge status-badge-active"
                } else {
                    "status-badge status-badge-inactive"
                };
                let last_run = job.last_run.clone().unwrap_or_else(|| "never".to_string());
                let next_run = job.next_run.clone().unwrap_or_else(|| "—".to_string());

                let id_for_trigger = id.clone();
                let id_for_delete = id.clone();
                let job_for_edit = job.clone();

                rsx! {
                    div { class: "agent-card", key: "{id}",
                        div { class: "agent-info",
                            div { class: "agent-name", "{job.name}" }
                            div { class: "agent-id", "Schedule: {job.schedule}" }
                            div { class: "agent-id", "Goal: {job.goal}" }
                            div { class: "agent-id", "Last: {last_run} · Next: {next_run}" }
                            span { class: "{enabled_class}", "{enabled_text}" }
                        }
                        div { class: "card-actions",
                            button {
                                class: "btn btn-sm",
                                title: "Manually trigger this job",
                                onclick: move |_| {
                                    let jid = id_for_trigger.clone();
                                    spawn(async move {
                                        let _ = api::post_action(&format!("/api/cron-jobs/{jid}/trigger")).await;
                                        resource.restart();
                                    });
                                },
                                IconPlay { size: 14 } " Trigger"
                            }
                            button {
                                class: "btn btn-sm",
                                style: "margin-left:4px",
                                title: "Edit this job",
                                onclick: move |_| {
                                    editing_id.set(Some(job_for_edit.id.clone()));
                                    form_name.set(job_for_edit.name.clone());
                                    form_schedule.set(job_for_edit.schedule.clone());
                                    form_goal.set(job_for_edit.goal.clone());
                                    form_constraints.set(job_for_edit.constraints.join(", "));
                                    form_ac.set(String::new());
                                    show_modal.set(true);
                                },
                                IconSettings { size: 14 }
                            }
                            button {
                                class: "btn btn-danger btn-sm",
                                title: "Delete this job",
                                onclick: move |_| {
                                    let jid = id_for_delete.clone();
                                    spawn(async move {
                                        let _ = api::delete_action(&format!("/api/cron-jobs/{jid}")).await;
                                        resource.restart();
                                    });
                                },
                                IconTrash { size: 14 }
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
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading cron jobs..." }
            }
        },
    };

    let modal: Element = if show_modal() {
        rsx! {
            div { class: "modal-overlay", onclick: move |_| show_modal.set(false),
                div { class: "modal", onclick: move |e| e.stop_propagation(),
                    div { class: "modal-header",
                        h3 { if editing_id().is_some() { "Edit Cron Job" } else { "Create Cron Job" } }
                        button {
                            class: "icon-btn",
                            onclick: move |_| show_modal.set(false),
                            IconX { size: 18 }
                        }
                    }
                    div { class: "modal-body",
                        div { class: "form-group",
                            label { "Name" }
                            input {
                                class: "form-input",
                                placeholder: "My Job",
                                value: "{form_name}",
                                oninput: move |e| form_name.set(e.value()),
                            }
                        }
                        div { class: "form-group",
                            label { "Schedule (cron expression)" }
                            input {
                                class: "form-input",
                                placeholder: "0 * * * *",
                                value: "{form_schedule}",
                                oninput: move |e| form_schedule.set(e.value()),
                            }
                        }
                        div { class: "form-group",
                            label { "Goal" }
                            textarea {
                                class: "form-input",
                                placeholder: "Describe what this job should accomplish...",
                                rows: "3",
                                value: "{form_goal}",
                                oninput: move |e| form_goal.set(e.value()),
                            }
                        }
                        div { class: "form-group",
                            label { "Constraints (comma-separated)" }
                            input {
                                class: "form-input",
                                placeholder: "max_time=30, allow_network=true",
                                value: "{form_constraints}",
                                oninput: move |e| form_constraints.set(e.value()),
                            }
                        }
                        div { class: "form-group",
                            label { "Acceptance Criteria (comma-separated)" }
                            input {
                                class: "form-input",
                                placeholder: "exit_code=0, output_contains=success",
                                value: "{form_ac}",
                                oninput: move |e| form_ac.set(e.value()),
                            }
                        }
                    }
                    div { class: "modal-footer",
                        button { class: "btn", onclick: move |_| show_modal.set(false), "Cancel" }
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let name = form_name();
                                let schedule = form_schedule();
                                let goal = form_goal();
                                let constraints: Vec<String> = form_constraints()
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                let ac: Vec<String> = form_ac()
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                spawn(async move {
                                    let req = api::CreateCronJobRequest {
                                        name,
                                        schedule,
                                        goal,
                                        constraints,
                                        acceptance_criteria: ac,
                                        toolchain: "default".to_string(),
                                    };
                                    if let Some(eid) = editing_id() {
                                        let _ = api::post_json::<serde_json::Value, _>(&format!("/api/cron-jobs/{eid}/edit"), &req).await;
                                    } else {
                                        let _ = api::post_json::<serde_json::Value, _>("/api/cron-jobs", &req).await;
                                    }
                                    show_modal.set(false);
                                    editing_id.set(None);
                                    form_name.set(String::new());
                                    form_schedule.set(String::new());
                                    form_goal.set(String::new());
                                    form_constraints.set(String::new());
                                    form_ac.set(String::new());
                                    resource.restart();
                                });
                            },
                            if editing_id().is_some() { "Save" } else { "Create" }
                        }
                    }
                }
            }
        }
    } else {
        rsx! { div {} }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconClock { size: 20 } " Cron Jobs" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| show_modal.set(true),
                    IconClock { size: 14 } " New Job"
                }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
            {modal}
        }
    }
}