//! App layout with responsive shell, mobile header, and error toast.

use dioxus::prelude::*;

use crate::Theme;
use crate::api;
use crate::components::sidebar::Panel;
use crate::components::icons::{IconMenu, IconX};

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
        Panel::Config    => rsx! { crate::views::config::ConfigView {} },
        Panel::Protocol  => rsx! { crate::views::protocol::ProtocolView {} },
        Panel::Agents    => rsx! { crate::views::agents::AgentsView {} },
        Panel::Personas  => rsx! { crate::views::personas::PersonasView {} },
        Panel::Scheduler => rsx! { crate::views::scheduler::SchedulerView {} },
        Panel::Seeds     => rsx! { crate::views::seeds::SeedsView {} },
        Panel::Workspace => rsx! { crate::views::workspace::WorkspaceView {} },
        Panel::Skills    => rsx! { crate::views::skills::SkillsView {} },
        Panel::Programs  => rsx! { crate::views::programs::ProgramsView {} },
        Panel::Memory    => rsx! { crate::views::memory::MemoryView {} },
        Panel::HostTools => rsx! { crate::views::host_tools::HostToolsView {} },
        Panel::Security  => rsx! { crate::views::security::SecurityView {} },
        Panel::Approvals => rsx! { crate::views::approvals::ApprovalsView {} },
        Panel::Events    => rsx! { crate::views::events::EventsView {} },
    };

    let mobile_open = if mobile_menu() { "mobile-open" } else { "" };

    // Error toast
    let toast: Element = match api::last_api_error() {
        Some(err) => rsx! {
            div { class: "toast-container",
                div { class: "toast toast-error",
                    span { "{err}" }
                    button { class: "toast-dismiss", onclick: move |_| api::clear_api_error(), IconX { size: 14 } }
                }
            }
        },
        None => rsx! { div {} },
    };

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
            {toast}
        }
    }
}
