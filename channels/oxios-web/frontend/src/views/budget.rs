//! Budget management view — lookup, set, reserve, reset, remove agent budgets.

use crate::api::{self, BudgetInfo, SetBudgetRequest};
use crate::components::icons::*;
use dioxus::prelude::*;

/// Budget management view with lookup, gauges, and action buttons.
#[component]
pub fn BudgetView() -> Element {
    // Agent ID input
    let mut agent_id = use_signal(String::new);

    // Fetched budget info
    let mut budget_resource = use_resource(move || {
        let id = agent_id();
        async move {
            if id.is_empty() {
                return None;
            }
            api::fetch_json::<BudgetInfo>(&format!("/api/budget/{id}")).await.ok()
        }
    });

    // Form fields for Set Budget
    let mut token_budget = use_signal(|| 10000u64.to_string());
    let mut calls_budget = use_signal(|| 100u64.to_string());
    let mut window_secs = use_signal(|| 3600u64.to_string());

    // Reserve tokens input
    let mut reserve_tokens = use_signal(String::new);

    // Status message
    let mut status_msg = use_signal(|| None::<String>);

    let budget_data = (budget_resource.value())();

    let budget_content: Element = match &budget_data {
        Some(Some(info)) => {
            let exhausted_class =
                if info.is_exhausted { "status-badge status-badge-inactive" } else { "status-badge status-badge-active" };
            let exhausted_text = if info.is_exhausted { "Exhausted" } else { "Active" };
            let agent_id_for_reset = info.agent_id.clone();
            let agent_id_for_delete = info.agent_id.clone();
            let agent_id_for_reserve = info.agent_id.clone();

            rsx! {
                div { class: "agent-card",
                    div { class: "agent-info",
                        div { class: "agent-name", "Agent: {info.agent_id}" }
                        div { class: "agent-id", "Window remaining: {info.window_remaining_secs}s" }
                    }
                    span { class: "{exhausted_class}", "{exhausted_text}" }
                }

                div { class: "stats-grid",
                    div { class: "stat-card",
                        div { class: "stat-label", "Tokens Remaining" }
                        div { class: "stat-value purple", "{info.tokens_remaining}" }
                        div { class: "text-xs text-muted mt-8", "Window: {info.window_remaining_secs}s left" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-label", "Calls Remaining" }
                        div { class: "stat-value blue", "{info.calls_remaining}" }
                    }
                }

                div { class: "form-row",
                    button {
                        class: "btn btn-sm",
                        onclick: move |_| {
                            let id = agent_id_for_reset.clone();
                            spawn(async move {
                                match api::post_action(&format!("/api/budget/{id}/reset")).await {
                                    Ok(()) => status_msg.set(Some("Budget reset.".to_string())),
                                    Err(e) => status_msg.set(Some(format!("Reset error: {e}"))),
                                }
                                budget_resource.restart();
                            });
                        },
                        "Reset Budget"
                    }
                    button {
                        class: "btn btn-danger btn-sm",
                        onclick: move |_| {
                            let id = agent_id_for_delete.clone();
                            spawn(async move {
                                match api::delete_action(&format!("/api/budget/{id}")).await {
                                    Ok(()) => status_msg.set(Some("Budget removed.".to_string())),
                                    Err(e) => status_msg.set(Some(format!("Delete error: {e}"))),
                                }
                                budget_resource.restart();
                            });
                        },
                        "Remove Budget"
                    }
                }

                div { class: "form-row",
                    input {
                        r#type: "number",
                        class: "input",
                        placeholder: "Tokens to reserve",
                        value: "{reserve_tokens}",
                        oninput: move |e| reserve_tokens.set(e.value()),
                    }
                    button {
                        class: "btn btn-sm",
                        onclick: move |_| {
                            let id = agent_id_for_reserve.clone();
                            let tokens_str = reserve_tokens();
                            let tokens: u64 = tokens_str.parse().unwrap_or(0);
                            if tokens > 0 {
                                spawn(async move {
                                    match api::post_json::<serde_json::Value, _>(
                                        &format!("/api/budget/{id}/reserve"),
                                        &serde_json::json!({ "tokens": tokens }),
                                    ).await {
                                        Ok(_) => status_msg.set(Some("Tokens reserved.".to_string())),
                                        Err(e) => status_msg.set(Some(format!("Reserve error: {e}"))),
                                    }
                                    budget_resource.restart();
                                });
                            }
                        },
                        "Reserve"
                    }
                }
            }
        }
        Some(None) => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconDollarSign { size: 40 } }
                p { "Enter an agent ID to look up budget." }
            }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading budget..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconDollarSign { size: 20 } " Budget" }
            }
            div { class: "panel-body",
                // Lookup section
                div { class: "form-row",
                    input {
                        r#type: "text",
                        class: "input",
                        placeholder: "Enter agent ID",
                        value: "{agent_id}",
                        oninput: move |e| agent_id.set(e.value()),
                    }
                    button {
                        class: "btn btn-sm btn-primary",
                        onclick: move |_| budget_resource.restart(),
                        "Lookup"
                    }
                }

                // Budget info display
                {budget_content}

                // Set Budget form
                h3 { style: "margin-top:16px", "Set Budget" }
                div { class: "form-row",
                    input {
                        r#type: "number",
                        class: "input",
                        placeholder: "Token budget",
                        value: "{token_budget}",
                        oninput: move |e| token_budget.set(e.value()),
                    }
                    input {
                        r#type: "number",
                        class: "input",
                        placeholder: "Calls budget",
                        value: "{calls_budget}",
                        oninput: move |e| calls_budget.set(e.value()),
                    }
                    input {
                        r#type: "number",
                        class: "input",
                        placeholder: "Window (secs)",
                        value: "{window_secs}",
                        oninput: move |e| window_secs.set(e.value()),
                    }
                }
                div { class: "form-row",
                    button {
                        class: "btn btn-sm btn-primary",
                        onclick: move |_| {
                            let id = agent_id();
                            if id.is_empty() {
                                status_msg.set(Some("Enter an agent ID first.".to_string()));
                                return;
                            }
                            let tb: u64 = token_budget().parse().unwrap_or(0);
                            let cb: u64 = calls_budget().parse().unwrap_or(0);
                            let ws: u64 = window_secs().parse().unwrap_or(0);
                            if tb == 0 || cb == 0 || ws == 0 {
                                status_msg.set(Some("All budget fields must be > 0.".to_string()));
                                return;
                            }
                            spawn(async move {
                                let req = SetBudgetRequest {
                                    token_budget: tb,
                                    calls_budget: cb,
                                    window_secs: ws,
                                };
                                match api::post_json::<serde_json::Value, _>(
                                    &format!("/api/budget/{id}"),
                                    &req,
                                ).await {
                                    Ok(_) => status_msg.set(Some("Budget set.".to_string())),
                                    Err(e) => status_msg.set(Some(format!("Set error: {e}"))),
                                }
                                budget_resource.restart();
                            });
                        },
                        "Set Budget"
                    }
                }

                // Status message
                if let Some(msg) = status_msg() {
                    div { class: "status-message", "{msg}" }
                }
            }
        }
    }
}
