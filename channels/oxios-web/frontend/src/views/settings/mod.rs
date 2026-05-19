//! Oxios Settings UI — main settings view with tabbed interface.

pub mod config_types;
pub mod general_tab;
pub mod engine_tab;
pub mod exec_security_tab;
pub mod agents_tab;
pub mod memory_context_tab;
pub mod integrations_tab;
pub mod monitoring_tab;
pub mod advanced_tab;

pub use config_types::ConfigSnapshot;

use dioxus::prelude::*;

use crate::api;
use crate::components::icons::*;
use crate::views::settings::general_tab::GeneralTab;
use crate::views::settings::engine_tab::EngineTab;
use crate::views::settings::exec_security_tab::ExecSecurityTab;
use crate::views::settings::agents_tab::AgentsTab;
use crate::views::settings::memory_context_tab::MemoryContextTab;
use crate::views::settings::integrations_tab::IntegrationsTab;
use crate::views::settings::monitoring_tab::MonitoringTab;
use crate::views::settings::advanced_tab::AdvancedTab;

// ---------------------------------------------------------------------------
// Tab definitions
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    General,
    Engine,
    ExecSecurity,
    Agents,
    MemoryContext,
    Integrations,
    Monitoring,
    Advanced,
}

/// Check if a specific tab has unsaved changes by comparing section subsets.
fn is_tab_dirty(tab: SettingsTab, current: &ConfigSnapshot, original: &ConfigSnapshot) -> bool {
    match tab {
        SettingsTab::General => {
            serde_json::to_string(&current.kernel).ok() != serde_json::to_string(&original.kernel).ok()
                || serde_json::to_string(&current.daemon).ok() != serde_json::to_string(&original.daemon).ok()
                || serde_json::to_string(&current.gateway).ok() != serde_json::to_string(&original.gateway).ok()
                || serde_json::to_string(&current.git).ok() != serde_json::to_string(&original.git).ok()
        }
        SettingsTab::Engine => {
            let cur_model = &current.engine.default_model;
            let orig_model = &original.engine.default_model;
            cur_model != orig_model
                // api_key change is tracked in the engine tab
                || current.engine.api_key.is_some() != original.engine.api_key.is_some()
        }
        SettingsTab::ExecSecurity => {
            serde_json::to_string(&current.exec).ok() != serde_json::to_string(&original.exec).ok()
                || serde_json::to_string(&current.security).ok() != serde_json::to_string(&original.security).ok()
        }
        SettingsTab::Agents => {
            serde_json::to_string(&current.scheduler).ok() != serde_json::to_string(&original.scheduler).ok()
                || serde_json::to_string(&current.orchestrator).ok() != serde_json::to_string(&original.orchestrator).ok()
                || serde_json::to_string(&current.persona).ok() != serde_json::to_string(&original.persona).ok()
                || serde_json::to_string(&current.budget).ok() != serde_json::to_string(&original.budget).ok()
        }
        SettingsTab::MemoryContext => {
            serde_json::to_string(&current.memory).ok() != serde_json::to_string(&original.memory).ok()
                || serde_json::to_string(&current.context).ok() != serde_json::to_string(&original.context).ok()
        }
        SettingsTab::Integrations => {
            serde_json::to_string(&current.channels).ok() != serde_json::to_string(&original.channels).ok()
                || serde_json::to_string(&current.mcp).ok() != serde_json::to_string(&original.mcp).ok()
                || serde_json::to_string(&current.cron).ok() != serde_json::to_string(&original.cron).ok()
                || serde_json::to_string(&current.browser).ok() != serde_json::to_string(&original.browser).ok()
        }
        SettingsTab::Monitoring => {
            serde_json::to_string(&current.otel).ok() != serde_json::to_string(&original.otel).ok()
                || serde_json::to_string(&current.audit).ok() != serde_json::to_string(&original.audit).ok()
                || serde_json::to_string(&current.resource_monitor).ok() != serde_json::to_string(&original.resource_monitor).ok()
        }
        SettingsTab::Advanced => false,
    }
}

// ---------------------------------------------------------------------------
// Main Settings View
// ---------------------------------------------------------------------------

