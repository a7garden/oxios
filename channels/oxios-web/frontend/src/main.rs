//! Oxios Dioxus web frontend — main entry point.

use dioxus::prelude::*;

mod api;
mod components;
mod views;

use components::sidebar::Panel;

/// Application theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let panel = use_signal(|| Panel::Chat);
    let theme = use_signal(|| Theme::Dark);
    let mobile_menu = use_signal(|| false);

    use_context_provider(|| panel);
    use_context_provider(|| theme);
    use_context_provider(|| mobile_menu);

    rsx! { components::layout::AppLayout {} }
}
