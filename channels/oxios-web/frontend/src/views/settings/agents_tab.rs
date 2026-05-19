//! Agents settings tab (scheduler + orchestrator + persona + budget).

use dioxus::prelude::*;

use crate::components::settings::section_card::SectionCard;
use crate::components::settings::toggle::SettingsToggle;
use crate::components::settings::slider::SettingsSlider;
use crate::components::settings::number_input::SettingsNumberInput;
use crate::views::settings::ConfigSnapshot;

#[component]
pub fn AgentsTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;

    rsx! {
        div { class: "settings-tab-content",
            // Scheduler
            SectionCard {
                title: "Scheduler",
                description: Some("AIOS-inspired priority-based task scheduling."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().scheduler = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsSlider {
                        label: "Max Concurrent",
                        value: cfg().scheduler.max_concurrent as f64,
                        onchange: move |v| cfg.write().scheduler.max_concurrent = v as usize,
                        min: 1.0,
                        max: 32.0,
                        step: Some(1.0),
                        show_max: Some(true),
                        unit: None,
                        description: Some("Maximum concurrent agent tasks."),
                    }
                    SettingsSlider {
                        label: "Rate Limit",
                        value: cfg().scheduler.rate_limit_per_minute as f64,
                        onchange: move |v| cfg.write().scheduler.rate_limit_per_minute = v as u32,
                        min: 1.0,
                        max: 600.0,
                        step: Some(10.0),
                        unit: Some("/min"),
                        show_max: None,
                        description: Some("Maximum LLM API calls per minute."),
                    }
                    SettingsSlider {
                        label: "Zombie Timeout",
                        value: cfg().scheduler.zombie_timeout_secs as f64,
                        onchange: move |v| cfg.write().scheduler.zombie_timeout_secs = v as u64,
                        min: 30.0,
                        max: 1800.0,
                        step: Some(30.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("Time before a running task is considered a zombie."),
                    }
                }
            }

            // Orchestrator
            SectionCard {
                title: "Orchestrator",
                description: Some("Ouroboros protocol: interview → seed → execute → evaluate → evolve."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().orchestrator = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsNumberInput {
                        label: "Max Evolution Iterations",
                        value: cfg().orchestrator.max_evolution_iterations as f64,
                        onchange: move |v| cfg.write().orchestrator.max_evolution_iterations = v as usize,
                        min: Some(1.0),
                        max: Some(10.0),
                        step: Some(1.0),
                        description: Some("How many times to retry failed tasks via evolution."),
                    }
                    SettingsSlider {
                        label: "Min Evaluation Score",
                        value: cfg().orchestrator.min_evaluation_score,
                        onchange: move |v| cfg.write().orchestrator.min_evaluation_score = v,
                        min: 0.0,
                        max: 1.0,
                        step: Some(0.05),
                        unit: None,
                        show_max: None,
                        description: Some("Minimum score for a task to be considered passed."),
                    }
                }
            }

            // Persona
            SectionCard {
                title: "Persona",
                description: Some("Agent persona system — controls agent personality and capabilities."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().persona = Default::default();
                })),
                div { class: "settings-fields",
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Default Persona" }
                            p { class: "description", "Persona ID to activate on startup." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                placeholder: "e.g. dev",
                                value: "{cfg().persona.default_persona_id.as_deref().unwrap_or(\"\")}",
                                oninput: move |e| {
                                    let v = e.value();
                                    cfg.write().persona.default_persona_id = if v.is_empty() { None } else { Some(v) };
                                },
                            }
                        }
                    }
                    SettingsNumberInput {
                        label: "Max Concurrent Personas",
                        value: cfg().persona.max_concurrent_personas as f64,
                        onchange: move |v| cfg.write().persona.max_concurrent_personas = v as usize,
                        min: Some(1.0),
                        max: Some(20.0),
                        step: Some(1.0),
                        description: Some("Maximum number of concurrent personas."),
                    }
                }
            }

            // Budget
            SectionCard {
                title: "Budget",
                description: Some("Token and call budget enforcement per agent."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().budget = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Budget Enforcement",
                        value: cfg().budget.enabled,
                        onchange: move |v| cfg.write().budget.enabled = v,
                        description: Some("Enable per-agent budget limits."),
                        dangerous: None,
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Token Budget" }
                            p { class: "description", "Default token budget per agent. 0 = unlimited." }
                        }
                        div { class: "settings-field-control",
                            div { style: "display:flex;align-items:center;gap:8px",
                                input {
                                    class: "input input-sm settings-number-input",
                                    r#type: "number",
                                    min: "0",
                                    step: "1000",
                                    value: "{cfg().budget.default_token_budget}",
                                    oninput: move |e| {
                                        if let Ok(v) = e.value().parse::<u64>() {
                                            cfg.write().budget.default_token_budget = v;
                                        }
                                    },
                                }
                                span { style: "color:var(--text-3);font-size:12px", "tokens" }
                            }
                        }
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Calls Budget" }
                            p { class: "description", "Default API call budget per agent. 0 = unlimited." }
                        }
                        div { class: "settings-field-control",
                            div { style: "display:flex;align-items:center;gap:8px",
                                input {
                                    class: "input input-sm settings-number-input",
                                    r#type: "number",
                                    min: "0",
                                    step: "10",
                                    value: "{cfg().budget.default_calls_budget}",
                                    oninput: move |e| {
                                        if let Ok(v) = e.value().parse::<u64>() {
                                            cfg.write().budget.default_calls_budget = v;
                                        }
                                    },
                                }
                                span { style: "color:var(--text-3);font-size:12px", "calls" }
                            }
                        }
                    }
                    SettingsSlider {
                        label: "Budget Window",
                        value: cfg().budget.default_window_secs as f64,
                        onchange: move |v| cfg.write().budget.default_window_secs = v as u64,
                        min: 60.0,
                        max: 86400.0,
                        step: Some(60.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("Time window for budget enforcement."),
                    }
                }
            }
        }
    }
}
