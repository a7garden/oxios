//! App layout with responsive shell and mobile header.

use dioxus::prelude::*;

use crate::Theme;
use crate::components::sidebar::Panel;
use crate::components::icons::IconMenu;

#[component]
pub fn AppLayout() -> Element {
    let panel = use_context::<Signal<Panel>>();
    let theme = use_context::<Signal<Theme>>();
    let mut mobile_menu = use_context::<Signal<bool>>();

    let theme_class = match theme() {
        Theme::Dark => "app theme-dark",
        Theme::Light => "app theme-light",
    };

    let content: Element = match panel() {
        Panel::Chat      => rsx! { crate::views::chat::ChatView {} },
        Panel::Dashboard => rsx! { crate::views::dashboard::DashboardView {} },
        Panel::Protocol  => rsx! { crate::views::protocol::ProtocolView {} },
        Panel::Agents    => rsx! { crate::views::agents::AgentsView {} },
        Panel::Seeds     => rsx! { crate::views::seeds::SeedsView {} },
        Panel::Workspace => rsx! { crate::views::workspace::WorkspaceView {} },
        Panel::Skills    => rsx! { crate::views::skills::SkillsView {} },
        Panel::Programs  => rsx! { crate::views::programs::ProgramsView {} },
        Panel::Memory    => rsx! { crate::views::memory::MemoryView {} },
        Panel::Scheduler => rsx! { crate::views::scheduler::SchedulerView {} },
        Panel::Security  => rsx! { crate::views::security::SecurityView {} },
        Panel::Approvals => rsx! { crate::views::approvals::ApprovalsView {} },
        Panel::Config    => rsx! { crate::views::config::ConfigView {} },
        Panel::Events    => rsx! { crate::views::events::EventsView {} },
        Panel::Personas  => rsx! { crate::views::personas::PersonasView {} },
        Panel::HostTools => rsx! { crate::views::host_tools::HostToolsView {} },
    };

    let mobile_open = if mobile_menu() { "mobile-open" } else { "" };

    rsx! {
        div { class: "{theme_class} {mobile_open}",
            // Mobile header bar
            header { class: "mobile-header",
                button {
                    class: "icon-btn",
                    onclick: move |_| mobile_menu.toggle(),
                    IconMenu { size: 22 }
                }
                span { class: "mobile-brand", "OXIOS" }
                div { style: "width:38px" }
            }
            crate::components::sidebar::Sidebar {}
            main { class: "main-content",
                {content}
            }
        }
    }
}
