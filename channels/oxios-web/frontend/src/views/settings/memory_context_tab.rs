//! Memory & Context settings tab.

use dioxus::prelude::*;

use crate::components::settings::section_card::SectionCard;
use crate::components::settings::toggle::SettingsToggle;
use crate::components::settings::slider::SettingsSlider;
use crate::components::settings::number_input::SettingsNumberInput;
use crate::views::settings::ConfigSnapshot;

#[component]
pub fn MemoryContextTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;

    rsx! {
        div { class: "settings-tab-content",
            // Memory
            SectionCard {
                title: "Memory",
                description: Some("Vector store with embedding-based recall for agent memory."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().memory = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Memory Enabled",
                        value: cfg().memory.enabled,
                        onchange: move |v| cfg.write().memory.enabled = v,
                        description: Some("Enable the memory system for agents."),
                        dangerous: None,
                    }
                    SettingsNumberInput {
                        label: "Max Recall",
                        value: cfg().memory.max_recall as f64,
                        onchange: move |v| cfg.write().memory.max_recall = v as usize,
                        min: Some(1.0),
                        max: Some(100.0),
                        step: Some(1.0),
                        description: Some("Maximum memories returned by recall."),
                    }
                    SettingsToggle {
                        label: "Auto Summarize",
                        value: cfg().memory.auto_summarize,
                        onchange: move |v| cfg.write().memory.auto_summarize = v,
                        description: Some("Auto-summarize sessions on completion."),
                        dangerous: None,
                    }
                    SettingsToggle {
                        label: "Capture Compaction",
                        value: cfg().memory.capture_compaction,
                        onchange: move |v| cfg.write().memory.capture_compaction = v,
                        description: Some("Capture compaction summaries as conversation memory."),
                        dangerous: None,
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Retention Days" }
                            p { class: "description", "Memory retention period. 0 = unlimited." }
                        }
                        div { class: "settings-field-control",
                            div { style: "display:flex;align-items:center;gap:8px",
                                input {
                                    class: "input input-sm settings-number-input",
                                    r#type: "number",
                                    min: "0",
                                    max: "365",
                                    step: "1",
                                    value: "{cfg().memory.retention_days}",
                                    oninput: move |e| {
                                        if let Ok(v) = e.value().parse::<u32>() {
                                            cfg.write().memory.retention_days = v;
                                        }
                                    },
                                }
                                span { style: "color:var(--text-3);font-size:12px", "days" }
                            }
                        }
                    }
                    SettingsToggle {
                        label: "Embedding Cache",
                        value: cfg().memory.cache_enabled,
                        onchange: move |v| cfg.write().memory.cache_enabled = v,
                        description: Some("Enable embedding result caching."),
                        dangerous: None,
                    }
                    SettingsSlider {
                        label: "Cache TTL",
                        value: cfg().memory.cache_ttl_secs as f64,
                        onchange: move |v| cfg.write().memory.cache_ttl_secs = v as u64,
                        min: 60.0,
                        max: 86400.0,
                        step: Some(60.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("Embedding cache time-to-live."),
                    }
                    SettingsNumberInput {
                        label: "Cache Max Entries",
                        value: cfg().memory.cache_max_entries as f64,
                        onchange: move |v| cfg.write().memory.cache_max_entries = v as usize,
                        min: Some(100.0),
                        max: Some(100000.0),
                        step: Some(1000.0),
                        description: Some("Maximum embedding cache entries."),
                    }
                }
            }

            // Context
            SectionCard {
                title: "Context Manager",
                description: Some("LLM context window management — controls how much context agents can use."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().context = Default::default();
                })),
                div { class: "settings-fields",
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Active Token Limit" }
                            p { class: "description", "Maximum tokens in the active (in-context) tier. GPT-4: 128000, Claude: 200000" }
                        }
                        div { class: "settings-field-control",
                            div { style: "display:flex;align-items:center;gap:8px",
                                input {
                                    class: "input input-sm settings-number-input",
                                    r#type: "number",
                                    min: "1000",
                                    max: "1000000",
                                    step: "10000",
                                    value: "{cfg().context.active_limit_tokens}",
                                    oninput: move |e| {
                                        if let Ok(v) = e.value().parse::<usize>() {
                                            cfg.write().context.active_limit_tokens = v;
                                        }
                                    },
                                }
                                span { style: "color:var(--text-3);font-size:12px", "tokens" }
                            }
                        }
                    }
                    SettingsNumberInput {
                        label: "Cache Limit",
                        value: cfg().context.cache_limit_entries as f64,
                        onchange: move |v| cfg.write().context.cache_limit_entries = v as usize,
                        min: Some(10.0),
                        max: Some(500.0),
                        step: Some(10.0),
                        description: Some("Maximum entries in the cache tier."),
                    }
                }
            }
        }
    }
}
