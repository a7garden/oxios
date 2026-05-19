//! Monitoring settings tab (OTel + Audit + Resource Monitor).

use dioxus::prelude::*;

use crate::components::settings::section_card::SectionCard;
use crate::components::settings::toggle::SettingsToggle;
use crate::components::settings::slider::SettingsSlider;
use crate::components::settings::number_input::SettingsNumberInput;
use crate::views::settings::ConfigSnapshot;

#[component]
pub fn MonitoringTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;

    rsx! {
        div { class: "settings-tab-content",
            // OpenTelemetry
            SectionCard {
                title: "OpenTelemetry",
                description: Some("Distributed tracing via OTLP gRPC export."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().otel = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Tracing Enabled",
                        value: cfg().otel.enabled,
                        onchange: move |v| cfg.write().otel.enabled = v,
                        description: Some("Enable OTLP trace export."),
                        dangerous: None,
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Endpoint" }
                            p { class: "description", "OTLP gRPC endpoint URL." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                placeholder: "http://localhost:4317",
                                value: "{cfg().otel.endpoint}",
                                oninput: move |e| cfg.write().otel.endpoint = e.value(),
                            }
                        }
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Service Name" }
                            p { class: "description", "Service name for traces." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                placeholder: "oxios",
                                value: "{cfg().otel.service_name}",
                                oninput: move |e| cfg.write().otel.service_name = e.value(),
                            }
                        }
                    }
                    SettingsSlider {
                        label: "Sampling Ratio",
                        value: cfg().otel.sampling_ratio,
                        onchange: move |v| cfg.write().otel.sampling_ratio = v,
                        min: 0.0,
                        max: 1.0,
                        step: Some(0.1),
                        unit: None,
                        show_max: None,
                        description: Some("Trace sampling ratio (0.0 = none, 1.0 = all)."),
                    }
                }
            }

            // Audit Trail
            SectionCard {
                title: "Audit Trail",
                description: Some("Merkle-chain style tamper-evident audit logging."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().audit = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Audit Enabled",
                        value: cfg().audit.enabled,
                        onchange: move |v| cfg.write().audit.enabled = v,
                        description: Some("Enable the audit trail system."),
                        dangerous: None,
                    }
                    SettingsNumberInput {
                        label: "Max Entries",
                        value: cfg().audit.max_entries as f64,
                        onchange: move |v| cfg.write().audit.max_entries = v as usize,
                        min: Some(100.0),
                        max: Some(1_000_000.0),
                        step: Some(10000.0),
                        description: Some("Maximum audit entries before pruning."),
                    }
                }
            }

            // Resource Monitor
            SectionCard {
                title: "Resource Monitor",
                description: Some("System resource tracking for agent budget enforcement."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().resource_monitor = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsSlider {
                        label: "Snapshot Interval",
                        value: cfg().resource_monitor.interval_secs as f64,
                        onchange: move |v| cfg.write().resource_monitor.interval_secs = v as u64,
                        min: 10.0,
                        max: 300.0,
                        step: Some(10.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("How often to sample system resources."),
                    }
                    SettingsNumberInput {
                        label: "History Max",
                        value: cfg().resource_monitor.history_max as f64,
                        onchange: move |v| cfg.write().resource_monitor.history_max = v as usize,
                        min: Some(10.0),
                        max: Some(500.0),
                        step: Some(10.0),
                        description: Some("Maximum history entries to retain."),
                    }
                    SettingsSlider {
                        label: "CPU Threshold",
                        value: cfg().resource_monitor.cpu_threshold as f64,
                        onchange: move |v| cfg.write().resource_monitor.cpu_threshold = v as f32,
                        min: 50.0,
                        max: 100.0,
                        step: Some(5.0),
                        unit: Some("%"),
                        show_max: None,
                        description: Some("CPU usage threshold for overload detection."),
                    }
                    SettingsSlider {
                        label: "Memory Threshold",
                        value: cfg().resource_monitor.memory_threshold as f64,
                        onchange: move |v| cfg.write().resource_monitor.memory_threshold = v as f32,
                        min: 50.0,
                        max: 100.0,
                        step: Some(5.0),
                        unit: Some("%"),
                        show_max: None,
                        description: Some("Memory usage threshold for overload detection."),
                    }
                    SettingsNumberInput {
                        label: "Load Average Threshold",
                        value: cfg().resource_monitor.load_threshold as f64,
                        onchange: move |v| cfg.write().resource_monitor.load_threshold = v as f32,
                        min: Some(1.0),
                        max: Some(64.0),
                        step: Some(0.5),
                        description: Some("Load average threshold for overload detection."),
                    }
                }
            }
        }
    }
}
