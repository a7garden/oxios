//! Engine settings tab (LLM model + API key).

use dioxus::prelude::*;

use crate::components::settings::section_card::SectionCard;
use crate::components::settings::password_input::SettingsPasswordInput;
use crate::views::settings::ConfigSnapshot;

#[component]
pub fn EngineTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;

    rsx! {
        div { class: "settings-tab-content",
            SectionCard {
                title: "LLM Engine",
                description: Some("Configure the default model and API credentials."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().engine = Default::default();
                })),
                div { class: "settings-fields",
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Default Model" }
                            p { class: "description", "Model in \"provider/model\" format, e.g. openai/gpt-4o" }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                placeholder: "provider/model, e.g. openai/gpt-4o",
                                value: "{cfg().engine.default_model}",
                                oninput: move |e| cfg.write().engine.default_model = e.value(),
                            }
                        }
                    }

                    SettingsPasswordInput {
                        label: "API Key",
                        is_set: cfg().engine.api_key_set,
                        onchange: move |v: String| {
                            cfg.write().engine.api_key = if v.is_empty() { None } else { Some(v) };
                        },
                        description: Some("Override the API key from environment or auth store."),
                    }
                }
            }
        }
    }
}
