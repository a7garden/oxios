//! Multi-checkbox for set selection.

use dioxus::prelude::*;

use crate::components::settings::SelectOption;

#[component]
pub fn SettingsMultiCheckbox(
    label: &'static str,
    options: Vec<SelectOption>,
    selected: Vec<String>,
    onchange: EventHandler<Vec<String>>,
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
                div { class: "settings-multi-checkbox",
                    for opt in options {
                        CheckboxItem {
                            key: "{opt.value}",
                            opt_value: opt.value,
                            label: opt.label,
                            is_checked: selected.iter().any(|s| *s == opt.value),
                            selected: selected.clone(),
                            onchange: onchange.clone(),
                        }
                    }
                }
            }
        }
    }
}

/// A single checkbox item.
#[component]
fn CheckboxItem(
    opt_value: &'static str,
    label: &'static str,
    is_checked: bool,
    selected: Vec<String>,
    onchange: EventHandler<Vec<String>>,
) -> Element {
    rsx! {
        div {
            class: "settings-checkbox-item",
            onclick: move |_| {
                let mut new_selected = selected.clone();
                let val = opt_value.to_string();
                if new_selected.contains(&val) {
                    new_selected.retain(|v| v != &val);
                } else {
                    new_selected.push(val);
                }
                onchange.call(new_selected);
            },
            if is_checked {
                span { class: "settings-checkbox checked" }
            } else {
                span { class: "settings-checkbox" }
            }
            span { style: "font-size:13px;color:var(--text-1)", "{label}" }
        }
    }
}