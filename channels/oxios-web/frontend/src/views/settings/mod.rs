//! Oxios Settings UI — main settings view with tabbed interface.

pub mod config_types;
pub mod general_tab;
pub mod engine_tab;
pub mod exec_security_tab;
pub mod advanced_tab;

pub use config_types::ConfigSnapshot;

use dioxus::prelude::*;

use crate::api;
use crate::components::icons::*;
use crate::views::settings::general_tab::GeneralTab;
use crate::views::settings::engine_tab::EngineTab;
use crate::views::settings::exec_security_tab::ExecSecurityTab;
use crate::views::settings::advanced_tab::AdvancedTab;

// ---------------------------------------------------------------------------
// Tab definitions
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    General,
    Engine,
    ExecSecurity,
    Advanced,
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

    let tab_class = |tab: SettingsTab| -> String {
        if active_tab() == tab { "tab tab-active".to_string() } else { "tab".to_string() }
    };

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
                            save_message.set(Some((true, "Configuration saved successfully. Restart required for all changes.".to_string())));
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
                            div { class: "message-error", "{msg}" }
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

            // Tab bar
            div { class: "settings-tabs",
                button { class: "{tab_class(SettingsTab::General)}",
                    onclick: move |_| active_tab.set(SettingsTab::General),
                    IconCpu { size: 16 } " General"
                }
                button { class: "{tab_class(SettingsTab::Engine)}",
                    onclick: move |_| active_tab.set(SettingsTab::Engine),
                    IconZap { size: 16 } " Engine"
                }
                button { class: "{tab_class(SettingsTab::ExecSecurity)}",
                    onclick: move |_| active_tab.set(SettingsTab::ExecSecurity),
                    IconShield { size: 16 } " Exec & Security"
                }
                button { class: "{tab_class(SettingsTab::Advanced)}",
                    onclick: move |_| active_tab.set(SettingsTab::Advanced),
                    IconFile { size: 16 } " Advanced"
                }
            }

            // Tab content
            div { class: "tab-content",
                div { class: "settings-tab-content",
                    if active_tab() == SettingsTab::General {
                        GeneralTab { config: config_signal }
                    } else if active_tab() == SettingsTab::Engine {
                        EngineTab { config: config_signal }
                    } else if active_tab() == SettingsTab::ExecSecurity {
                        ExecSecurityTab { config: config_signal }
                    } else {
                        AdvancedTab { config: config_signal }
                    }
                }
            }
        }
    }
}