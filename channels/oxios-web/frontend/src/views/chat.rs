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

/// JS helper: create WebSocket and return it.
#[wasm_bindgen(inline_js = r#"
export function create_ws(url) {
    try { return new WebSocket(url); } catch(e) { return null; }
}
"#)]
extern "C" {
    fn create_ws(url: &str) -> Option<js_sys::Object>;
}

/// JS helper: send a message on a WebSocket.
#[wasm_bindgen(inline_js = "export function ws_send(ws, msg) { if (ws && ws.readyState === 1) { ws.send(msg); } }")]
extern "C" {
    fn ws_send(ws: &js_sys::Object, msg: &str);
}

/// JS helper: get WebSocket readyState.
#[wasm_bindgen(inline_js = "export function ws_ready(ws) { return ws ? ws.readyState : -1; }")]
extern "C" {
    fn ws_ready(ws: &js_sys::Object) -> i32;
}

/// JS helper: extract the "data" field from a MessageEvent.
#[wasm_bindgen(inline_js = "export function evt_data(evt) { return evt.data; }")]
extern "C" {
    fn evt_data(evt: &JsValue) -> String;
}

/// JS helper: set onmessage callback on WebSocket.
#[wasm_bindgen(inline_js = r#"
export function ws_on_msg(ws, cb) {
    ws.onmessage = cb;
}
"#)]
extern "C" {
    fn ws_on_msg(ws: &js_sys::Object, cb: &js_sys::Function);
}

/// JS helper: set onclose callback on WebSocket.
#[wasm_bindgen(inline_js = "export function ws_on_close(ws, cb) { ws.onclose = cb; }")]
extern "C" {
    fn ws_on_close(ws: &js_sys::Object, cb: &js_sys::Function);
}

/// JS helper: set onerror callback on WebSocket.
#[wasm_bindgen(inline_js = "export function ws_on_err(ws, cb) { ws.onerror = cb; }")]
extern "C" {
    fn ws_on_err(ws: &js_sys::Object, cb: &js_sys::Function);
}

/// JS helper: set onopen callback on WebSocket.
#[wasm_bindgen(inline_js = "export function ws_on_open(ws, cb) { ws.onopen = cb; }")]
extern "C" {
    fn ws_on_open(ws: &js_sys::Object, cb: &js_sys::Function);
}

#[component]
pub fn ChatView() -> Element {
    let mut messages = use_signal(Vec::<MessageEntry>::new);
    let mut processing = use_signal(|| false);
    let mut session_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut ws_state = use_signal(|| WsState::Disconnected);

    // WebSocket handle stored as JsValue
    let mut ws_handle: Signal<Option<js_sys::Object>> = use_signal(|| None);

    // Establish WebSocket connection on mount
    use_effect(move || {
        if ws_handle().is_some() {
            return;
        }

        let token = api::auth_token();
        let host = window_location_host();
        let ws_url = match token {
            Some(ref t) => format!("ws://{host}/api/chat/stream?token={t}"),
            None => format!("ws://{host}/api/chat/stream"),
        };

        ws_state.set(WsState::Connecting);

        if let Some(ws_obj) = create_ws(&ws_url) {
            ws_handle.set(Some(ws_obj.clone()));

            // onopen
            let on_open = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                ws_state.set(WsState::Connected);
            });
            ws_on_open(&ws_obj, on_open.as_ref().unchecked_ref());
            on_open.forget();

            // onmessage
            let on_msg = Closure::<dyn FnMut(JsValue)>::new(move |evt: JsValue| {
                let json_str = evt_data(&evt);
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
            });
            ws_on_msg(&ws_obj, on_msg.as_ref().unchecked_ref());
            on_msg.forget();

            // onclose
            let on_close = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                ws_state.set(WsState::Disconnected);
            });
            ws_on_close(&ws_obj, on_close.as_ref().unchecked_ref());
            on_close.forget();

            // onerror
            let on_err = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                ws_state.set(WsState::Disconnected);
            });
            ws_on_err(&ws_obj, on_err.as_ref().unchecked_ref());
            on_err.forget();
        } else {
            ws_state.set(WsState::Disconnected);
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

        // Try WebSocket first
        if let Some(ws) = &*ws_handle.read() {
            if ws_ready(ws) == 1 {
                // OPEN
                let request = ChatRequest {
                    content: msg.clone(),
                    user_id: "default".to_string(),
                    session_id: session_id(),
                };
                if let Ok(json) = serde_json::to_string(&request) {
                    ws_send(ws, &json);
                    return;
                }
            }
        }

        // Fallback to REST POST
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
