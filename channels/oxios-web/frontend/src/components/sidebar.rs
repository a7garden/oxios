//! Sidebar navigation with section grouping, theme toggle, API key, and mobile support.

use dioxus::prelude::*;

use crate::api;
use crate::{local_storage_remove, local_storage_set, Theme};
use crate::components::icons::*;

/// Top-level navigation panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Dashboard,
    Config,
    Agents,
    AgentGroups,
    Personas,
    Budget,
    Scheduler,
    Protocol,
    Seeds,
    Workspace,
    Skills,
    Programs,
    CronJobs,
    Git,
    Memory,
    HostTools,
    Security,
    Approvals,
    Events,
    Sessions,
    Resources,
}

/// Section label for grouping navigation items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Core,
    Agents,
    Ouroboros,
    System,
    Security,
    Monitor,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::Core      => "Core",
            Section::Agents    => "Agents",
            Section::Ouroboros => "Ouroboros",
            Section::System    => "System",
            Section::Security  => "Security",
            Section::Monitor   => "Monitor",
        }
    }
}

/// Navigation item definition.
struct NavItem {
    panel: Panel,
    label: &'static str,
    section: Section,
}

const NAV_ITEMS: &[NavItem] = &[
    // Core
    NavItem { panel: Panel::Chat,      label: "Chat",      section: Section::Core },
    NavItem { panel: Panel::Dashboard, label: "Dashboard",  section: Section::Core },
    NavItem { panel: Panel::Config,    label: "Config",     section: Section::Core },
    // Agents
    NavItem { panel: Panel::Agents,     label: "Agents",       section: Section::Agents },
    NavItem { panel: Panel::AgentGroups, label: "Agent Groups", section: Section::Agents },
    NavItem { panel: Panel::Personas,   label: "Personas",    section: Section::Agents },
    NavItem { panel: Panel::Budget,     label: "Budget",      section: Section::Agents },
    NavItem { panel: Panel::Scheduler,  label: "Scheduler",   section: Section::Agents },
    // Ouroboros
    NavItem { panel: Panel::Protocol,  label: "Protocol",   section: Section::Ouroboros },
    NavItem { panel: Panel::Seeds,     label: "Seeds",      section: Section::Ouroboros },
    // System
    NavItem { panel: Panel::Workspace, label: "Workspace",  section: Section::System },
    NavItem { panel: Panel::Skills,    label: "Skills",     section: Section::System },
    NavItem { panel: Panel::Programs,  label: "Programs",   section: Section::System },
    NavItem { panel: Panel::CronJobs,  label: "Cron Jobs",  section: Section::System },
    NavItem { panel: Panel::Git,       label: "Git",        section: Section::System },
    NavItem { panel: Panel::Memory,    label: "Memory",     section: Section::System },
    NavItem { panel: Panel::HostTools, label: "Host Tools", section: Section::System },
    // Security
    NavItem { panel: Panel::Security,  label: "Security",   section: Section::Security },
    NavItem { panel: Panel::Approvals, label: "Approvals",  section: Section::Security },
    // Monitor
    NavItem { panel: Panel::Events,    label: "Events",     section: Section::Monitor },
    NavItem { panel: Panel::Sessions,  label: "Sessions",   section: Section::Monitor },
    NavItem { panel: Panel::Resources, label: "Resources",  section: Section::Monitor },
];

/// Render the icon for a given panel.
fn panel_icon(panel: Panel) -> Element {
    match panel {
        Panel::Chat       => rsx! { IconChat { size: 18 } },
        Panel::Dashboard  => rsx! { IconDashboard { size: 18 } },
        Panel::Config     => rsx! { IconSettings { size: 18 } },
        Panel::Agents     => rsx! { IconAgents { size: 18 } },
        Panel::AgentGroups => rsx! { IconLayers { size: 18 } },
        Panel::Personas   => rsx! { IconUsers { size: 18 } },
        Panel::Budget     => rsx! { IconDollarSign { size: 18 } },
        Panel::Scheduler  => rsx! { IconClock { size: 18 } },
        Panel::Protocol   => rsx! { IconProtocol { size: 18 } },
        Panel::Seeds      => rsx! { IconSeeds { size: 18 } },
        Panel::Workspace  => rsx! { IconFolder { size: 18 } },
        Panel::Skills     => rsx! { IconSkills { size: 18 } },
        Panel::Programs   => rsx! { IconPackage { size: 18 } },
        Panel::CronJobs   => rsx! { IconClock { size: 18 } },
        Panel::Git        => rsx! { IconGit { size: 18 } },
        Panel::Memory     => rsx! { IconMemory { size: 18 } },
        Panel::HostTools  => rsx! { IconWrench { size: 18 } },
        Panel::Security   => rsx! { IconShield { size: 18 } },
        Panel::Approvals  => rsx! { IconCheckSquare { size: 18 } },
        Panel::Events     => rsx! { IconActivity { size: 18 } },
        Panel::Sessions   => rsx! { IconDatabase { size: 18 } },
        Panel::Resources  => rsx! { IconCpu { size: 18 } },
    }
}

