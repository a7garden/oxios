use dioxus::prelude::*;

use crate::components::icons::*;

/// Placeholder view for panels not yet fully implemented.
#[component]
pub fn PlaceholderView(title: String) -> Element {
    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "{title}" }
            }
            div { class: "empty-state",
                div { class: "empty-icon", IconInfo { size: 40 } }
                p { "{title} panel coming soon..." }
            }
        }
    }
}
