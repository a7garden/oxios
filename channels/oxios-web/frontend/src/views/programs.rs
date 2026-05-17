//! Program list with install, delete, details, and enable/disable toggle.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Modal state for installing a new program.
#[derive(Clone, Default)]
struct InstallModal {
    /// Whether the modal is visible.
    open: bool,
    /// Form field: source path or URL.
    source: String,
    /// Status feedback message.
    status: Option<String>,
}

/// State for viewing program detail.
#[derive(Clone, Default)]
struct ProgramDetail {
    /// Whether the detail view is visible.
    open: bool,
    /// Name of the program being viewed.
    name: String,
    /// Pretty-printed JSON detail.
    content: String,
}

/// State for the delete confirmation dialog.
#[derive(Clone, Default)]
struct DeleteConfirm {
    /// Whether the dialog is visible.
    open: bool,
    /// Name of the program to delete.
    name: String,
}

#[component]
pub fn ProgramsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::ProgramSummary>("/api/programs").await
    });

    let mut install_modal = use_signal(InstallModal::default);
    let mut program_detail = use_signal(ProgramDetail::default);
    let mut delete_confirm = use_signal(DeleteConfirm::default);

    let content: Element = match &(resource.value())() {
        Some(Ok(programs)) if programs.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconPackage { size: 40 } }
                p { "No programs installed. Install a program to extend agent capabilities." }
            }
        },
        Some(Ok(programs)) => {
            let cards: Vec<Element> = programs
                .iter()
                .map(|prog| {
                    let name = prog.name.clone();
                    let enabled = prog.enabled;
                    let enabled_class = if enabled { "program-enabled yes" } else { "program-enabled no" };
                    let enabled_text = if enabled { "Enabled" } else { "Disabled" };
                    let action_class = if enabled { "btn btn-danger btn-sm" } else { "btn btn-sm" };
                    let action_label = if enabled { "Disable" } else { "Enable" };
                    let action_name = name.clone();
                    let view_name = name.clone();
                    let del_name = name.clone();

                    rsx! {
                        div { class: "program-card", key: "{name}",
                            div { style: "display:flex;align-items:center;justify-content:space-between",
                                div {
                                    span { class: "program-name", "{name}" }
                                    span { class: "program-version", "v{prog.version}" }
                                    span { class: "{enabled_class}", "{enabled_text}" }
                                }
                                div { style: "display:flex;gap:4px",
                                    // Enable / Disable toggle
                                    {
                                        let an = action_name.clone();
                                        rsx! {
                                            button {
                                                class: "{action_class}",
                                                onclick: move |_| {
                                                    let n = an.clone();
                                                    let is_enabled = enabled;
                                                    spawn(async move {
                                                        if is_enabled {
                                                            let _ = api::post_action(&format!("/api/programs/{n}/disable")).await;
                                                        } else {
                                                            let _ = api::post_action(&format!("/api/programs/{n}/enable")).await;
                                                        }
                                                        resource.restart();
                                                    });
                                                },
                                                "{action_label}"
                                            }
                                        }
                                    }
                                    // Details button
                                    {
                                        let vn = view_name.clone();
                                        rsx! {
                                            button {
                                                class: "btn btn-sm",
                                                onclick: move |_| {
                                                    let n = vn.clone();
                                                    program_detail.set(ProgramDetail {
                                                        open: true,
                                                        name: n.clone(),
                                                        content: "Loading…".to_string(),
                                                    });
                                                    spawn(async move {
                                                        match api::fetch_json::<serde_json::Value>(
                                                            &format!("/api/programs/{n}"),
                                                        )
                                                        .await
                                                        {
                                                            Ok(val) => {
                                                                let pretty = serde_json::to_string_pretty(&val)
                                                                    .unwrap_or_default();
                                                                program_detail.set(ProgramDetail {
                                                                    open: true,
                                                                    name: n,
                                                                    content: pretty,
                                                                });
                                                            }
                                                            Err(e) => {
                                                                program_detail.set(ProgramDetail {
                                                                    open: true,
                                                                    name: n,
                                                                    content: format!("Error: {e}"),
                                                                });
                                                            }
                                                        }
                                                    });
                                                },
                                                IconEye { size: 14 }
                                            }
                                        }
                                    }
                                    // Delete button
                                    {
                                        let dn = del_name.clone();
                                        rsx! {
                                            button {
                                                class: "btn btn-danger btn-sm",
                                                onclick: move |_| {
                                                    delete_confirm.set(DeleteConfirm {
                                                        open: true,
                                                        name: dn.clone(),
                                                    });
                                                },
                                                IconTrash { size: 14 }
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "program-desc", "{prog.description}" }
                        }
                    }
                })
                .collect();
            rsx! { div { {cards.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading programs..." }
            }
        },
    };

    // Install program modal
    let install_el: Element = {
        let im = install_modal();
        if !im.open {
            return rsx! { div {} };
        }
        let status_msg = im.status.clone();
        rsx! {
            div {
                class: "modal-overlay",
                onclick: move |_| {
                    install_modal.set(InstallModal::default());
                },
                div {
                    class: "modal modal-sm",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-header",
                        h3 { "Install Program" }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                install_modal.set(InstallModal::default());
                            },
                            IconX { size: 16 }
                        }
                    }
                    div { class: "modal-body",
                        div { class: "form-group",
                            label { "Source (path or URL)" }
                            input {
                                r#type: "text",
                                placeholder: "/path/to/program or https://…",
                                value: "{im.source}",
                                oninput: move |evt| {
                                    let mut m = install_modal();
                                    m.source = evt.value();
                                    install_modal.set(m);
                                },
                            }
                        }
                    }
                    div { class: "modal-footer",
                        {
                            let msg = status_msg.clone();
                            rsx! {
                                match msg {
                                    Some(s) if s.starts_with("✓") => rsx! {
                                        span { style: "color:var(--success);font-size:12px", "{s}" }
                                    },
                                    Some(s) => rsx! {
                                        span { style: "color:var(--danger);font-size:12px", "{s}" }
                                    },
                                    None => rsx! { div {} },
                                }
                            }
                        }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                install_modal.set(InstallModal::default());
                            },
                            "Cancel"
                        }
                        {
                            let source = im.source.clone();
                            rsx! {
                                button {
                                    class: "btn btn-primary btn-sm",
                                    onclick: move |_| {
                                        let body = serde_json::json!({ "source": source });
                                        spawn(async move {
                                            match api::post_json::<serde_json::Value, _>(
                                                "/api/programs",
                                                &body,
                                            )
                                            .await
                                            {
                                                Ok(_) => {
                                                    install_modal.set(InstallModal::default());
                                                    resource.restart();
                                                }
                                                Err(e) => {
                                                    let mut m = install_modal();
                                                    m.status = Some(format!("✗ {e}"));
                                                    install_modal.set(m);
                                                }
                                            }
                                        });
                                    },
                                    IconPackage { size: 14 }
                                    " Install"
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // Program detail view
    let detail_el: Element = {
        let pd = program_detail();
        if !pd.open {
            return rsx! { div {} };
        }
        let detail_content = pd.content.clone();
        let detail_name = pd.name.clone();
        rsx! {
            div {
                class: "modal-overlay",
                onclick: move |_| {
                    program_detail.set(ProgramDetail::default());
                },
                div {
                    class: "modal",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-header",
                        h3 { "Program: {detail_name}" }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                program_detail.set(ProgramDetail::default());
                            },
                            IconX { size: 16 }
                        }
                    }
                    div { class: "modal-body",
                        pre { style: "white-space:pre-wrap;word-break:break-word;max-height:400px;overflow:auto;font-size:12px", "{detail_content}" }
                    }
                }
            }
        }
    };

    // Delete confirmation dialog
    let delete_el: Element = {
        let dc = delete_confirm();
        if !dc.open {
            return rsx! { div {} };
        }
        let del_name = dc.name.clone();
        rsx! {
            div {
                class: "modal-overlay",
                onclick: move |_| {
                    delete_confirm.set(DeleteConfirm::default());
                },
                div {
                    class: "modal modal-sm",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-header",
                        h3 { "Delete Program" }
                    }
                    div { class: "modal-body",
                        p { "Are you sure you want to delete \"{del_name}\"?" }
                    }
                    div { class: "modal-footer",
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                delete_confirm.set(DeleteConfirm::default());
                            },
                            "Cancel"
                        }
                        button {
                            class: "btn btn-danger btn-sm",
                            onclick: move |_| {
                                let name = del_name.clone();
                                spawn(async move {
                                    let _ = api::delete_action(&format!("/api/programs/{name}")).await;
                                    resource.restart();
                                });
                                delete_confirm.set(DeleteConfirm::default());
                            },
                            "Delete"
                        }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconPackage { size: 20 } " Programs" }
                div { style: "display:flex;gap:6px",
                    button {
                        class: "btn btn-primary btn-sm",
                        onclick: move |_| {
                            install_modal.set(InstallModal {
                                open: true,
                                ..Default::default()
                            });
                        },
                        IconPackage { size: 14 }
                        " Install"
                    }
                    button {
                        class: "btn btn-sm",
                        onclick: move |_| resource.restart(),
                        "Refresh"
                    }
                }
            }
            div { class: "panel-body",
                {content}
            }
            {install_el}
            {detail_el}
            {delete_el}
        }
    }
}
