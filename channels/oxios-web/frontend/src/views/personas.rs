//! Persona cards with full CRUD: create, edit, delete, and set-active.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Modal state for the persona create/edit dialog.
#[derive(Clone, Default)]
struct PersonaModal {
    /// Whether the modal is visible.
    open: bool,
    /// If Some, we are editing an existing persona; otherwise creating new.
    edit_id: Option<String>,
    /// Form field: persona name.
    name: String,
    /// Form field: persona role.
    role: String,
    /// Form field: persona description.
    description: String,
    /// Form field: whether persona is enabled.
    enabled: bool,
    /// Form field: comma-separated personality traits.
    traits: String,
    /// Status feedback message.
    status: Option<String>,
}

impl PersonaModal {
    fn open_create() -> Self {
        Self {
            open: true,
            edit_id: None,
            name: String::new(),
            role: String::new(),
            description: String::new(),
            enabled: true,
            traits: String::new(),
            status: None,
        }
    }

    fn open_edit(p: &api::PersonaSummary) -> Self {
        Self {
            open: true,
            edit_id: Some(p.id.clone()),
            name: p.name.clone(),
            role: p.role.clone(),
            description: p.description.clone(),
            enabled: p.enabled,
            traits: p.personality_traits.join(", "),
            status: None,
        }
    }
}

/// State for the delete confirmation dialog.
#[derive(Clone, Default)]
struct DeleteConfirm {
    /// Whether the dialog is visible.
    open: bool,
    /// ID of the persona to delete.
    id: String,
    /// Name of the persona to delete (for display).
    name: String,
}

