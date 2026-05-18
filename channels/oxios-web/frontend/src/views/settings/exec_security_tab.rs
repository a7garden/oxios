//! Execution & Security settings tab.

use dioxus::prelude::*;

use crate::components::settings::section_card::SectionCard;
use crate::components::settings::toggle::SettingsToggle;
use crate::components::settings::slider::SettingsSlider;
use crate::components::settings::number_input::SettingsNumberInput;
use crate::components::settings::tag_input::SettingsTagInput;
use crate::components::settings::multi_checkbox::SettingsMultiCheckbox;
use crate::components::settings::select_group::SettingsSelectGroup;
use crate::components::settings::SelectOption;
use crate::views::settings::ConfigSnapshot;

const EXEC_MODE_OPTIONS: &[SelectOption] = &[
    SelectOption { value: "structured", label: "Structured" },
    SelectOption { value: "shell", label: "Shell" },
];

const TOOL_OPTIONS: &[SelectOption] = &[
    SelectOption { value: "read", label: "Read" },
    SelectOption { value: "write", label: "Write" },
    SelectOption { value: "edit", label: "Edit" },
    SelectOption { value: "bash", label: "Bash" },
    SelectOption { value: "grep", label: "Grep" },
    SelectOption { value: "find", label: "Find" },
    SelectOption { value: "web", label: "Web" },
    SelectOption { value: "mcp", label: "MCP" },
];

#[component]
pub fn ExecSecurityTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut cfg = config;

    rsx! {
        div { class: "settings-tab-content",
            SectionCard {
                title: "Execution",
                description: Some("How the kernel dispatches commands to the host."),
                div { class: "settings-fields",
                    SettingsSelectGroup {
                        label: "Default Mode",
                        options: EXEC_MODE_OPTIONS.to_vec(),
                        selected: cfg().exec.default_mode.clone(),
                        onchange: move |v: String| cfg.write().exec.default_mode = v,
                        description: Some("Structured: binary allowlist + metacharacter blocking. Shell: raw bash -c (dangerous)."),
                    }
                    SettingsToggle {
                        label: "Allow Shell Mode",
                        value: cfg().exec.allow_shell_mode,
                        onchange: move |v| cfg.write().exec.allow_shell_mode = v,
                        description: Some("Allow raw bash -c execution. DANGEROUS in production."),
                        dangerous: Some(true),
                    }
                    SettingsTagInput {
                        label: "Allowed Commands",
                        values: cfg().exec.allowed_commands.clone(),
                        onchange: move |v| cfg.write().exec.allowed_commands = v,
                        placeholder: Some("Command name..."),
                        description: Some("Commands agents may execute. Empty = all commands allowed."),
                    }
                    SettingsSlider {
                        label: "Default Timeout",
                        value: cfg().exec.default_timeout_secs as f64,
                        onchange: move |v| cfg.write().exec.default_timeout_secs = v as u64,
                        min: 10.0,
                        max: 600.0,
                        step: Some(10.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("Default execution timeout per call."),
                    }
                    SettingsNumberInput {
                        label: "Max Timeout",
                        value: cfg().exec.max_timeout_secs as f64,
                        onchange: move |v| cfg.write().exec.max_timeout_secs = v as u64,
                        min: Some(30.0),
                        max: Some(3600.0),
                        step: Some(30.0),
                        unit: Some("sec"),
                        description: Some("Maximum allowed timeout for a single exec call."),
                    }
                }
            }

            SectionCard {
                title: "Security",
                description: Some("Access control, resource limits, and audit configuration."),
                div { class: "settings-fields",
                    SettingsToggle {
                        label: "Auth Enabled",
                        value: cfg().security.auth_enabled,
                        onchange: move |v| cfg.write().security.auth_enabled = v,
                        description: Some("Require API key authentication for all endpoints."),
                        dangerous: None,
                    }
                    SettingsToggle {
                        label: "Network Access",
                        value: cfg().security.network_access,
                        onchange: move |v| cfg.write().security.network_access = v,
                        description: Some("Allow agents to make outbound network requests."),
                        dangerous: Some(true),
                    }
                    SettingsToggle {
                        label: "Can Fork",
                        value: cfg().security.can_fork,
                        onchange: move |v| cfg.write().security.can_fork = v,
                        description: Some("Allow agents to fork sub-agents."),
                        dangerous: Some(true),
                    }
                    SettingsMultiCheckbox {
                        label: "Allowed Tools",
                        options: TOOL_OPTIONS.to_vec(),
                        selected: cfg().security.allowed_tools.clone(),
                        onchange: move |v| cfg.write().security.allowed_tools = v,
                        description: Some("Tools agents are permitted to use by default."),
                    }
                    SettingsSlider {
                        label: "Max Execution Time",
                        value: cfg().security.max_execution_time_secs as f64,
                        onchange: move |v| cfg.write().security.max_execution_time_secs = v as u64,
                        min: 30.0,
                        max: 3600.0,
                        step: Some(30.0),
                        unit: Some("sec"),
                        show_max: None,
                        description: Some("Maximum time an agent task can run."),
                    }
                    SettingsSlider {
                        label: "Max Memory",
                        value: cfg().security.max_memory_mb as f64,
                        onchange: move |v| cfg.write().security.max_memory_mb = v as u64,
                        min: 128.0,
                        max: 4096.0,
                        step: Some(128.0),
                        unit: Some("MB"),
                        show_max: None,
                        description: Some("Memory cap per agent task."),
                    }
                    SettingsTagInput {
                        label: "CORS Origins",
                        values: cfg().security.cors_origins.clone(),
                        onchange: move |v| cfg.write().security.cors_origins = v,
                        placeholder: Some("https://example.com"),
                        description: Some("Allowed CORS origins for the web gateway."),
                    }
                    SettingsSlider {
                        label: "Rate Limit",
                        value: cfg().security.rate_limit_per_minute as f64,
                        onchange: move |v| cfg.write().security.rate_limit_per_minute = v as u32,
                        min: 10.0,
                        max: 1000.0,
                        step: Some(10.0),
                        unit: Some("/min"),
                        show_max: None,
                        description: Some("API rate limit (requests per minute)."),
                    }
                    SettingsNumberInput {
                        label: "Max Audit Entries",
                        value: cfg().security.max_audit_entries as f64,
                        onchange: move |v| cfg.write().security.max_audit_entries = v as usize,
                        min: Some(100.0),
                        max: Some(1_000_000.0),
                        step: Some(1000.0),
                        description: Some("Maximum audit log entries to retain."),
                    }
                    div { class: "settings-field",
                        div { class: "settings-field-label",
                            span { class: "settings-field-title", "Audit Log Path" }
                            p { class: "description", "File path for audit log. Leave empty to disable file logging." }
                        }
                        div { class: "settings-field-control",
                            input {
                                class: "input input-sm",
                                style: "width:100%",
                                placeholder: "/var/log/oxios/audit.jsonl",
                                value: "{cfg().security.audit_log_path.as_deref().unwrap_or(\"\")}",
                                oninput: move |e| {
                                    let v = e.value();
                                    cfg.write().security.audit_log_path = if v.is_empty() { None } else { Some(v) };
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}