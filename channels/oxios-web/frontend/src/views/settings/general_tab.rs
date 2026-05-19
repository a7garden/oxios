//! General settings tab (kernel + daemon + gateway + git).

use dioxus::prelude::*;

use crate::components::settings::section_card::SectionCard;
use crate::components::settings::number_input::SettingsNumberInput;
use crate::components::settings::toggle::SettingsToggle;
use crate::views::settings::ConfigSnapshot;

#[component]
pub fn GeneralTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;

    rsx! {
        div { class: "settings-tab-content",
            SectionCard {
                title: "Kernel",
                description: Some("Core runtime settings for the Oxios kernel."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().kernel = Default::default();
                })),
                div { class: "settings-fields",
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Workspace" }
                            p { class: "description", "Directory where agents operate." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                value: "{cfg().kernel.workspace}",
                                oninput: move |e| cfg.write().kernel.workspace = e.value(),
                            }
                        }
                    }
                    SettingsNumberInput {
                        label: "Max Agents",
                        value: cfg().kernel.max_agents as f64,
                        onchange: move |v| cfg.write().kernel.max_agents = v as usize,
                        min: Some(1.0),
                        max: Some(64.0),
                        step: Some(1.0),
                        description: Some("Maximum concurrent agents."),
                    }
                    SettingsNumberInput {
                        label: "Event Bus Capacity",
                        value: cfg().kernel.event_bus_capacity as f64,
                        onchange: move |v| cfg.write().kernel.event_bus_capacity = v as usize,
                        min: Some(16.0),
                        max: Some(4096.0),
                        step: Some(16.0),
                        description: Some("Broadcast channel capacity."),
                    }
                }
            }

            SectionCard {
                title: "Gateway",
                description: Some("HTTP gateway binding settings."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().gateway = Default::default();
                })),
                div { class: "settings-fields",
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Host" }
                            p { class: "description", "Address to bind the gateway to." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                value: "{cfg().gateway.host}",
                                oninput: move |e| cfg.write().gateway.host = e.value(),
                            }
                        }
                    }
                    SettingsNumberInput {
                        label: "Port",
                        value: cfg().gateway.port as f64,
                        onchange: move |v| cfg.write().gateway.port = v as u16,
                        min: Some(1.0),
                        max: Some(65535.0),
                        step: Some(1.0),
                        description: Some("Port for the web gateway."),
                    }
                }
            }

            SectionCard {
                title: "Daemon",
                description: Some("Daemon mode PID and log settings."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().daemon = Default::default();
                })),
                div { class: "settings-fields",
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "PID File" }
                            p { class: "description", "Path for the daemon PID file." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                value: "{cfg().daemon.pid_file}",
                                oninput: move |e| cfg.write().daemon.pid_file = e.value(),
                            }
                        }
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Log Directory" }
                            p { class: "description", "Directory for daemon log files." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                value: "{cfg().daemon.log_dir}",
                                oninput: move |e| cfg.write().daemon.log_dir = e.value(),
                            }
                        }
                    }
                }
            }

            SectionCard {
                title: "Version Control",
                description: Some("Git integration settings."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().git = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Auto Commit",
                        value: cfg().git.auto_commit,
                        onchange: move |v| cfg.write().git.auto_commit = v,
                        description: Some("Enable automatic git commits for state changes."),
                        dangerous: None,
                    }
                }
            }
        }
    }
}
