//! Sidebar navigation with section grouping, theme toggle, and mobile support.

use dioxus::prelude::*;

use crate::Theme;
use crate::components::icons::*;

/// Top-level navigation panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Dashboard,
    Config,
    Agents,
    Personas,
    Scheduler,
    Protocol,
    Seeds,
    Workspace,
    Skills,
    Programs,
    Memory,
    HostTools,
    Security,
    Approvals,
    Events,
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
    NavItem { panel: Panel::Agents,    label: "Agents",     section: Section::Agents },
    NavItem { panel: Panel::Personas,  label: "Personas",   section: Section::Agents },
    NavItem { panel: Panel::Scheduler, label: "Scheduler",  section: Section::Agents },
    // Ouroboros
    NavItem { panel: Panel::Protocol,  label: "Protocol",   section: Section::Ouroboros },
    NavItem { panel: Panel::Seeds,     label: "Seeds",      section: Section::Ouroboros },
    // System
    NavItem { panel: Panel::Workspace, label: "Workspace",  section: Section::System },
    NavItem { panel: Panel::Skills,    label: "Skills",     section: Section::System },
    NavItem { panel: Panel::Programs,  label: "Programs",   section: Section::System },
    NavItem { panel: Panel::Memory,    label: "Memory",     section: Section::System },
    NavItem { panel: Panel::HostTools, label: "Host Tools", section: Section::System },
    // Security
    NavItem { panel: Panel::Security,  label: "Security",   section: Section::Security },
    NavItem { panel: Panel::Approvals, label: "Approvals",  section: Section::Security },
    // Monitor
    NavItem { panel: Panel::Events,    label: "Events",     section: Section::Monitor },
];

/// Render the icon for a given panel.
fn panel_icon(panel: Panel) -> Element {
    match panel {
        Panel::Chat      => rsx! { IconChat { size: 18 } },
        Panel::Dashboard => rsx! { IconDashboard { size: 18 } },
        Panel::Config    => rsx! { IconSettings { size: 18 } },
        Panel::Agents    => rsx! { IconAgents { size: 18 } },
        Panel::Personas  => rsx! { IconUsers { size: 18 } },
        Panel::Scheduler => rsx! { IconClock { size: 18 } },
        Panel::Protocol  => rsx! { IconProtocol { size: 18 } },
        Panel::Seeds     => rsx! { IconSeeds { size: 18 } },
        Panel::Workspace => rsx! { IconFolder { size: 18 } },
        Panel::Skills    => rsx! { IconSkills { size: 18 } },
        Panel::Programs  => rsx! { IconPackage { size: 18 } },
        Panel::Memory    => rsx! { IconMemory { size: 18 } },
        Panel::HostTools => rsx! { IconWrench { size: 18 } },
        Panel::Security  => rsx! { IconShield { size: 18 } },
        Panel::Approvals => rsx! { IconCheckSquare { size: 18 } },
        Panel::Events    => rsx! { IconActivity { size: 18 } },
    }
}

#[component]
pub fn Sidebar() -> Element {
    let mut panel = use_context::<Signal<Panel>>();
    let mut theme = use_context::<Signal<Theme>>();
    let mut mobile_menu = use_context::<Signal<bool>>();
    let mut collapsed = use_signal(|| false);

    let collapsed_val = collapsed();
    let is_dark = theme() == Theme::Dark;

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
