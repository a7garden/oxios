//! Skill list with create and view-detail support.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Modal state for creating a new skill.
#[derive(Clone, Default)]
struct CreateModal {
    /// Whether the modal is visible.
    open: bool,
    /// Form field: skill name.
    name: String,
    /// Form field: skill description.
    description: String,
    /// Form field: skill markdown content.
    content: String,
    /// Status feedback message.
    status: Option<String>,
}

/// State for viewing skill detail.
#[derive(Clone, Default)]
struct SkillDetail {
    /// Whether the detail view is visible.
    open: bool,
    /// Name of the skill being viewed.
    name: String,
    /// Pretty-printed JSON detail.
    content: String,
}

#[component]
pub fn SkillsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::SkillInfo>("/api/skills").await
    });

    let mut create_modal = use_signal(CreateModal::default);
    let mut skill_detail = use_signal(SkillDetail::default);

    let content: Element = match &(resource.value())() {
        Some(Ok(skills)) if skills.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconSkills { size: 40 } }
                p { "No skills registered. Skills define agent instruction templates." }
            }
        },
        Some(Ok(skills)) => {
            let items: Vec<Element> = skills
                .iter()
                .map(|skill| {
                    let name = skill.name.clone();
                    let desc = skill.description.clone();
                    let del_name = name.clone();
                    let view_name = name.clone();

                    rsx! {
                        div { class: "item-card", key: "{name}",
                            div { style: "display:flex;justify-content:space-between;align-items:center",
                                div {
                                    div { class: "item-title", "{name}" }
                                    div { class: "item-subtitle", "{desc}" }
                                }
                                div { style: "display:flex;gap:4px",
                                    {
                                        let vn = view_name.clone();
                                        rsx! {
                                            button {
                                                class: "btn btn-sm",
                                                onclick: move |_| {
                                                    let n = vn.clone();
                                                    skill_detail.set(SkillDetail {
                                                        open: true,
                                                        name: n.clone(),
                                                        content: "Loading…".to_string(),
                                                    });
                                                    spawn(async move {
                                                        match api::fetch_json::<serde_json::Value>(
                                                            &format!("/api/skills/{n}"),
                                                        )
                                                        .await
                                                        {
                                                            Ok(val) => {
                                                                let pretty = serde_json::to_string_pretty(&val)
                                                                    .unwrap_or_default();
                                                                skill_detail.set(SkillDetail {
                                                                    open: true,
                                                                    name: n,
                                                                    content: pretty,
                                                                });
                                                            }
                                                            Err(e) => {
                                                                skill_detail.set(SkillDetail {
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
                                    {
                                        let dn = del_name.clone();
                                        rsx! {
                                            button {
                                                class: "btn btn-danger btn-sm",
                                                onclick: move |_| {
                                                    let n = dn.clone();
                                                    spawn(async move {
                                                        let _ = api::delete_action(&format!("/api/skills/{n}")).await;
                                                        resource.restart();
                                                    });
                                                },
                                                IconTrash { size: 14 }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
                .collect();
            rsx! { div { class: "item-list", {items.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading skills..." }
            }
        },
    };

    // Create skill modal
    let create_el: Element = {
        let cm = create_modal();
        if !cm.open {
            return rsx! { div {} };
        }
        let status_msg = cm.status.clone();
        rsx! {
            div {
                class: "modal-overlay",
                onclick: move |_| {
                    create_modal.set(CreateModal::default());
                },
                div {
                    class: "modal",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-header",
                        h3 { "Create Skill" }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                create_modal.set(CreateModal::default());
                            },
                            IconX { size: 16 }
                        }
                    }
                    div { class: "modal-body",
                        div { class: "form-group",
                            label { "Name" }
                            input {
                                r#type: "text",
                                value: "{cm.name}",
                                oninput: move |evt| {
                                    let mut m = create_modal();
                                    m.name = evt.value();
                                    create_modal.set(m);
                                },
                            }
                        }
                        div { class: "form-group",
                            label { "Description" }
                            textarea {
                                rows: "2",
                                value: "{cm.description}",
                                oninput: move |evt| {
                                    let mut m = create_modal();
                                    m.description = evt.value();
                                    create_modal.set(m);
                                },
                            }
                        }
                        div { class: "form-group",
                            label { "Content (Markdown)" }
                            textarea {
                                rows: "10",
                                style: "font-family:monospace;font-size:13px",
                                value: "{cm.content}",
                                oninput: move |evt| {
                                    let mut m = create_modal();
                                    m.content = evt.value();
                                    create_modal.set(m);
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
                                create_modal.set(CreateModal::default());
                            },
                            "Cancel"
                        }
                        {
                            let name = cm.name.clone();
                            let desc = cm.description.clone();
                            let content = cm.content.clone();
                            rsx! {
                                button {
                                    class: "btn btn-primary btn-sm",
                                    onclick: move |_| {
                                        let body = serde_json::json!({
                                            "name": name,
                                            "description": desc,
                                            "content": content,
                                        });
                                        spawn(async move {
                                            match api::post_json::<serde_json::Value, _>(
                                                "/api/skills",
                                                &body,
                                            )
                                            .await
                                            {
                                                Ok(_) => {
                                                    create_modal.set(CreateModal::default());
                                                    resource.restart();
                                                }
                                                Err(e) => {
                                                    let mut m = create_modal();
                                                    m.status = Some(format!("✗ {e}"));
                                                    create_modal.set(m);
                                                }
                                            }
                                        });
                                    },
                                    "Create"
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // Skill detail view
    let detail_el: Element = {
        let sd = skill_detail();
        if !sd.open {
            return rsx! { div {} };
        }
        let detail_content = sd.content.clone();
        let detail_name = sd.name.clone();
        rsx! {
            div {
                class: "modal-overlay",
                onclick: move |_| {
                    skill_detail.set(SkillDetail::default());
                },
                div {
                    class: "modal",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-header",
                        h3 { "Skill: {detail_name}" }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| {
                                skill_detail.set(SkillDetail::default());
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

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconSkills { size: 20 } " Skills" }
                div { style: "display:flex;gap:6px",
                    button {
                        class: "btn btn-primary btn-sm",
                        onclick: move |_| {
                            create_modal.set(CreateModal {
                                open: true,
                                ..Default::default()
                            });
                        },
                        IconSkills { size: 14 }
                        " Create"
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
            {create_el}
            {detail_el}
        }
    }
}
