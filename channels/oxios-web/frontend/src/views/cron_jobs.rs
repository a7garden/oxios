//! Cron job list with create modal, delete, and manual trigger.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Wrapper for the backend list response.
#[derive(Debug, Clone, serde::Deserialize)]
struct CronJobsListResponse {
    jobs: Vec<api::CronJobSummary>,
}

/// Create job form state.
#[derive(Default)]
struct CreateForm {
    name: String,
    schedule: String,
    goal: String,
    constraints: String,
    acceptance_criteria: String,
}

#[component]
pub fn CronJobsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<CronJobsListResponse>("/api/cron-jobs").await
    });

    let mut show_modal = use_signal(|| false);
    let mut form = use_signal(|| CreateForm::default());

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
                                    let jid = id.clone();
                                    spawn(async move {
                                        let _ = api::post_action(&format!("/api/cron-jobs/{jid}/trigger")).await;
                                        resource.restart();
                                    });
                                },
                                IconPlay { size: 14 } " Trigger"
                            }
                            button {
                                class: "btn btn-danger btn-sm",
                                title: "Delete this job",
                                onclick: move |_| {
                                    let jid = id.clone();
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
        let name = form().name.clone();
        let schedule = form().schedule.clone();
        let goal = form().goal.clone();
        let constraints = form().constraints.clone();
        let ac = form().acceptance_criteria.clone();

        rsx! {
            div { class: "modal-overlay",
                div { class: "modal",
                    div { class: "modal-header",
                        h3 { "Create Cron Job" }
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
                                value: "{name}",
                                oninput: move |e| {
                                    let mut f = form();
                                    f.name = e.value();
                                    form.set(f);
                                }
                            }
                        }
                        div { class: "form-group",
                            label { "Schedule (cron expression)" }
                            input {
                                class: "form-input",
                                placeholder: "0 * * * *",
                                value: "{schedule}",
                                oninput: move |e| {
                                    let mut f = form();
                                    f.schedule = e.value();
                                    form.set(f);
                                }
                            }
                        }
                        div { class: "form-group",
                            label { "Goal" }
                            textarea {
                                class: "form-input",
                                placeholder: "Describe what this job should accomplish...",
                                rows: "3",
                                value: "{goal}",
                                oninput: move |e| {
                                    let mut f = form();
                                    f.goal = e.value();
                                    form.set(f);
                                }
                            }
                        }
                        div { class: "form-group",
                            label { "Constraints (comma-separated)" }
                            input {
                                class: "form-input",
                                placeholder: "max_time=30, allow_network=true",
                                value: "{constraints}",
                                oninput: move |e| {
                                    let mut f = form();
                                    f.constraints = e.value();
                                    form.set(f);
                                }
                            }
                        }
                        div { class: "form-group",
                            label { "Acceptance Criteria (comma-separated)" }
                            input {
                                class: "form-input",
                                placeholder: "exit_code=0, output_contains=success",
                                value: "{ac}",
                                oninput: move |e| {
                                    let mut f = form();
                                    f.acceptance_criteria = e.value();
                                    form.set(f);
                                }
                            }
                        }
                    }
                    div { class: "modal-footer",
                        button {
                            class: "btn",
                            onclick: move |_| show_modal.set(false),
                            "Cancel"
                        }
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let f = form();
                                let constraints_vec: Vec<String> = f
                                    .constraints
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                let ac_vec: Vec<String> = f
                                    .acceptance_criteria
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                spawn(async move {
                                    let req = api::CreateCronJobRequest {
                                        name: f.name.clone(),
                                        schedule: f.schedule.clone(),
                                        goal: f.goal.clone(),
                                        constraints: constraints_vec,
                                        acceptance_criteria: ac_vec,
                                        toolchain: "default".to_string(),
                                    };
                                    let _ = api::post_json::<serde_json::Value, _>("/api/cron-jobs", &req).await;
                                    show_modal.set(false);
                                    let mut blank = CreateForm::default();
                                    form.set(blank);
                                    resource.restart();
                                });
                            },
                            "Create"
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
                button { class: "btn btn-sm", onclick: move |_| show_modal.set(true), IconClock { size: 14 } " New Job" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
            {modal}
        }
    }
}