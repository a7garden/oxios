//! SSE view: use gloo_net eventsource + futures_util StreamExt, keep last 200 events.

use crate::api::EventEntry;
use crate::components::icons::*;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn EventsView() -> Element {
    let mut events = use_signal(|| Vec::<EventEntry>::new());
    let mut connected = use_signal(|| false);

    // Set up EventSource on mount
    let mut started = use_signal(|| false);
    if !started() {
        started.set(true);
        spawn(async move {
            use gloo_net::eventsource::futures::EventSource;

            match EventSource::new("/api/events") {
                Ok(mut es) => {
                    connected.set(true);
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
                                                        .and_then(|obj| obj.keys().next().cloned())
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
                }
                Err(_) => {
                    connected.set(false);
                }
            }
        });
    }

    let conn_status = if connected() { "connected" } else { "disconnected" };
    let event_count = events().len();

    let events_content: Element = {
        let evts = events();
        if evts.is_empty() {
            rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconActivity { size: 40 } }
                    p { "Waiting for events..." }
                }
            }
        } else {
            let items: Vec<Element> = evts
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    rsx! {
                        div { class: "event-line", key: "{i}",
                            span { class: "event-time", "{entry.time}" }
                            span { class: "event-type", "{entry.event_type}" }
                            " {entry.data}"
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
                div { style: "display:flex;gap:8px;align-items:center",
                    span { style: "font-size:12px;color:var(--text-3);font-family:var(--font-mono)",
                        "{conn_status} · {event_count} events"
                    }
                    button {
                        class: "btn btn-danger btn-sm",
                        onclick: move |_| {
                            events.write().clear();
                        },
                        "Clear"
                    }
                }
            }
            div { class: "panel-body",
                {events_content}
            }
        }
    }
}
