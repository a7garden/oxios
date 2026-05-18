//! Approval cards with approve/reject buttons (only for pending).

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[component]
pub fn ApprovalsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::ApprovalResponse>>("/api/approvals").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(approvals)) if approvals.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconCheckSquare { size: 40 } }
                p { "No approval requests." }
            }
        },
        Some(Ok(approvals)) => {
            let cards: Vec<Element> = approvals.iter().map(|approval| {
                let id = approval.id.clone();
                let status = approval.status.clone();
                let is_pending = status == "pending";
                let action = approval.action.clone();
                let resource_name = approval.resource.clone();
                let subject = approval.subject.clone();
                let reason = approval.reason.clone();
                let timestamp = approval.created_at.clone();

                let status_class = match status.as_str() {
                    "pending" => "status-badge status-badge-active",
                    "approved" => "status-badge status-badge-active",
                    "rejected" => "status-badge status-badge-inactive",
                    _ => "status-badge status-badge-inactive",
                };

                let approve_id = id.clone();
                let reject_id = id.clone();

                let buttons: Element = if is_pending {
                    rsx! {
                        {
                            let aid = approve_id.clone();
                            rsx! {
                                button {
                                    class: "btn btn-success btn-sm",
                                    onclick: move |_| {
                                        let a = aid.clone();
                                        spawn(async move {
                                            let _ = api::post_action(&format!("/api/approvals/{a}/approve")).await;
                                            resource.restart();
                                        });
                                    },
                                    "Approve"
                                }
                            }
                        }
                        {
                            let rid = reject_id.clone();
                            rsx! {
                                button {
                                    class: "btn btn-danger btn-sm",
                                    onclick: move |_| {
                                        let r = rid.clone();
                                        spawn(async move {
                                            let _ = api::post_action(&format!("/api/approvals/{r}/reject")).await;
                                            resource.restart();
                                        });
                                    },
                                    "Reject"
                                }
                            }
                        }
                    }
                } else {
                    rsx! { div {} }
                };

                rsx! {
                    div { class: "agent-card", key: "{id}",
                        div { class: "agent-info",
                            div { class: "agent-name", "{subject} → {action} on {resource_name}" }
                            div { class: "agent-id", "{reason} · {timestamp}" }
                        }
                        div { class: "card-actions",
                            span { class: "{status_class}", "{status}" }
                            {buttons}
                        }
                    }
                }
            }).collect();
            rsx! { div { {cards.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "error-box", { format!("Error: {e}") } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading approvals..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconCheckSquare { size: 20 } " Approvals" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
