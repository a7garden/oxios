//! Chat view — message history with input, WebSocket streaming with REST fallback.

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

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

/// WebSocket connection state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WsState {
    Disconnected,
    Connecting,
    Connected,
}

/// Send a message via WebSocket.
#[wasm_bindgen(inline_js = "export function ws_send(ws, msg) { if (ws && ws.readyState === 1) { ws.send(msg); } }")]
extern "C" {
    fn ws_send(ws: &JsValue, msg: &str);
}

/// Get WebSocket readyState.
#[wasm_bindgen(inline_js = "export function ws_state(ws) { return ws ? ws.readyState : -1; }")]
extern "C" {
    fn ws_state(ws: &JsValue) -> i32;
}

#[component]
pub fn ChatView() -> Element {
    let mut messages = use_signal(Vec::<MessageEntry>::new);
    let mut processing = use_signal(|| false);
    let mut session_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut ws_state = use_signal(|| WsState::Disconnected);

    // WebSocket reference stored as a raw JsValue
    let mut ws_handle: Signal<Option<js_sys::Object>> = use_signal(|| None);

    // Establish WebSocket connection on mount
    use_effect(move || {
        if ws_handle().is_some() {
            return;
        }

        let token = api::auth_token();
        let ws_url = match token {
            Some(t) => format!(
                "ws://{}{}",
                window_location_host(),
                format!("/api/chat/stream?token={}", t)
            ),
            None => format!(
                "ws://{}/api/chat/stream",
                window_location_host()
            ),
        };

        ws_state.set(WsState::Connecting);

        match web_sys::WebSocket::new(&ws_url) {
            Ok(ws) => {
                ws_handle.set(Some(ws.clone().into()));
                ws_state.set(WsState::Connected);

                let ws_clone = ws.clone();
                let on_message = Closure::<dyn FnMut(JsValue)>::new(move |evt: JsValue| {
                    let text = js_sys::Reflect::get(&evt, &JsValue::from_str("data"))
                        .ok()
                        .and_then(|v| v.as_string());
                    if let Some(json_str) = text {
                        if let Ok(resp) = serde_json::from_str::<ChatResponse>(&json_str) {
                            if let Some(sid) = &resp.session_id {
                                session_id.set(sid.clone());
                            }
                            messages.write().push(MessageEntry {
                                text: resp.reply,
                                msg_type: "agent".to_string(),
                                phase: resp.phase,
                            });
                            processing.set(false);
                        }
                    }
                    // Prevent Rust from collecting the closure early
                    let _ = &ws_clone;
                });
                let _ = ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                on_message.forget();

                let on_close = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                    ws_state.set(WsState::Disconnected);
                });
                let _ = ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
                on_close.forget();

                let on_error = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                    ws_state.set(WsState::Disconnected);
                });
                let _ = ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
                on_error.forget();
            }
            Err(_) => {
                ws_state.set(WsState::Disconnected);
            }
        }
    });

    let on_send = move |msg: String| {
        let user_msg = MessageEntry {
            text: msg.clone(),
            msg_type: "user".to_string(),
            phase: None,
        };
        messages.push(user_msg);
        processing.set(true);
        error.set(None);

        let sid = session_id();

        // Try WebSocket first
        if let Some(ws) = &*ws_handle.read() {
            if ws_state(ws) == 1 {
                // OPEN
                let request = ChatRequest {
                    content: msg,
                    user_id: "default".to_string(),
                    session_id: sid,
                };
                if let Ok(json) = serde_json::to_string(&request) {
                    ws_send(ws, &json);
                    return;
                }
            }
        }

        // Fallback to REST POST
        let sid_for_rest = session_id();
        spawn(async move {
            let req = ChatRequest {
                content: msg,
                user_id: "default".to_string(),
                session_id: sid_for_rest,
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

    let ws_label = match ws_state() {
        WsState::Connected => "● Connected",
        WsState::Connecting => "◐ Connecting…",
        WsState::Disconnected => "○ REST mode",
    };

    rsx! {
        div { class: "panel-container chat-panel",
            div { class: "panel-header",
                h2 { IconChat { size: 20 } " Chat" }
                span { class: "ws-status", "{ws_label}" }
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
            if let Some(err) = error() {
                div { class: "error-box", "{err}" }
            }
            ChatInput { on_send: on_send }
        }
    }
}

/// Get window.location.host as a String.
fn window_location_host() -> String {
    web_sys::window()
        .and_then(|w| w.location().host().ok())
        .unwrap_or_else(|| "localhost:4200".to_string())
}