#[component]
pub fn PersonasView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<Vec<api::PersonaSummary>>("/api/personas").await
    });

    let mut active_resource = use_resource(|| async move {
        api::fetch_json::<serde_json::Value>("/api/personas/active").await
    });

    let mut modal = use_signal(PersonaModal::default);
    let mut delete_confirm = use_signal(DeleteConfirm::default);

    let active_id: Option<String> = match &(active_resource.value())() {
        Some(Ok(val)) => val.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    };

    let content: Element = match &(resource.value())() {
        Some(Ok(personas)) if personas.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconUsers { size: 40 } }
                p { "No personas configured." }
            }
        },
        Some(Ok(personas)) => {
            let cards: Vec<Element> = personas
                .iter()
                .map(|persona| {
                    let id = persona.id.clone();
                    let is_active = active_id.as_ref() == Some(&id);
                    let traits = persona.personality_traits.join(", ");
                    let set_active_id = id.clone();

                    let active_badge: Element = if is_active {
                        rsx! { span { class: "status-badge status-badge-active", "Active" } }
                    } else {
                        rsx! { div {} }
                    };

                    let set_active_btn: Element = if is_active {
                        rsx! { div {} }
                    } else {
                        let sid = set_active_id.clone();
                        rsx! {
                            button {
                                class: "btn btn-sm",
                                onclick: move |_| {
                                    let s = sid.clone();
                                    spawn(async move {
                                        let _ = api::put_json::<serde_json::Value, _>(
                                            "/api/personas/active",
                                            &serde_json::json!({ "id": s }),
                                        )
                                        .await;
                                        resource.restart();
                                        active_resource.restart();
                                    });
                                },
                                "Set Active"
                            }
                        }
                    };

                    let _edit_id = id.clone();
                    let edit_persona = persona.clone();
                    let del_id = id.clone();
                    let del_name = persona.name.clone();

                    rsx! {
                        div { class: "agent-card", key: "{id}",
                            div { class: "agent-info",
                                div { class: "agent-name",
                                    "{persona.name}"
                                    {active_badge}
                                }
                                div { class: "agent-id", "{persona.role} · {traits}" }
                                div { class: "agent-id", "{persona.description}" }
                            }
                            div { class: "card-actions",
                                {set_active_btn}
                                button {
                                    class: "btn btn-sm",
                                    onclick: move |_| {
                                        modal.set(PersonaModal::open_edit(&edit_persona));
                                    },
                                    IconWrench { size: 14 }
                                }
                                button {
                                    class: "btn btn-danger btn-sm",
                                    onclick: move |_| {
                                        delete_confirm.set(DeleteConfirm {
                                            open: true,
                                            id: del_id.clone(),
                                            name: del_name.clone(),
                                        });
                                    },
                                    IconTrash { size: 14 }
                                }
                            }
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
                p { "Loading personas..." }
            }
        },
    };

    // Modal overlay for create/edit
    let modal_el: Element = {
        let m = modal();
        if !m.open {
            return rsx! { div {} };
        }
        let title = if m.edit_id.is_some() { "Edit Persona" } else { "Create Persona" };
        let status_msg = m.status.clone();
        let edit_id = m.edit_id.clone();

        rsx! {
            div {
                class: "modal-overlay",
                onclick: move |_| {
                    let mut m = modal();
                    m.open = false;
                    modal.set(m);
                },
                div {
                    class: "modal",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-header",
                        h3 { "{title}" }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                let mut m = modal();
                                m.open = false;
                                modal.set(m);
                            },
                            IconX { size: 16 }
                        }
                    }
                    div { class: "modal-body",
                        div { class: "form-group",
                            label { "Name" }
                            input {
                                r#type: "text",
                                value: "{m.name}",
                                oninput: move |evt| {
                                    let mut m = modal();
                                    m.name = evt.value();
                                    modal.set(m);
                                },
                            }
                        }
                        div { class: "form-group",
                            label { "Role" }
                            input {
                                r#type: "text",
                                value: "{m.role}",
                                oninput: move |evt| {
                                    let mut m = modal();
                                    m.role = evt.value();
                                    modal.set(m);
                                },
                            }
                        }
                        div { class: "form-group",
                            label { "Description" }
                            textarea {
                                value: "{m.description}",
                                oninput: move |evt| {
                                    let mut m = modal();
                                    m.description = evt.value();
                                    modal.set(m);
                                },
                            }
                        }
                        div { class: "form-group",
                            label { style: "display:flex;align-items:center;gap:6px;cursor:pointer",
                                input {
                                    r#type: "checkbox",
                                    checked: "{m.enabled}",
                                    onchange: move |evt| {
                                        let mut m = modal();
                                        m.enabled = evt.checked();
                                        modal.set(m);
                                    },
                                }
                                "Enabled"
                            }
                        }
                        div { class: "form-group",
                            label { "Personality Traits (comma-separated)" }
                            input {
                                r#type: "text",
                                value: "{m.traits}",
                                oninput: move |evt| {
                                    let mut m = modal();
                                    m.traits = evt.value();
                                    modal.set(m);
                                },
                            }
                        }
                    },
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
                                let mut m = modal();
                                m.open = false;
                                modal.set(m);
                            },
                            "Cancel"
                        }
                        {
                            let name = m.name.clone();
                            let role = m.role.clone();
                            let desc = m.description.clone();
                            let enabled = m.enabled;
                            let traits_str = m.traits.clone();
                            let eid = edit_id.clone();
                            rsx! {
                                button {
                                    class: "btn btn-primary btn-sm",
                                    onclick: move |_| {
                                        let body = serde_json::json!({
                                            "name": name,
                                            "role": role,
                                            "description": desc,
                                            "enabled": enabled,
                                            "personality_traits": traits_str.split(',')
                                                .map(|s| s.trim().to_string())
                                                .filter(|s| !s.is_empty())
                                                .collect::<Vec<_>>(),
                                        });
                                        let eid = eid.clone();
                                        spawn(async move {
                                            let result = if let Some(id) = &eid {
                                                api::put_json::<serde_json::Value, _>(
                                                    &format!("/api/personas/{id}"),
                                                    &body,
                                                )
                                                .await
                                            } else {
                                                api::post_json::<serde_json::Value, _>(
                                                    "/api/personas",
                                                    &body,
                                                )
                                                .await
                                            };
                                            match result {
                                                Ok(_) => {
                                                    let mut m = modal();
                                                    m.open = false;
                                                    modal.set(m);
                                                    resource.restart();
                                                    active_resource.restart();
                                                }
                                                Err(e) => {
                                                    let mut m = modal();
                                                    m.status = Some(format!("✗ {e}"));
                                                    modal.set(m);
                                                }
                                            }
                                        });
                                    },
                                    "Save"
                                }
                            }
                        }
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
        let del_id = dc.id.clone();
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
                        h3 { "Delete Persona" }
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
                                let id = del_id.clone();
                                spawn(async move {
                                    let _ = api::delete_action(&format!("/api/personas/{id}")).await;
                                    resource.restart();
                                    active_resource.restart();
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
                h2 { IconUsers { size: 20 } " Personas" }
                div { style: "display:flex;gap:6px",
                    button {
                        class: "btn btn-primary btn-sm",
                        onclick: move |_| {
                            modal.set(PersonaModal::open_create());
                        },
                        IconWrench { size: 14 }
                        " Create"
                    }
                    button {
                        class: "btn btn-sm",
                        onclick: move |_| {
                            resource.restart();
                            active_resource.restart();
                        },
                        "Refresh"
                    }
                }
            }
            div { class: "panel-body",
                {content}
            }
            {modal_el}
            {delete_el}
        }
    }
}
