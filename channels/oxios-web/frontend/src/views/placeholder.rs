use dioxus::prelude::*;

/// Placeholder view for panels not yet fully implemented.
#[component]
pub fn PlaceholderView(title: String, emoji: String) -> Element {
    rsx! {
        div { class: "panel-container",
            h1 { "{emoji} {title}" }
            div { class: "empty-state",
                div { class: "empty-icon", "{emoji}" }
                p { "{title} panel coming soon…" }
            }
        }
    }
}
