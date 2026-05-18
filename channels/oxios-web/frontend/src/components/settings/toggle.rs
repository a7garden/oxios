//! Boolean toggle switch using div onClick (not CSS ::after pseudo-elements).

use dioxus::prelude::*;

#[component]
pub fn SettingsToggle(
    label: &'static str,
    value: bool,
    onchange: EventHandler<bool>,
    description: Option<&'static str>,
    dangerous: Option<bool>,
) -> Element {
    let active_class = if value { " settings-toggle-thumb active" } else { " settings-toggle-thumb" };
    let dangerous_class = if dangerous.is_some() && dangerous.unwrap() && value {
        " settings-toggle dangerous"
    } else {
        " settings-toggle"
    };

    rsx! {
        div { class: "settings-field",
            div { class: "settings-field-label",
                span { class: "settings-field-title", "{label}" }
                if let Some(desc) = description {
                    p { class: "description", "{desc}" }
                }
                if dangerous.is_some() && dangerous.unwrap() && value {
                    span { class: "danger-badge", "DANGEROUS" }
                }
            }
            div { class: "settings-field-control",
                div {
                    class: "{dangerous_class}",
                    onclick: move |_| onchange.call(!value),
                    div { class: "{active_class}" }
                }
            }
        }
    }
}