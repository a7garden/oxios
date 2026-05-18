//! Wrapper card for grouping related settings.

use dioxus::prelude::*;

#[component]
pub fn SectionCard(
    title: &'static str,
    description: Option<&'static str>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "config-section",
            h3 { "{title}" }
            if let Some(desc) = description {
                p { class: "section-description", "{desc}" }
            }
            {children}
        }
    }
}