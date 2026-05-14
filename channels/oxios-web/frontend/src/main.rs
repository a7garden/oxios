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

/// Read a value from localStorage via wasm-bindgen.
fn local_storage_get(key: &str) -> Option<String> {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "export function ls_get(k) { return localStorage.getItem(k); }")]
    extern "C" {
        fn ls_get(k: &str) -> Option<String>;
    }

    ls_get(key)
}

/// Write a value to localStorage via wasm-bindgen.
fn local_storage_set(key: &str, value: &str) {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "export function ls_set(k, v) { localStorage.setItem(k, v); }")]
    extern "C" {
        fn ls_set(k: &str, v: &str);
    }

    ls_set(key, value);
}

#[component]
fn App() -> Element {
    let panel = use_signal(|| Panel::Chat);
    let theme = use_signal(|| {
        match local_storage_get("oxios-theme").as_deref() {
            Some("light") => Theme::Light,
            _ => Theme::Dark,
        }
    });
    let mobile_menu = use_signal(|| false);

    // Persist theme changes
    use_effect(move || {
        let t = theme();
        local_storage_set("oxios-theme", if t == Theme::Light { "light" } else { "dark" });
    });

    use_context_provider(|| panel);
    use_context_provider(|| theme);
    use_context_provider(|| mobile_menu);

    rsx! { components::layout::AppLayout {} }
}
