//! Integrations settings tab (channels + MCP servers + cron + browser).

use dioxus::prelude::*;

use crate::components::icons::IconPlus;
use crate::components::settings::section_card::SectionCard;
use crate::components::settings::toggle::SettingsToggle;
use crate::components::settings::slider::SettingsSlider;
use crate::components::settings::tag_input::SettingsTagInput;
use crate::components::settings::multi_checkbox::SettingsMultiCheckbox;
use crate::components::settings::mcp_server_card::McpServerCard;
use crate::components::settings::SelectOption;
use crate::views::settings::config_types::{ConfigSnapshot, McpServerDefSnapshot};

const CHANNEL_OPTIONS: &[SelectOption] = &[
    SelectOption { value: "web", label: "Web" },
    SelectOption { value: "cli", label: "CLI" },
    SelectOption { value: "telegram", label: "Telegram" },
];

#[component]
pub fn IntegrationsTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;
    let mut show_add_server = use_signal(|| false);
    let mut new_server_name = use_signal(String::new);
    let mut add_error = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "settings-tab-content",
            // Channels
            SectionCard {
                title: "Channels",
                description: Some("Activate communication channels on startup."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().channels = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsMultiCheckbox {
                        label: "Enabled Channels",
                        options: CHANNEL_OPTIONS.to_vec(),
                        selected: cfg().channels.enabled.clone(),
                        onchange: move |v| cfg.write().channels.enabled = v,
                        description: Some("Which channels to start on boot."),
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Bot Token Env" }
                            p { class: "description", "Environment variable name holding the Telegram bot token." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                placeholder: "TELEGRAM_BOT_TOKEN",
                                value: "{cfg().channels.telegram.bot_token_env}",
                                oninput: move |e| cfg.write().channels.telegram.bot_token_env = e.value(),
                            }
                        }
                    }
                    SettingsTagInput {
                        label: "Allowed Telegram Users",
                        values: cfg().channels.telegram.allowed_users.iter().map(|id| id.to_string()).collect(),
                        onchange: move |v| {
                            cfg.write().channels.telegram.allowed_users = v.iter()
                                .filter_map(|s| s.parse::<i64>().ok())
                                .collect();
                        },
                        placeholder: Some("User ID..."),
                        description: Some("Telegram user IDs allowed to interact. Empty = allow all."),
                    }
                }
            }

            // MCP Servers
            SectionCard {
                title: "MCP Servers",
                description: Some("Model Context Protocol servers for tool integration."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().mcp = Default::default();
                })),
                div { class: "settings-fields",
                    {
                        let servers: Vec<(String, McpServerDefSnapshot)> = cfg().mcp.servers.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let server_elements: Vec<Element> = servers.into_iter().map(|(name, server)| {
                            let name_clone = name.clone();
                            rsx! {
                                McpServerCard {
                                    key: "{name}",
                                    name: name,
                                    server: server,
                                    on_change: move |(n, s): (String, McpServerDefSnapshot)| {
                                        cfg.write().mcp.servers.insert(n, s);
                                    },
                                    on_delete: move |n: String| {
                                        cfg.write().mcp.servers.remove(&n);
                                    },
                                }
                            }
                        }).collect();
                        server_elements.into_iter()
                    }

                    // Add server button / inline form
                    if show_add_server() {
                        div { class: "mcp-server-card",
                            div { style: "display:flex;align-items:center;gap:8px",
                                input {
                                    class: "input input-sm",
                                    style: "flex:1",
                                    placeholder: "Server name (e.g. filesystem)",
                                    value: "{new_server_name}",
                                    oninput: move |e| new_server_name.set(e.value()),
                                    onkeydown: move |e: KeyboardEvent| {
                                        if e.key().to_string() == "Enter" {
                                            let name = new_server_name().trim().to_string();
                                            if name.is_empty() {
                                                add_error.set(Some("Name cannot be empty".to_string()));
                                            } else if cfg().mcp.servers.contains_key(&name) {
                                                add_error.set(Some(format!("Server '{name}' already exists")));
                                            } else {
                                                cfg.write().mcp.servers.insert(name, McpServerDefSnapshot::default());
                                                new_server_name.set(String::new());
                                                show_add_server.set(false);
                                                add_error.set(None);
                                            }
                                        }
                                    },
                                }
                                button {
                                    class: "btn btn-primary btn-sm",
                                    onclick: move |_| {
                                        let name = new_server_name().trim().to_string();
                                        if name.is_empty() {
                                            add_error.set(Some("Name cannot be empty".to_string()));
                                        } else if cfg().mcp.servers.contains_key(&name) {
                                            add_error.set(Some(format!("Server '{name}' already exists")));
                                        } else {
                                            cfg.write().mcp.servers.insert(name, McpServerDefSnapshot::default());
                                            new_server_name.set(String::new());
                                            show_add_server.set(false);
                                            add_error.set(None);
                                        }
                                    },
                                    "Add"
                                }
                                button {
                                    class: "btn btn-sm",
                                    onclick: move |_| {
                                        show_add_server.set(false);
                                        add_error.set(None);
                                    },
                                    "Cancel"
                                }
                            }
                            if let Some(err) = add_error() {
                                p { class: "description error-text", style: "margin-top:8px", "{err}" }
                            }
                        }
                    } else {
                        button {
                            class: "btn btn-sm",
                            style: "margin-top:8px",
                            onclick: move |_| show_add_server.set(true),
                            IconPlus { size: 14 } " Add MCP Server"
                        }
                    }
                }
            }

            // Cron
            SectionCard {
                title: "Cron Scheduler",
                description: Some("Scheduled job execution with persistent state."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().cron = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Cron Enabled",
                        value: cfg().cron.enabled,
                        onchange: move |v| cfg.write().cron.enabled = v,
                        description: Some("Enable the cron scheduler for periodic tasks."),
                        dangerous: None,
                    }
                    SettingsSlider {
                        label: "Tick Interval",
                        value: cfg().cron.tick_interval_secs as f64,
                        onchange: move |v| cfg.write().cron.tick_interval_secs = v as u64,
                        min: 10.0,
                        max: 600.0,
                        step: Some(10.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("How often the cron scheduler checks for pending jobs."),
                    }
                }
            }

            // Browser
            SectionCard {
                title: "Headless Browser",
                description: Some("Built-in browser integration for web browsing tasks."),
                on_reset: Some(EventHandler::new(move |_: ()| {
                    cfg.write().browser = Default::default();
                })),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Browser Enabled",
                        value: cfg().browser.enabled,
                        onchange: move |v| cfg.write().browser.enabled = v,
                        description: Some("Enable headless browser integration."),
                        dangerous: None,
                    }
                }
            }
        }
    }
}
