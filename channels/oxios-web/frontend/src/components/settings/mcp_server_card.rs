//! MCP Server editing card — command, args, env, enabled toggle.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::components::icons::*;
use crate::components::settings::tag_input::SettingsTagInput;
use crate::components::settings::kv_input::SettingsKeyValueInput;
use crate::views::settings::config_types::McpServerDefSnapshot;

#[component]
pub fn McpServerCard(
    name: String,
    server: McpServerDefSnapshot,
    on_change: EventHandler<(String, McpServerDefSnapshot)>,
    on_delete: EventHandler<String>,
) -> Element {
    let mut show_confirm_delete = use_signal(|| false);

    rsx! {
        div { class: "mcp-server-card",
            div { class: "mcp-server-card-header",
                span { class: "mcp-server-card-name", "{name}" }
                div { style: "display:flex;align-items:center;gap:8px",
                    button {
                        class: if server.enabled { "btn btn-sm btn-success" } else { "btn btn-sm" },
                        onclick: {
                            let n = name.clone();
                            let s = server.clone();
                            move |_| {
                                let mut updated = s.clone();
                                updated.enabled = !updated.enabled;
                                on_change.call((n.clone(), updated));
                            }
                        },
                        if server.enabled { "Enabled" } else { "Disabled" }
                    }
                    if show_confirm_delete() {
                        button {
                            class: "btn btn-sm btn-danger",
                            onclick: {
                                let n = name.clone();
                                move |_| on_delete.call(n.clone())
                            },
                            "Confirm Delete"
                        }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| show_confirm_delete.set(false),
                            "Cancel"
                        }
                    } else {
                        button {
                            class: "btn btn-sm",
                            style: "color:var(--danger)",
                            onclick: move |_| show_confirm_delete.set(true),
                            IconTrash { size: 14 }
                        }
                    }
                }
            }
            div { class: "settings-fields",
                // Command
                div { class: "settings-field",
                    div { class: "settings-field-label",
                        span { class: "settings-field-title", "Command" }
                        p { class: "description", "Executable to run (e.g. npx, python)" }
                    }
                    div { class: "settings-field-control",
                        input {
                            class: "input input-sm",
                            style: "width:100%",
                            value: "{server.command}",
                            oninput: {
                                let n = name.clone();
                                let s = server.clone();
                                move |e| {
                                    let mut updated = s.clone();
                                    updated.command = e.value();
                                    on_change.call((n.clone(), updated));
                                }
                            },
                        }
                    }
                }
                // Args
                SettingsTagInput {
                    label: "Arguments",
                    values: server.args.clone(),
                    onchange: {
                        let n = name.clone();
                        let s = server.clone();
                        move |v| {
                            let mut updated = s.clone();
                            updated.args = v;
                            on_change.call((n.clone(), updated));
                        }
                    },
                    placeholder: Some("Add argument..."),
                    description: Some("Arguments passed to the command."),
                }
                // Env
                SettingsKeyValueInput {
                    label: "Environment",
                    values: server.env.clone(),
                    onchange: {
                        let n = name.clone();
                        let s = server.clone();
                        move |v| {
                            let mut updated = s.clone();
                            updated.env = v;
                            on_change.call((n.clone(), updated));
                        }
                    },
                    placeholder: Some("KEY=VALUE"),
                    description: Some("Environment variables for the server process."),
                }
            }
        }
    }
}
