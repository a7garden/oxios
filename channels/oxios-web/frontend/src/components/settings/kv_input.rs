//! Key=Value tag input for MCP server environment variables.

use std::collections::HashMap;

use dioxus::prelude::*;

#[component]
pub fn SettingsKeyValueInput(
    label: &'static str,
    values: HashMap<String, String>,
    onchange: EventHandler<HashMap<String, String>>,
    placeholder: Option<&'static str>,
    description: Option<&'static str>,
) -> Element {
    let mut input_key = use_signal(String::new);
    let placeholder_str = placeholder.unwrap_or("KEY=VALUE");

    let entries: Vec<(String, String)> = values.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

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
                    for (key, value) in entries.iter() {
                        KvChip {
                            key: key.clone(),
                            value: value.clone(),
                            onchange: onchange.clone(),
                            current: values.clone(),
                        }
                    }
                    input {
                        class: "input input-sm settings-tag-input",
                        placeholder: "{placeholder_str}",
                        value: "{input_key}",
                        oninput: move |e| input_key.set(e.value()),
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key().to_string() == "Enter" {
                                let raw = input_key().trim().to_string();
                                if let Some((k, v)) = raw.split_once('=') {
                                    let k = k.trim().to_string();
                                    let v = v.trim().to_string();
                                    if !k.is_empty() {
                                        let mut new_map = values.clone();
                                        new_map.insert(k, v);
                                        onchange.call(new_map);
                                        input_key.set(String::new());
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

/// A single KEY=VALUE chip with remove button.
#[component]
fn KvChip(
    key: String,
    value: String,
    onchange: EventHandler<HashMap<String, String>>,
    current: HashMap<String, String>,
) -> Element {
    let display = format!("{key}={value}");
    rsx! {
        div { class: "tag-chip",
            span { "{display}" }
            button {
                class: "tag-chip-remove",
                onclick: move |_| {
                    let mut new_map = current.clone();
                    new_map.remove(&key);
                    onchange.call(new_map);
                },
                "×"
            }
        }
    }
}