#[component]
pub fn SettingsView() -> Element {
    let mut config_resource = use_resource(|| async move {
        api::fetch_json::<ConfigSnapshot>("/api/config").await
    });

    // Load from resource into local signals
    let mut config_signal = use_signal(|| ConfigSnapshot::default());
    let mut original_signal = use_signal(|| ConfigSnapshot::default());
    let mut is_loaded = use_signal(|| false);

    let resource_val = &(config_resource.value())();
    if let Some(Ok(cfg)) = resource_val {
        if !is_loaded() {
            config_signal.set(cfg.clone());
            original_signal.set(cfg.clone());
            is_loaded.set(true);
        }
    }

    let mut active_tab = use_signal(|| SettingsTab::General);
    let mut save_message = use_signal(|| Option::<(bool, String)>::None);

    // Derive dirty from config vs original
    let current_cfg = config_signal();
    let original_cfg = original_signal();
    let is_dirty = serde_json::to_string(&current_cfg).ok()
        != serde_json::to_string(&original_cfg).ok();

    let reload = move |_| {
        is_loaded.set(false);
        config_resource.restart();
    };

    let do_save = move |_| {
        let cfg_val = config_signal();
        save_message.set(None);
        spawn(async move {
            let mut json_val = serde_json::to_value(&cfg_val).ok();
            // Omit empty api_key from PUT body
            if let Some(ref mut j) = json_val {
                if j.get("engine")
                    .and_then(|e| e.get("api_key"))
                    .and_then(|k| k.as_str())
                    .map(|s| s.is_empty())
                    .unwrap_or(false)
                {
                    if let Some(engine) = j.get_mut("engine") {
                        engine.as_object_mut().map(|m| { m.remove("api_key"); });
                    }
                }
            }
            match json_val {
                Some(body) => {
                    match api::put_json::<serde_json::Value, _>("/api/config", &body).await {
                        Ok(_) => {
                            save_message.set(Some((true, "Configuration saved. Restart Oxios to apply changes.".to_string())));
                            original_signal.set(cfg_val);
                        }
                        Err(e) => save_message.set(Some((false, format!("Save error: {e}")))),
                    }
                }
                None => save_message.set(Some((false, "Serialization error".to_string()))),
            }
        });
    };

    if !is_loaded() {
        return rsx! {
            div { class: "panel-container",
                div { class: "panel-header",
                    h2 { IconSettings { size: 20 } " Settings" }
                }
                div { class: "panel-body",
                    div { class: "empty-state",
                        div { class: "empty-icon", IconLoading { size: 40 } }
                        p { "Loading configuration..." }
                    }
                }
            }
        };
    }

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconSettings { size: 20 } " Settings" }
                div { style: "display:flex;align-items:center;gap:12px",
                    if let Some((success, msg)) = save_message() {
                        if success {
                            div { class: "restart-banner",
                                IconAlertTriangle { size: 16 }
                                span { "{msg}" }
                            }
                        } else {
                            div { class: "message message-error", "{msg}" }
                        }
                    }
                    button {
                        class: if is_dirty { "btn btn-primary btn-sm" } else { "btn btn-sm" },
                        disabled: !is_dirty,
                        onclick: do_save,
                        IconSave { size: 14 } " Save Changes"
                    }
                    button { class: "btn btn-sm", onclick: reload, "Refresh" }
                }
            }

            // Tab bar with dirty badges
            div { class: "settings-tabs",
                {render_tab_button(SettingsTab::General, "General", IconCpu, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::Engine, "Engine", IconZap, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::ExecSecurity, "Exec & Security", IconShield, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::Agents, "Agents", IconAgents, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::MemoryContext, "Memory & Context", IconMemory, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::Integrations, "Integrations", IconLayers, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::Monitoring, "Monitoring", IconActivity, active_tab, &current_cfg, &original_cfg)}
                {render_tab_button(SettingsTab::Advanced, "Advanced", IconFile, active_tab, &current_cfg, &original_cfg)}
            }

            // Tab content
            div { class: "tab-content",
                div { class: "settings-tab-content",
                    {
                        let tab = active_tab();
                        match tab {
                            SettingsTab::General => rsx! { GeneralTab { config: config_signal } },
                            SettingsTab::Engine => rsx! { EngineTab { config: config_signal } },
                            SettingsTab::ExecSecurity => rsx! { ExecSecurityTab { config: config_signal } },
                            SettingsTab::Agents => rsx! { AgentsTab { config: config_signal } },
                            SettingsTab::MemoryContext => rsx! { MemoryContextTab { config: config_signal } },
                            SettingsTab::Integrations => rsx! { IntegrationsTab { config: config_signal } },
                            SettingsTab::Monitoring => rsx! { MonitoringTab { config: config_signal } },
                            SettingsTab::Advanced => rsx! { AdvancedTab { config: config_signal } },
                        }
                    }
                }
            }
        }
    }
}

/// Render a tab button with icon and optional dirty dot badge.
fn render_tab_button(
    tab: SettingsTab,
    label: &str,
    icon: fn(class: Option<String>, size: Option<u32>) -> Element,
    mut active_tab: Signal<SettingsTab>,
    current: &ConfigSnapshot,
    original: &ConfigSnapshot,
) -> Element {
    let is_active = active_tab() == tab;
    let is_dirty = is_tab_dirty(tab, current, original);
    let class = if is_active { "tab tab-active" } else { "tab" };
    let dirty_class = if is_dirty { "tab-dirty" } else { "" };

    rsx! {
        button {
            class: "{class} {dirty_class}",
            onclick: move |_| active_tab.set(tab),
            {icon(None, Some(16))}
            " {label}"
        }
    }
}
