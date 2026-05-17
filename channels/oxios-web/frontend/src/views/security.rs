//! Audit log table with allow/deny coloring, plus permissions management tab.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum SecurityTab {
    AuditLog,
    Permissions,
}

// ---------------------------------------------------------------------------
// Audit Log Tab
// ---------------------------------------------------------------------------

#[component]
fn AuditLogTab() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::AuditLogEntry>("/api/audit").await
    });
    let mut verify_valid = use_signal(|| Option::<bool>::None);
    let mut is_exporting = use_signal(|| false);

    let content: Element = match &(resource.value())() {
        Some(Ok(entries)) if entries.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconShield { size: 40 } }
                p { "No audit log entries yet." }
            }
        },
        Some(Ok(entries)) => {
            let rows: Vec<Element> = entries.iter().map(|entry| {
                let timestamp = entry.timestamp.clone();
                let agent = entry.agent_name.clone();
                let action = entry.action.clone();
                let resource_name = entry.resource.clone();
                let allowed = entry.allowed;
                let reason = entry.reason.clone().unwrap_or_default();

                let status_class = if allowed {
                    "status-badge status-badge-active"
                } else {
                    "status-badge status-badge-inactive"
                };
                let status_text = if allowed { "Allow" } else { "Deny" };

                rsx! {
                    div { class: "agent-card", key: "{timestamp}-{action}-{agent}",
                        div { class: "agent-info",
                            div { class: "agent-name",
                                span { style: "color:var(--text-0)", "{agent}" }
                                span { style: "color:var(--text-3);margin:0 8px", "→" }
                                span { style: "color:var(--accent)", "{action}" }
                                span { style: "color:var(--text-3);margin:0 8px", "on" }
                                span { style: "color:var(--text-0)", "{resource_name}" }
                            }
                            div { class: "agent-id", "{timestamp} · {reason}" }
                        }
                        span { class: "{status_class}", "{status_text}" }
                    }
                }
            }).collect();
            rsx! { div { {rows.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading audit log..." }
            }
        },
    };

    let verify_msg = match verify_valid() {
        Some(true) => "✓ Integrity verified: log is intact",
        Some(false) => "✗ Integrity check failed: log may be tampered",
        None => "",
    };

    rsx! {
        div { class: "tab-content",
            div { class: "action-bar",
                button {
                    class: "btn btn-sm",
                    disabled: is_exporting(),
                    onclick: move |_| {
                        is_exporting.set(true);
                        spawn(async move {
                            match api::post_action("/api/audit/export").await {
                                Ok(_) => {
                                    is_exporting.set(false);
                                }
                                Err(_) => {
                                    is_exporting.set(false);
                                }
                            }
                        });
                    },
                    IconCopy { size: 14 }
                    " Export"
                }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        verify_valid.set(None);
                        spawn(async move {
                            #[derive(serde::Deserialize)]
                            struct VerifyResponse { valid: bool }
                            match api::fetch_json::<VerifyResponse>("/api/audit/verify").await {
                                Ok(resp) => verify_valid.set(Some(resp.valid)),
                                Err(_) => verify_valid.set(Some(false)),
                            }
                        });
                    },
                    IconShield { size: 14 }
                    " Verify Integrity"
                }
                if !verify_msg.is_empty() {
                    span { class: "verify-result", "{verify_msg}" }
                }
            }
            div { class: "panel-body", {content} }
            div { class: "panel-footer",
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Permissions Tab
// ---------------------------------------------------------------------------

#[component]
fn PermissionsTab() -> Element {
    let mut agent_name = use_signal(|| String::new());
    let mut permissions_data = use_signal(|| Option::<serde_json::Value>::None);
    let mut is_loading = use_signal(|| false);
    let mut is_saving = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut success_msg = use_signal(|| Option::<String>::None);
    let mut editor_content = use_signal(|| String::new());

    rsx! {
        div { class: "tab-content",
            div { class: "permissions-toolbar",
                input {
                    class: "input input-sm",
                    placeholder: "Agent name",
                    value: "{agent_name}",
                    oninput: move |e| agent_name.set(e.value().clone()),
                    onkeydown: move |e| {
                        if e.key() == "Enter" {
                            let name = agent_name().trim().to_string();
                            if name.is_empty() {
                                error_msg.set(Some("Agent name required".to_string()));
                                return;
                            }
                            error_msg.set(None);
                            success_msg.set(None);
                            is_loading.set(true);
                            let name_clone = name.clone();
                            spawn(async move {
                                match api::fetch_json::<serde_json::Value>(&format!("/api/permissions/{name_clone}")).await {
                                    Ok(data) => {
                                        permissions_data.set(Some(data.clone()));
                                        editor_content.set(serde_json::to_string_pretty(&data).unwrap_or_default());
                                        is_loading.set(false);
                                    }
                                    Err(e) => {
                                        error_msg.set(Some(format!("Error: {e}")));
                                        is_loading.set(false);
                                    }
                                }
                            });
                        }
                    }
                }
                button {
                    class: "btn btn-sm",
                    disabled: is_loading(),
                    onclick: move |_| {
                        let name = agent_name().trim().to_string();
                        if name.is_empty() {
                            error_msg.set(Some("Agent name required".to_string()));
                            return;
                        }
                        error_msg.set(None);
                        success_msg.set(None);
                        is_loading.set(true);
                        let name_clone = name.clone();
                        spawn(async move {
                            match api::fetch_json::<serde_json::Value>(&format!("/api/permissions/{name_clone}")).await {
                                Ok(data) => {
                                    permissions_data.set(Some(data.clone()));
                                    editor_content.set(serde_json::to_string_pretty(&data).unwrap_or_default());
                                    is_loading.set(false);
                                }
                                Err(e) => {
                                    error_msg.set(Some(format!("Error: {e}")));
                                    is_loading.set(false);
                                }
                            }
                        });
                    },
                    IconSearch { size: 14 }
                    " Lookup"
                }
            }
            if let Some(err) = error_msg() {
                div { class: "message message-error", "{err}" }
            }
            if let Some(msg) = success_msg() {
                div { class: "message message-success", "{msg}" }
            }
            if is_loading() {
                div { class: "loading-state",
                    div { class: "empty-icon", IconLoading { size: 24 } }
                    p { "Loading permissions..." }
                }
            } else if permissions_data().is_some() {
                div { class: "permissions-editor",
                    div { class: "editor-header",
                        span { "JSON Editor" }
                        button {
                            class: "btn btn-sm",
                            disabled: is_saving(),
                            onclick: move |_| {
                                let name = agent_name().trim().to_string();
                                if name.is_empty() {
                                    error_msg.set(Some("Agent name required".to_string()));
                                    return;
                                }
                                let json_str = editor_content();
                                match serde_json::from_str::<serde_json::Value>(&json_str) {
                                    Ok(json) => {
                                        error_msg.set(None);
                                        success_msg.set(None);
                                        is_saving.set(true);
                                        let name_clone = name.clone();
                                        spawn(async move {
                                            match api::put_json::<(), _>(&format!("/api/permissions/{name_clone}"), &json).await {
                                                Ok(_) => {
                                                    success_msg.set(Some("Permissions saved successfully".to_string()));
                                                    is_saving.set(false);
                                                }
                                                Err(e) => {
                                                    error_msg.set(Some(format!("Save error: {e}")));
                                                    is_saving.set(false);
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        error_msg.set(Some(format!("Invalid JSON: {e}")));
                                    }
                                }
                            },
                            IconCheck { size: 14 }
                            " Save"
                        }
                    }
                    textarea {
                        class: "json-editor",
                        rows: 20,
                        value: "{editor_content}",
                        oninput: move |e| editor_content.set(e.value().clone()),
                        spellcheck: "false"
                    }
                }
            } else {
                div { class: "empty-state",
                    div { class: "empty-icon", IconShield { size: 40 } }
                    p { "Enter an agent name and click Lookup to view/edit permissions." }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main Security View
// ---------------------------------------------------------------------------

#[component]
pub fn SecurityView() -> Element {
    let mut active_tab = use_signal(|| SecurityTab::AuditLog);

    let tab_class = |tab: SecurityTab| -> String {
        if active_tab() == tab {
            "tab tab-active".to_string()
        } else {
            "tab".to_string()
        }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconShield { size: 20 } " Security" }
            }
            div { class: "tabs",
                div {
                    class: "{tab_class(SecurityTab::AuditLog)}",
                    onclick: move |_| active_tab.set(SecurityTab::AuditLog),
                    IconShield { size: 16 }
                    " Audit Log"
                }
                div {
                    class: "{tab_class(SecurityTab::Permissions)}",
                    onclick: move |_| active_tab.set(SecurityTab::Permissions),
                    IconKey { size: 16 }
                    " Permissions"
                }
            }
            if active_tab() == SecurityTab::AuditLog {
                AuditLogTab {}
            } else {
                PermissionsTab {}
            }
        }
    }
}