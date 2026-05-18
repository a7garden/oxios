//! Advanced tab — raw JSON editor extracted from the original ConfigView.

use dioxus::prelude::*;

use crate::api;
use crate::components::icons::{IconCheck, IconX, IconCopy};
use crate::views::settings::ConfigSnapshot;

fn copy_to_clipboard(text: &str) {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function cp(t) { navigator.clipboard.writeText(t); }")]
    extern "C" { fn cp(t: &str); }
    cp(text);
}

#[component]
pub fn AdvancedTab(config: Signal<ConfigSnapshot>) -> Element {
    let mut editing = use_signal(|| false);
    let mut edit_text = use_signal(String::new);
    let mut save_error = use_signal(|| Option::<String>::None);
    let mut save_success = use_signal(|| false);

    // When entering edit mode, serialize config to pretty JSON
    let enter_edit = move |_| {
        let val = config();
        match serde_json::to_string_pretty(&val) {
            Ok(pretty) => {
                edit_text.set(pretty);
                save_error.set(None);
                save_success.set(false);
                editing.set(true);
            }
            Err(_) => save_error.set(Some("Failed to serialize config".to_string())),
        }
    };

    let cancel_edit = move |_| {
        editing.set(false);
        save_error.set(None);
    };

    let save_edit = move |_| {
        let text = edit_text();
        save_error.set(None);
        save_success.set(false);
        spawn(async move {
            // Parse as JSON to validate, then PUT
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&text);
            match parsed {
                Ok(json) => {
                    match api::put_json::<serde_json::Value, _>("/api/config", &json).await {
                        Ok(_) => {
                            save_success.set(true);
                            editing.set(false);
                            // Re-fetch config (the parent will handle this via use_resource)
                        }
                        Err(e) => save_error.set(Some(e)),
                    }
                }
                Err(e) => save_error.set(Some(format!("Invalid JSON: {e}"))),
            }
        });
    };

    if editing() {
        let text = edit_text();
        let err = save_error();
        rsx! {
            div { class: "settings-tab-content",
                div { class: "config-section",
                    div { style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px",
                        h3 { "JSON Configuration Editor" }
                        div { style: "display:flex;gap:8px",
                            button { class: "btn btn-primary btn-sm", onclick: save_edit, IconCheck { size: 14 } " Save" }
                            button { class: "btn btn-sm", onclick: cancel_edit, IconX { size: 14 } " Cancel" }
                        }
                    }
                    textarea {
                        class: "config-editor",
                        rows: 24,
                        value: "{text}",
                        oninput: move |evt| edit_text.set(evt.value()),
                        spellcheck: "false"
                    }
                    if let Some(e) = err {
                        div { class: "error-box", style: "margin-top:8px", "{e}" }
                    }
                }
            }
        }
    } else {
        let pretty = serde_json::to_string_pretty(&config())
            .unwrap_or_else(|_| "Failed to format".to_string());
        rsx! {
            div { class: "settings-tab-content",
                div { class: "config-section",
                    div { style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px",
                        h3 { "Full Configuration (JSON)" }
                        div { style: "display:flex;gap:8px",
                            button { class: "btn btn-sm", onclick: move |_| copy_to_clipboard(&pretty), IconCopy { size: 14 } " Copy" }
                            button { class: "btn btn-sm", onclick: enter_edit, "Edit" }
                        }
                    }
                    if save_success() {
                        div { class: "toast-success", "Configuration saved successfully." }
                    }
                    pre {
                        style: "font-family:var(--font-mono);font-size:13px;line-height:1.6;white-space:pre-wrap;word-break:break-word;color:var(--text-1);",
                        "{pretty}"
                    }
                }
            }
        }
    }
}