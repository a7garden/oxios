//! Chat view — message history with input via REST.
//!
//! WebSocket (GET /api/chat/stream) is designed for A2A event streaming, not
//! user-to-agent chat. The frontend uses REST POST /api/chat for messages.
//! This implementation avoids the bidirectional schema mismatch that would
//! occur if we tried to send JSON over WS to a handler expecting plain text.

use dioxus::prelude::*;

use crate::api::{self, ChatRequest, ChatResponse};
use crate::components::chat::{ChatInput, ChatMessage, ProcessingIndicator};
use crate::components::icons::IconChat;

/// A single message in the chat history.
#[derive(Debug, Clone)]
struct MessageEntry {
    text: String,
    msg_type: String, // "user" | "agent" | "error"
    phase: Option<String>,
}

#[component]
pub fn ChatView() -> Element {
    let mut messages = use_signal(Vec::<MessageEntry>::new);
    let mut processing = use_signal(|| false);
    let mut session_id = use_signal(String::new);

    let on_send = move |msg: String| {
        let user_msg = MessageEntry {
            text: msg.clone(),
            msg_type: "user".to_string(),
            phase: None,
        };
        messages.push(user_msg);
        processing.set(true);

        let sid = session_id();
        spawn(async move {
            let req = ChatRequest {
                content: msg,
                user_id: "default".to_string(),
                session_id: sid,
            };
            match api::post_json::<ChatResponse, _>("/api/chat", &req).await {
                Ok(resp) => {
                    if let Some(sid) = resp.session_id {
                        session_id.set(sid);
                    }
                    messages.write().push(MessageEntry {
                        text: resp.reply,
                        msg_type: "agent".to_string(),
                        phase: resp.phase,
                    });
                    processing.set(false);
                }
                Err(e) => {
                    messages.write().push(MessageEntry {
                        text: e,
                        msg_type: "error".to_string(),
                        phase: None,
                    });
                    processing.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "panel-container chat-panel",
            div { class: "panel-header",
                h2 { IconChat { size: 20 } " Chat" }
                span { class: "ws-status", "● REST mode" }
            }
            div { class: "messages",
                for msg in messages() {
                    ChatMessage {
                        text: msg.text,
                        msg_type: msg.msg_type,
                        phase: msg.phase,
                    }
                }
                if processing() {
                    ProcessingIndicator { phase: "Processing".to_string() }
                }
            }
            ChatInput { on_send: on_send }
        }
    }
}