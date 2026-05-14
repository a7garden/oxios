//! Sidebar navigation with icon-based items, theme toggle, and mobile support.

use dioxus::prelude::*;

use crate::Theme;
use crate::components::icons::*;

/// Top-level navigation panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Dashboard,
    Protocol,
    Agents,
    Seeds,
    Workspace,
    Skills,
    Programs,
    Memory,
    Scheduler,
    Security,
    Approvals,
    Config,
    Events,
    Personas,
    HostTools,
}

/// Navigation item definition: (panel variant, icon component name, label).
struct NavItem {
    panel: Panel,
    label: &'static str,
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem { panel: Panel::Chat,      label: "Chat" },
    NavItem { panel: Panel::Dashboard, label: "Dashboard" },
    NavItem { panel: Panel::Protocol,  label: "Protocol" },
    NavItem { panel: Panel::Agents,    label: "Agents" },
    NavItem { panel: Panel::Seeds,     label: "Seeds" },
    NavItem { panel: Panel::Workspace, label: "Workspace" },
    NavItem { panel: Panel::Skills,    label: "Skills" },
    NavItem { panel: Panel::Programs,  label: "Programs" },
    NavItem { panel: Panel::Memory,    label: "Memory" },
    NavItem { panel: Panel::Scheduler, label: "Scheduler" },
    NavItem { panel: Panel::Security,  label: "Security" },
    NavItem { panel: Panel::Approvals, label: "Approvals" },
    NavItem { panel: Panel::Config,    label: "Config" },
    NavItem { panel: Panel::Events,    label: "Events" },
    NavItem { panel: Panel::Personas,  label: "Personas" },
    NavItem { panel: Panel::HostTools, label: "Host Tools" },
};

/// Render the icon for a given panel.
fn panel_icon(panel: Panel) -> Element {
    match panel {
        Panel::Chat      => rsx! { IconChat { size: 18 } },
        Panel::Dashboard => rsx! { IconDashboard { size: 18 } },
        Panel::Protocol  => rsx! { IconProtocol { size: 18 } },
        Panel::Agents    => rsx! { IconAgents { size: 18 } },
        Panel::Seeds     => rsx! { IconSeeds { size: 18 } },
        Panel::Workspace => rsx! { IconFolder { size: 18 } },
        Panel::Skills    => rsx! { IconSkills { size: 18 } },
        Panel::Programs  => rsx! { IconPackage { size: 18 } },
        Panel::Memory    => rsx! { IconMemory { size: 18 } },
        Panel::Scheduler => rsx! { IconClock { size: 18 } },
        Panel::Security  => rsx! { IconShield { size: 18 } },
        Panel::Approvals => rsx! { IconCheckSquare { size: 18 } },
        Panel::Config    => rsx! { IconSettings { size: 18 } },
        Panel::Events    => rsx! { IconActivity { size: 18 } },
        Panel::Personas  => rsx! { IconUsers { size: 18 } },
        Panel::HostTools => rsx! { IconWrench { size: 18 } },
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

    rsx! {
        aside { class: "sidebar {if collapsed_val { "collapsed" } else { "" }}",
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
                        if is_dark {
                            rsx! { IconSun { size: 16 } }
                        } else {
                            rsx! { IconMoon { size: 16 } }
                        }
                    }
                    // Collapse toggle (desktop only)
                    button {
                        class: "icon-btn collapse-toggle",
                        onclick: move |_| collapsed.toggle(),
                        if collapsed_val {
                            rsx! { IconChevronRight { size: 16 } }
                        } else {
                            rsx! { IconChevronLeft { size: 16 } }
                        }
                    }
                }
            }
            nav { class: "sidebar-nav",
                for item in NAV_ITEMS {
                    {{
                        let is_active = panel() == item.panel;
                        let p_val = item.panel;
                        let label = item.label;
                        let active_class = if is_active { "nav-item active" } else { "nav-item" };
                        rsx! {
                            button {
                                class: "{active_class}",
                                onclick: move |_| {
                                    panel.set(p_val);
                                    mobile_menu.set(false);
                                },
                                span { class: "nav-icon", {panel_icon(p_val)} }
                                if !collapsed_val {
                                    span { class: "nav-label", "{label}" }
                                }
                            }
                        }
                    }}
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
