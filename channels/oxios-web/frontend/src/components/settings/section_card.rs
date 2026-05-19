//! Wrapper card for grouping related settings with optional reset.

use dioxus::prelude::*;

#[component]
pub fn SectionCard(
    title: &'static str,
    description: Option<&'static str>,
    /// Optional reset callback — shows a "↺ Reset" button when provided.
    on_reset: Option<EventHandler<()>>,
    children: Element,
) -> Element {
    let mut confirm_reset = use_signal(|| false);

    rsx! {
        div { class: "config-section",
            div { class: "section-card-header",
                h3 { "{title}" }
                if on_reset.is_some() {
                    if confirm_reset() {
                        span { style: "display:flex;align-items:center;gap:6px",
                            span { style: "font-size:11px;color:var(--danger)", "Reset to defaults?" }
                            button {
                                class: "btn btn-sm",
                                style: "font-size:11px",
                                onclick: move |_| {
                                    if let Some(handler) = on_reset.as_ref() {
                                        handler.call(());
                                    }
                                    confirm_reset.set(false);
                                },
                                "Yes"
                            }
                            button {
                                class: "btn btn-sm",
                                style: "font-size:11px",
                                onclick: move |_| confirm_reset.set(false),
                                "No"
                            }
                        }
                    } else {
                        button {
                            class: "btn btn-sm section-reset-btn",
                            onclick: move |_| confirm_reset.set(true),
                            "↺ Reset"
                        }
                    }
                }
            }
            if let Some(desc) = description {
                p { class: "section-description", "{desc}" }
            }
            {children}
        }
    }
}
