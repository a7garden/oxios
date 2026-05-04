use dioxus::prelude::*;

use crate::api::{self, ChatRequest, ChatResponse};
use crate::components::chat::{ChatInput, ChatMessage, ProcessingIndicator};

/// A single message in the chat history.
#[derive(Debug, Clone)]
struct MessageEntry {
    text: String,
    msg_type: String,
    phase: Option<String>,
}

#[component]
pub fn ChatView() -> Element {
    let mut messages = use_signal(Vec::<MessageEntry>::new);
    let mut processing = use_signal(|| false);
    let mut session_id = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);

    let on_send = move |msg: String| {
        let user_msg = MessageEntry {
            text: msg.clone(),
            msg_type: "user".to_string(),
            phase: None,
        };
        messages.push(user_msg);
        processing.set(true);
        error.set(None);

        let sid = session_id().clone();
        spawn(async move {
            let req = ChatRequest {
                message: msg,
                session_id: sid,
            };
            match api::post_json::<ChatResponse, _>("/api/chat", &req).await {
                Ok(resp) => {
                    session_id.set(Some(resp.session_id));
                    let agent_msg = MessageEntry {
                        text: resp.response,
                        msg_type: "agent".to_string(),
                        phase: resp.phase,
                    };
                    messages.push(agent_msg);
                    processing.set(false);
                }
                Err(e) => {
                    error.set(Some(e));
                    processing.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "panel-container chat-panel",
            h1 { "💬 Chat" }
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
            if let Some(err) = error() {
                div { class: "error-box", "{err}" }
            }
            ChatInput { on_send: on_send }
        }
    }
}
