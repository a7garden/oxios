//! Tag chip list with add/remove.

use dioxus::prelude::*;

#[component]
pub fn SettingsTagInput(
    label: &'static str,
    values: Vec<String>,
    onchange: EventHandler<Vec<String>>,
    placeholder: Option<&'static str>,
    description: Option<&'static str>,
) -> Element {
    let mut input_value = use_signal(String::new);
    let placeholder_str = placeholder.unwrap_or("Add tag...");

    rsx! {
        div { class: "settings-field",
            div { class: "settings-field-label",
                span { class: "settings-field-title", "{label}" }
                if let Some(desc) = description {
                    p { class: "description", "{desc}" }
                }
            }
            div { class: "settings-field-control",
                div { class: "settings-tag-container",
                    for (idx, tag) in values.iter().enumerate() {
                        TagChip {
                            key: "{idx}",
                            tag: tag.clone(),
                            idx: idx,
                            onchange: onchange.clone(),
                            current_values: values.clone(),
                        }
                    }
                    input {
                        class: "input input-sm settings-tag-input",
                        placeholder: "{placeholder_str}",
                        value: "{input_value}",
                        oninput: move |e| input_value.set(e.value()),
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key().to_string() == "Enter" {
                                let val = input_value().trim().to_string();
                                if !val.is_empty() {
                                    let mut new_vals = values.clone();
                                    if !new_vals.contains(&val) {
                                        new_vals.push(val);
                                        onchange.call(new_vals);
                                    }
                                    input_value.set(String::new());
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

/// A single tag chip with remove button.
#[component]
fn TagChip(
    tag: String,
    idx: usize,
    onchange: EventHandler<Vec<String>>,
    current_values: Vec<String>,
) -> Element {
    rsx! {
        div { class: "tag-chip",
            span { "{tag}" }
            button {
                class: "tag-chip-remove",
                onclick: move |_| {
                    let mut new_vals = current_values.clone();
                    if idx < new_vals.len() {
                        new_vals.remove(idx);
                        onchange.call(new_vals);
                    }
                },
                "×"
            }
        }
    }
}