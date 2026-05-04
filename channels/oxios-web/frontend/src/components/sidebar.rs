use dioxus::prelude::*;

/// Top-level navigation panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Dashboard,
    Protocol,
    Agents,
    Seeds,
    Workspace,
    Gardens,
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

/// Navigation items: (panel variant, emoji, label).
const NAV_ITEMS: &[(Panel, &str, &str)] = &[
    (Panel::Chat,      "💬", "Chat"),
    (Panel::Dashboard, "📊", "Dashboard"),
    (Panel::Protocol,  "🔄", "Protocol"),
    (Panel::Agents,    "🤖", "Agents"),
    (Panel::Seeds,     "🌱", "Seeds"),
    (Panel::Workspace, "📁", "Workspace"),
    (Panel::Gardens,   "🌿", "Gardens"),
    (Panel::Skills,    "🎯", "Skills"),
    (Panel::Programs,  "📦", "Programs"),
    (Panel::Memory,    "🧠", "Memory"),
    (Panel::Scheduler, "⏰", "Scheduler"),
    (Panel::Security,  "🔒", "Security"),
    (Panel::Approvals, "✅", "Approvals"),
    (Panel::Config,    "⚙️", "Config"),
    (Panel::Events,    "📡", "Events"),
    (Panel::Personas,  "🎭", "Personas"),
    (Panel::HostTools, "🔧", "Host Tools"),
];

#[component]
pub fn Sidebar() -> Element {
    let mut panel = use_context::<Signal<Panel>>();
    let mut collapsed = use_signal(|| false);

    let collapsed_val = collapsed();
    let class = if collapsed_val { "sidebar collapsed" } else { "sidebar" };

    rsx! {
        aside { class: "{class}",
            div { class: "sidebar-header",
                if !collapsed_val {
                    h2 { "OXIOS" }
                }
                button {
                    class: "collapse-btn",
                    onclick: move |_| collapsed.toggle(),
                    if collapsed_val { "▸" } else { "◂" }
                }
            }
            nav {
                for (p, emoji, label) in NAV_ITEMS {
                    {{
                        let is_active = panel() == *p;
                        let active_class = if is_active { "nav-item active" } else { "nav-item" };
                        let p_val = *p;
                        rsx! {
                            div {
                                class: "{active_class}",
                                onclick: move |_| panel.set(p_val),
                                span { class: "emoji", "{emoji}" }
                                span { class: "label", "{label}" }
                            }
                        }
                    }}
                }
            }
        }
    }
}
