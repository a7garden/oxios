//! SSE view: use gloo_net eventsource + futures_util StreamExt, keep last 200 events.
//! Includes automatic reconnection with exponential backoff.

use crate::api::EventEntry;
use crate::components::icons::*;
use dioxus::prelude::*;
use futures_util::StreamExt;

/// Maximum backoff duration (milliseconds).
const MAX_BACKOFF_MS: u32 = 30_000;
/// Initial backoff duration (milliseconds).
const INITIAL_BACKOFF_MS: u32 = 1_000;

#[component]
pub fn EventsView() -> Element {
    let mut events = use_signal(|| Vec::<EventEntry>::new());
    let mut connected = use_signal(|| false);

    // Spawn EventSource listener with auto-reconnect
    let mut started = use_signal(|| false);
    if !started() {
        started.set(true);
        spawn(async move {
            use gloo_net::eventsource::futures::EventSource;

            let mut backoff = INITIAL_BACKOFF_MS;

            loop {
                match EventSource::new("/api/events") {
                    Ok(mut es) => {
                        connected.set(true);
                        backoff = INITIAL_BACKOFF_MS;

                        match es.subscribe("message") {
                            Ok(mut subscription) => {
                                while let Some(msg) = StreamExt::next(&mut subscription).await {
                                    match msg {
                                        Ok((_event_type, msg_event)) => {
                                            let data_str = msg_event
                                                .data()
                                                .as_string()
                                                .unwrap_or_default();
                                            let now = chrono::Local::now()
                                                .format("%H:%M:%S")
                                                .to_string();
                                            let event_type =
                                                serde_json::from_str::<serde_json::Value>(&data_str)
                                                    .ok()
                                                    .and_then(|v| {
                                                        v.as_object()
                                                            .and_then(|obj| obj.get("type"))
                                                            .and_then(|v| v.as_str())
                                                            .map(String::from)
                                                    })
                                                    .unwrap_or_else(|| "unknown".to_string());
                                            let mut evts = events.write();
                                            evts.push(EventEntry {
                                                time: now,
                                                event_type,
                                                data: data_str,
                                            });
                                            while evts.len() > 200 {
                                                evts.remove(0);
                                            }
                                        }
                                        Err(_) => {
                                            connected.set(false);
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                connected.set(false);
                            }
                        }
                        // Stream ended — fall through to reconnect
                    }
                    Err(_) => {
                        connected.set(false);
                    }
                }

                // Wait before reconnecting
                gloo_timers::future::TimeoutFuture::new(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF_MS);
            }
        });
    }

    let mut filter = use_signal(String::new);

    let conn_status = if connected() { "connected" } else { "disconnected" };
    let conn_class = if connected() { "text-success" } else { "text-danger" };
    let event_count = events().len();

    let events_content: Element = {
        let evts = events();
        let filter_str = filter().to_lowercase();
        let filtered: Vec<&EventEntry> = if filter_str.is_empty() {
            evts.iter().collect()
        } else {
            evts.iter().filter(|e| {
                e.event_type.to_lowercase().contains(&filter_str) || e.data.to_lowercase().contains(&filter_str)
            }).collect()
        };
        if evts.is_empty() {
            rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconActivity { size: 40 } }
                    p { "Waiting for events..." }
                }
            }
        } else {
            let items: Vec<Element> = filtered
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    rsx! {
                        div { class: "event-line", key: "{i}",
                            span { class: "event-time", "{entry.time}" }
                            span { class: "event-type", "{entry.event_type}" }
                            span { class: "event-payload", "{entry.data}" }
                        }
                    }
                })
                .collect();
            rsx! {
                div { class: "event-log",
                    {items.into_iter()}
                }
            }
        }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconActivity { size: 20 } " Events" }
                div { style: "display:flex;gap:8px;align-items:center;margin-bottom:12px",
                    div { style: "display:flex;align-items:center;gap:6px;flex:1;background:var(--bg-2);border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 8px",
                        IconSearch { size: 14 }
                        input {
                            style: "border:none;background:transparent;color:var(--text-0);font-size:12px;font-family:var(--font-mono);outline:none;flex:1",
                            placeholder: "Filter events...",
                            value: "{filter}",
                            oninput: move |evt| filter.set(evt.value()),
                        }
                    }
                    span { class: "{conn_class}", style: "font-size:12px;font-family:var(--font-mono);white-space:nowrap",
                        "{conn_status} · {event_count}"
                    }
                    button {
                        class: "btn btn-danger btn-sm",
                        onclick: move |_| {
                            events.write().clear();
                        },
                        IconTrash { size: 14 } " Clear"
                    }
                }
            }
            div { class: "panel-body",
                {events_content}
            }
        }
    }
}
