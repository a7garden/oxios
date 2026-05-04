//! File tree browser with breadcrumb and directory navigation.

use crate::api;
use dioxus::prelude::*;

#[component]
pub fn WorkspaceView() -> Element {
    let mut current_dir = use_signal(String::new);
    let mut selected_file = use_signal(|| None::<String>);
    let mut file_content = use_signal(String::new);

    let mut resource = use_resource(move || {
        let dir = current_dir().clone();
        async move {
            let url = if dir.is_empty() {
                "/api/workspace/tree".to_string()
            } else {
                format!("/api/workspace/tree?dir={dir}")
            };
            api::fetch_json::<Vec<api::TreeEntry>>(&url).await
        }
    });

    let breadcrumb = {
        let dir = current_dir().clone();
        if dir.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", dir)
        }
    };

    let tree_content: Element = match &(resource.value())() {
        Some(Ok(entries)) if entries.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "icon", "📂" }
                p { "Empty directory." }
            }
        },
        Some(Ok(entries)) => {
            let items: Vec<Element> = entries.iter().map(|entry| {
                let name = entry.name.clone();
                let is_dir = entry.is_dir;
                let click_name = name.clone();
                let icon = if is_dir { "📁" } else { "📄" };
                let item_class = if is_dir { "tree-item dir" } else { "tree-item file" };
                let size_str = if is_dir { String::new() } else { format_bytes(entry.size) };
                rsx! {
                    div {
                        class: "{item_class}",
                        key: "{name}",
                        onclick: move |_| {
                            let cn = click_name.clone();
                            if is_dir {
                                let cur = current_dir().clone();
                                let new_dir = if cur.is_empty() { cn } else { format!("{cur}/{cn}") };
                                current_dir.set(new_dir);
                                selected_file.set(None);
                                file_content.set(String::new());
                                resource.restart();
                            } else {
                                let cur = current_dir().clone();
                                let full = if cur.is_empty() { cn.clone() } else { format!("{cur}/{cn}") };
                                selected_file.set(Some(cn));
                                spawn(async move {
                                    match api::fetch_text(&format!("/api/workspace/file/{full}")).await {
                                        Ok(c) => file_content.set(c),
                                        Err(e) => file_content.set(format!("Error: {e}")),
                                    }
                                });
                            }
                        },
                        span { class: "icon", "{icon}" }
                        span { "{name}" }
                        span { style: "margin-left:auto;font-size:11px;color:var(--text-muted)", "{size_str}" }
                    }
                }
            }).collect();
            rsx! { div { {items.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "loading" }
                p { "Loading tree..." }
            }
        },
    };

    let viewer_content: Element = {
        let sel = (selected_file()).clone();
        match sel {
            Some(_name) => {
                let content = file_content().clone();
                rsx! { pre { "{content}" } }
            }
            None => rsx! {
                div { class: "empty-state",
                    div { class: "icon", "📄" }
                    p { "Select a file from the tree to view its contents." }
                }
            },
        }
    };

    let bc = breadcrumb;
    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { "📁 Workspace" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        current_dir.set(String::new());
                        selected_file.set(None);
                        file_content.set(String::new());
                        resource.restart();
                    },
                    "Refresh"
                }
            }
            div { class: "workspace-split",
                div { class: "workspace-tree",
                    {
                        let dir = current_dir().clone();
                        let has_parent = !dir.is_empty();
                        has_parent.then(|| rsx! {
                            div {
                                class: "tree-item dir",
                                onclick: move |_| {
                                    let cur = current_dir().clone();
                                    if let Some(idx) = cur.rfind('/') {
                                        current_dir.set(cur[..idx].to_string());
                                    } else {
                                        current_dir.set(String::new());
                                    }
                                    selected_file.set(None);
                                    file_content.set(String::new());
                                    resource.restart();
                                },
                                span { class: "icon", "⬆️" }
                                span { ".." }
                            }
                        })
                    }
                    {tree_content}
                }
                div { style: "flex:1;display:flex;flex-direction:column;overflow:hidden;",
                    div { class: "workspace-toolbar",
                        span { class: "breadcrumb", "{bc}" }
                    }
                    div { class: "workspace-viewer",
                        {viewer_content}
                    }
                }
            }
        }
    }
}

/// Format byte count as human-readable string.
fn format_bytes(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}
