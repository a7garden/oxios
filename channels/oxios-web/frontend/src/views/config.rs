//! Display and edit TOML config with save support.

use dioxus::prelude::*;
use crate::api;
use crate::components::icons::*;

/// Copy text to clipboard via JS interop.
fn copy_to_clipboard(text: &str) {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function cp(t) { navigator.clipboard.writeText(t); }")]
    extern "C" { fn cp(t: &str); }
    cp(text);
}

#[component]
pub fn ConfigView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_json::<serde_json::Value>("/api/config").await
    });

    let mut editing = use_signal(|| false);
    let mut edit_text = use_signal(String::new);
    let mut save_error = use_signal(|| None::<String>);
    let mut save_success = use_signal(|| false);

    // Initialize edit text when entering edit mode
    let enter_edit = move |_| {
        if let Some(Ok(val)) = &(resource.value())() {
            let pretty = serde_json::to_string_pretty(val)
                .unwrap_or_else(|_| "Failed to format".to_string());
            edit_text.set(pretty);
            save_error.set(None);
            save_success.set(false);
            editing.set(true);
        }
    };

    let save_edit = move |_| {
        let text = edit_text();
        save_error.set(None);
        save_success.set(false);
        spawn(async move {
            // Parse as JSON to validate
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&text);
            match parsed {
                Ok(json) => {
                    match api::put_json::<serde_json::Value, _>("/api/config", &json).await {
                        Ok(_) => {
                            save_success.set(true);
                            editing.set(false);
                            resource.restart();
                        }
                        Err(e) => save_error.set(Some(e)),
                    }
                }
                Err(e) => save_error.set(Some(format!("Invalid JSON: {e}"))),
            }
        });
    };

    let cancel_edit = move |_| {
        editing.set(false);
        save_error.set(None);
    };

    let content: Element = match &(resource.value())() {
        Some(Ok(value)) => {
            if editing() {
                let text = edit_text();
                let err = save_error();
                rsx! {
                    div { class: "config-grid",
                        div { class: "config-section",
                            h3 { "Configuration Editor" }
                            textarea {
                                class: "config-editor",
                                value: "{text}",
                                oninput: move |evt| edit_text.set(evt.value()),
                            }
                            if let Some(e) = err {
                                div { class: "error-box", style: "margin-top:8px", "{e}" }
                            }
                            div { style: "display:flex;gap:8px;margin-top:12px",
                                button { class: "btn btn-primary btn-sm", onclick: save_edit, IconCheck { size: 14 } " Save" }
                                button { class: "btn btn-sm", onclick: cancel_edit, IconX { size: 14 } " Cancel" }
                            }
                        }
                    }
                }
            } else {
                let pretty = serde_json::to_string_pretty(value)
                    .unwrap_or_else(|_| "Failed to format".to_string());
                rsx! {
                    div { class: "config-grid",
                        div { class: "config-section",
                            h3 { "Configuration" }
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
        Some(Err(e)) => rsx! {
            div { class: "error-box", { format!("Error: {e}") } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading configuration..." }
            }
        },
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconSettings { size: 20 } " Config" }
                div { style: "display:flex;gap:8px",
                    if !editing() {
                        button { class: "btn btn-sm", onclick: enter_edit, "Edit" }
                        button { class: "btn btn-sm", onclick: move |_| {
                            if let Some(Ok(val)) = &(resource.value())() {
                                let pretty = serde_json::to_string_pretty(val).unwrap_or_default();
                                copy_to_clipboard(&pretty);
                            }
                        }, IconCopy { size: 14 } " Copy" }
                    }
                    button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
                }
            }
            div { class: "panel-body",
                {content}
            }
        }
    }
}
