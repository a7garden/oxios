//! Numeric input with min/max validation.

use dioxus::prelude::*;

#[component]
pub fn SettingsNumberInput(
    label: &'static str,
    value: f64,
    onchange: EventHandler<f64>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    unit: Option<&'static str>,
    description: Option<&'static str>,
) -> Element {
    let step_val = step.unwrap_or(1.0);
    let unit_str = unit.unwrap_or("");

    // Range hint
    let range_hint = match (min, max) {
        (Some(m), Some(x)) => Some(format!("({:.0} – {:.0})", m, x)),
        _ => None,
    };

    // Validate if out of range
    let is_out_of_range = match (min, max) {
        (Some(m), _) if value < m => true,
        (_, Some(x)) if value > x => true,
        _ => false,
    };

    let input_class = if is_out_of_range {
        "input input-sm settings-number-input error"
    } else {
        "input input-sm settings-number-input"
    };

    rsx! {
        div { class: "settings-field",
            div { class: "settings-field-label",
                span { class: "settings-field-title", "{label}" }
                if let Some(desc) = description {
                    p { class: "description", "{desc}" }
                }
                if let Some(hint) = range_hint {
                    p { class: "description", "{hint}" }
                }
                if is_out_of_range {
                    p { class: "description error-text", "Value out of range!" }
                }
            }
            div { class: "settings-field-control",
                div { style: "display:flex;align-items:center;gap:8px",
                    input {
                        class: "{input_class}",
                        r#type: "number",
                        min: min.map(|v| v.to_string()).unwrap_or_default(),
                        max: max.map(|v| v.to_string()).unwrap_or_default(),
                        step: "{step_val}",
                        value: "{value}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f64>() {
                                onchange.call(v);
                            }
                        },
                    }
                    if !unit_str.is_empty() {
                        span { style: "color:var(--text-3);font-size:12px", "{unit_str}" }
                    }
                }
            }
        }
    }
}