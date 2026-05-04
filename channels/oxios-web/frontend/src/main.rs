//! Oxios Dioxus web frontend — main entry point.

use dioxus::prelude::*;

mod api;
mod components;
mod views;

use components::sidebar::Panel;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let panel = use_signal(|| Panel::Chat);
    use_context_provider(|| panel);
    rsx! { components::layout::AppLayout {} }
}
