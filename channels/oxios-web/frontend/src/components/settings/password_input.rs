//! Password input with masking and set indicator.

use dioxus::prelude::*;

use crate::components::icons::IconEye;

#[component]
pub fn SettingsPasswordInput(
    label: &'static str,
    is_set: bool,
    onchange: EventHandler<String>,
    description: Option<&'static str>,
) -> Element {
    let mut show_password = use_signal(|| false);
    let mut local_value = use_signal(String::new);

    let input_type = if show_password() { "text" } else { "password" };

    // Status text
    let status_text = if local_value().is_empty() {
        if is_set {
            "✓ Currently set"
        } else {
            "Not set"
        }
    } else {
        "New value entered"
    };

    let status_class = if local_value().is_empty() {
        if is_set { "password-status-set" } else { "password-status-none" }
    } else {
        "password-status-new"
    };

    rsx! {
        div { class: "settings-field",
            div { class: "settings-field-label",
                span { class: "settings-field-title", "{label}" }
                if let Some(desc) = description {
                    p { class: "description", "{desc}" }
                }
            }
            div { class: "settings-field-control",
                div { class: "settings-password-container",
                    input {
                        class: "input input-sm settings-password-input",
                        r#type: "{input_type}",
                        placeholder: "Enter new value or leave empty...",
                        value: "{local_value}",
                        oninput: move |e| {
                            local_value.set(e.value().clone());
                            onchange.call(e.value().clone());
                        },
                    }
                    button {
                        class: "icon-btn settings-password-toggle",
                        onclick: move |_| show_password.toggle(),
                        IconEye { size: 16 }
                    }
                    p { class: "{status_class}", "{status_text}" }
                }
            }
        }
    }
}