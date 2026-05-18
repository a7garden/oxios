//! Range slider with current value display.

use dioxus::prelude::*;

#[component]
pub fn SettingsSlider(
    label: &'static str,
    value: f64,
    onchange: EventHandler<f64>,
    min: f64,
    max: f64,
    step: Option<f64>,
    unit: Option<&'static str>,
    show_max: Option<bool>,
    description: Option<&'static str>,
) -> Element {
    let step_val = step.unwrap_or(1.0);
    let show_max_val = show_max.unwrap_or(false);
    let unit_str = unit.unwrap_or("");

    let value_display = if show_max_val {
        format!("{:.0} / {:.0}", value, max)
    } else {
        format!("{:.0}{}", value, unit_str)
    };

    rsx! {
        div { class: "settings-field",
            div { class: "settings-field-label",
                span { class: "settings-field-title", "{label}" }
                if let Some(desc) = description {
                    p { class: "description", "{desc}" }
                }
            }
            div { class: "settings-field-control settings-slider-control",
                input {
                    class: "settings-slider",
                    r#type: "range",
                    min: "{min}",
                    max: "{max}",
                    step: "{step_val}",
                    value: "{value}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f64>() {
                            onchange.call(v);
                        }
                    },
                }
                span { class: "settings-slider-value", "{value_display}" }
            }
        }
    }
}