#[component]
pub fn Sidebar() -> Element {
    let mut panel = use_context::<Signal<Panel>>();
    let mut theme = use_context::<Signal<Theme>>();
    let mut mobile_menu = use_context::<Signal<bool>>();
    let mut collapsed = use_signal(|| false);
    let mut show_api_key = use_signal(|| false);
    let mut api_key_input = use_signal(String::new);
    let mut api_key_saved = use_signal(|| false);

    let collapsed_val = collapsed();
    let is_dark = theme() == Theme::Dark;
    let has_api_key = api::auth_token().is_some();

    let sidebar_class = if collapsed_val { "sidebar collapsed" } else { "sidebar" };

    // Build navigation with section headers
    let mut nav_elements: Vec<Element> = Vec::new();
    let mut current_section: Option<Section> = None;

    for item in NAV_ITEMS {
        // Insert section header when section changes
        if current_section != Some(item.section) && !collapsed_val {
            current_section = Some(item.section);
            let section_label = item.section.label();
            nav_elements.push(rsx! {
                div { class: "nav-section", key: "section-{section_label}",
                    span { class: "nav-section-label", "{section_label}" }
                }
            });
        } else if current_section != Some(item.section) {
            current_section = Some(item.section);
        }

        let is_active = panel() == item.panel;
        let p_val = item.panel;
        let label = item.label;
        let active_class = if is_active { "nav-item active" } else { "nav-item" };

        nav_elements.push(rsx! {
            button {
                class: "{active_class}",
                key: "{label}",
                onclick: move |_| {
                    panel.set(p_val);
                    mobile_menu.set(false);
                },
                span { class: "nav-icon", {panel_icon(p_val)} }
                if !collapsed_val {
                    span { class: "nav-label", "{label}" }
                }
            }
        });
    }

    rsx! {
        aside { class: "{sidebar_class}",
            div { class: "sidebar-header",
                if !collapsed_val {
                    span { class: "sidebar-brand", "OXIOS" }
                }
                div { class: "sidebar-header-actions",
                    // Theme toggle
                    button {
                        class: "icon-btn",
                        title: if is_dark { "Switch to light" } else { "Switch to dark" },
                        onclick: move |_| {
                            theme.set(if theme() == Theme::Dark { Theme::Light } else { Theme::Dark });
                        },
                        {if is_dark {
                            rsx! { IconSun { size: 16 } }
                        } else {
                            rsx! { IconMoon { size: 16 } }
                        }}
                    }
                    // Collapse toggle (desktop only)
                    button {
                        class: "icon-btn collapse-toggle",
                        onclick: move |_| collapsed.toggle(),
                        {if collapsed_val {
                            rsx! { IconChevronRight { size: 16 } }
                        } else {
                            rsx! { IconChevronLeft { size: 16 } }
                        }}
                    }
                }
            }
            nav { class: "sidebar-nav",
                {nav_elements.into_iter()}
            }
            // API Key section at bottom
            if !collapsed_val {
                div { class: "sidebar-footer",
                    button {
                        class: "btn btn-sm btn-block",
                        style: "margin-bottom:4px",
                        onclick: move |_| show_api_key.set(true),
                        if has_api_key {
                            "🔑 API Key Set"
                        } else {
                            "🔑 Set API Key"
                        }
                    }
                }
            }
            if show_api_key() {
                div { class: "modal-overlay", onclick: move |_| show_api_key.set(false),
                    div { class: "modal", style: "max-width:400px", onclick: move |e| e.stop_propagation(),
                        div { class: "modal-header",
                            h3 { "API Key" }
                        }
                        div { class: "modal-body",
                            p { style: "font-size:13px;color:var(--text-secondary);margin-bottom:12px",
                                "Enter the API key for authenticating with the Oxios backend."
                            }
                            input {
                                r#type: "password",
                                class: "form-input",
                                placeholder: "API key...",
                                value: "{api_key_input}",
                                oninput: move |e| {
                                    api_key_input.set(e.value());
                                    api_key_saved.set(false);
                                },
                            }
                            if api_key_saved() {
                                p { style: "font-size:12px;color:var(--green);margin-top:8px", "✓ Saved" }
                            }
                        }
                        div { class: "modal-footer",
                            button { class: "btn", onclick: move |_| show_api_key.set(false), "Close" }
                            button { class: "btn btn-primary", onclick: move |_| {
                                let key = api_key_input();
                                if key.is_empty() {
                                    local_storage_remove("oxios-api-key");
                                    api::set_auth_token(None);
                                } else {
                                    local_storage_set("oxios-api-key", &key);
                                    api::set_auth_token(Some(key));
                                }
                                api_key_saved.set(true);
                            }, "Save" }
                        }
                    }
                }
            }
        }
        // Mobile overlay
        if mobile_menu() {
            div {
                class: "sidebar-overlay",
                onclick: move |_| mobile_menu.set(false),
            }
        }
    }
}
