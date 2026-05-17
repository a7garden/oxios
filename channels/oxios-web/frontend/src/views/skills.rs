//! Skill list with delete support.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[component]
pub fn SkillsView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::SkillInfo>("/api/skills").await
    });

    let content: Element = match &(resource.value())() {
        Some(Ok(skills)) if skills.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconSkills { size: 40 } }
                p { "No skills registered. Skills define agent instruction templates." }
            }
        },
        Some(Ok(skills)) => {
            let items: Vec<Element> = skills.iter().map(|skill| {
                let name = skill.name.clone();
                let desc = skill.description.clone();
                let del_name = name.clone();
                rsx! {
                    div { class: "item-card", key: "{name}",
                        div { style: "display:flex;justify-content:space-between;align-items:center",
                            div {
                                div { class: "item-title", "{name}" }
                                div { class: "item-subtitle", "{desc}" }
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
            }).collect();
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

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconSkills { size: 20 } " Skills" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
