//! Chat input and message components.

use dioxus::prelude::*;

use crate::components::icons::IconSend;

/// Chat text input with send button and Enter-key support.
#[component]
pub fn ChatInput(on_send: EventHandler<String>) -> Element {
    let mut text = use_signal(String::new);

    let handle_send = move |_| {
        let msg = text().trim().to_string();
        if !msg.is_empty() {
            on_send.call(msg);
            text.set(String::new());
        }
    };

    let handle_key = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter {
            let msg = text().trim().to_string();
            if !msg.is_empty() {
                on_send.call(msg);
                text.set(String::new());
            }
        }
    };

    rsx! {
        div { class: "chat-input-row",
            textarea {
                class: "chat-textarea",
                placeholder: "Send a message...",
                value: "{text}",
                rows: "1",
                oninput: move |evt| text.set(evt.value()),
                onkeydown: handle_key,
            }
            button { class: "btn btn-primary btn-icon", onclick: handle_send,
                IconSend { size: 18 }
            }
        }
    }
}

/// Single chat message bubble.
#[component]
pub fn ChatMessage(text: String, msg_type: String, phase: Option<String>) -> Element {
    let class = if msg_type == "user" { "message user" } else { "message agent" };

    rsx! {
        div { class: "{class}",
            div { class: "message-content", "{text}" }
            if let Some(p) = &phase {
                span { class: "phase-tag", "{p}" }
            }
        }
    }
}

/// Animated processing indicator.
#[component]
pub fn ProcessingIndicator(phase: String) -> Element {
    rsx! {
        div { class: "processing",
            div { class: "spinner" }
            span { "Processing ({phase})..." }
        }
    }
}
