//! Toggle button group for enum selection.

use dioxus::prelude::*;

use crate::components::settings::SelectOption;

#[component]
pub fn SettingsSelectGroup(
    label: &'static str,
    options: Vec<SelectOption>,
    selected: String,
    onchange: EventHandler<String>,
    description: Option<&'static str>,
) -> Element {
    rsx! {
        div { class: "settings-field",
            div { class: "settings-field-label",
                span { class: "settings-field-title", "{label}" }
                if let Some(desc) = description {
                    p { class: "description", "{desc}" }
                }
            }
            div { class: "settings-field-control",
                div { class: "settings-select-group",
                    for opt in options {
                        SelectGroupButton {
                            key: "{opt.value}",
                            opt_value: opt.value,
                            label: opt.label,
                            is_selected: selected == opt.value,
                            onchange: onchange.clone(),
                        }
                    }
                }
            }
        }
    }
}

/// A single button in the select group.
#[component]
fn SelectGroupButton(
    opt_value: &'static str,
    label: &'static str,
    is_selected: bool,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            class: if is_selected { "btn btn-primary btn-sm" } else { "btn btn-sm" },
            onclick: move |_| onchange.call(opt_value.to_string()),
            "{label}"
        }
    }
}