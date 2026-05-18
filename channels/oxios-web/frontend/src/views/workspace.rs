//! File tree browser with breadcrumb, directory navigation, and file editing.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Copy text to clipboard via JS interop.
fn copy_to_clipboard(text: &str) {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function cp(t) { navigator.clipboard.writeText(t); }")]
    extern "C" {
        fn cp(t: &str);
    }
    cp(text);
}

/// Whether the viewer is in View or Edit mode.
#[derive(Clone, Copy, PartialEq)]
enum ViewerMode {
    /// Read-only view with pre/code block.
    View,
    /// Editable textarea for modifying file content.
    Edit,
}

#[component]
pub fn WorkspaceView() -> Element {
    let mut current_dir = use_signal(String::new);
    let mut selected_file = use_signal(|| None::<String>);
    let mut file_content = use_signal(String::new);
    let mut edit_content = use_signal(String::new);
    let mut viewer_mode = use_signal(|| ViewerMode::View);
    let mut save_status = use_signal(|| None::<String>);

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
                div { class: "empty-icon", IconFolder { size: 40 } }
                p { "Empty directory." }
            }
        },
        Some(Ok(entries)) => {
            let items: Vec<Element> = entries
                .iter()
                .map(|entry| {
                    let name = entry.name.clone();
                    let is_dir = entry.is_dir;
                    let click_name = name.clone();
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
                                    edit_content.set(String::new());
                                    viewer_mode.set(ViewerMode::View);
                                    save_status.set(None);
                                    resource.restart();
                                } else {
                                    let cur = current_dir().clone();
                                    let full = if cur.is_empty() { cn.clone() } else { format!("{cur}/{cn}") };
                                    selected_file.set(Some(cn));
                                    viewer_mode.set(ViewerMode::View);
                                    save_status.set(None);
                                    spawn(async move {
                                        match api::fetch_text(&format!("/api/workspace/file/{full}")).await {
                                            Ok(c) => {
                                                // Detect binary files by checking for null bytes
                                                if c.contains('\0') {
                                                    file_content.set("Binary file — cannot display as text.".to_string());
                                                } else {
                                                    file_content.set(c);
                                                }
                                            }
                                            Err(e) => file_content.set(format!("Error: {e}")),
                                        }
                                        edit_content.set(file_content().clone());
                                    });
                                }
                            },
                            span { class: "icon",
                                {if is_dir {
                                    rsx! { IconFolder { size: 16 } }
                                } else {
                                    rsx! { IconFile { size: 16 } }
                                }}
                            }
                            span { "{name}" }
                            span { style: "margin-left:auto;font-size:11px;color:var(--text-3)", "{size_str}" }
                        }
                    }
                })
                .collect();
            rsx! { div { {items.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "error-box", { format!("Error: {e}") } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading tree..." }
            }
        },
    };

    /// Check whether the current file content looks like a binary file.
    fn is_binary_content(content: &str) -> bool {
        content.starts_with("Binary file") || content.contains('\0')
    }

    let viewer_content: Element = {
        let sel = (selected_file()).clone();
        match sel {
            Some(name) => {
                let content = file_content().clone();
                let mode = (viewer_mode()).clone();
                let is_binary = is_binary_content(&content);
                let status = save_status().clone();

                let mode_toggle: Element = if is_binary {
                    rsx! { div {} }
                } else {
                    let is_view = mode == ViewerMode::View;
                    let view_class = if is_view { "btn btn-sm btn-active" } else { "btn btn-sm" };
                    let edit_class = if is_view { "btn btn-sm" } else { "btn btn-sm btn-active" };
                    rsx! {
                        div { style: "display:flex;gap:2px",
                            button {
                                class: "{view_class}",
                                onclick: move |_| {
                                    viewer_mode.set(ViewerMode::View);
                                    edit_content.set(file_content().clone());
                                    save_status.set(None);
                                },
                                "View"
                            }
                            button {
                                class: "{edit_class}",
                                onclick: move |_| {
                                    edit_content.set(file_content().clone());
                                    viewer_mode.set(ViewerMode::Edit);
                                    save_status.set(None);
                                },
                                "Edit"
                            }
                        }
                    }
                };

                let status_msg: Element = match status {
                    Some(msg) if msg.starts_with("✓") => rsx! {
                        span { style: "color:var(--success);font-size:12px", "{msg}" }
                    },
                    Some(msg) if msg.starts_with("✗") => rsx! {
                        span { style: "color:var(--danger);font-size:12px", "{msg}" }
                    },
                    _ => rsx! { div {} },
                };

                let body: Element = if is_binary {
                    rsx! {
                        pre { "{content}" }
                    }
                } else if mode == ViewerMode::View {
                    rsx! {
                        pre { style: "white-space:pre-wrap;word-break:break-word", "{content}" }
                    }
                } else {
                    // Edit mode
                    let dir = current_dir().clone();
                    let full_path = if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") };
                    rsx! {
                        div { style: "display:flex;flex-direction:column;gap:8px",
                            textarea {
                                style: "width:100%;min-height:300px;font-family:var(--font-mono);font-size:13px;padding:8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg-1);color:var(--text-0);resize:vertical",
                                value: "{edit_content}",
                                oninput: move |evt| edit_content.set(evt.value()),
                            }
                            div { style: "display:flex;align-items:center;gap:8px",
                                {
                                    let fp = full_path.clone();
                                    let ec = (edit_content()).clone();
                                    rsx! {
                                        button {
                                            class: "btn btn-sm",
                                            onclick: move |_| {
                                                let path = fp.clone();
                                                let text = ec.clone();
                                                save_status.set(Some("Saving…".to_string()));
                                                spawn(async move {
                                                    match api::put_text(&format!("/api/workspace/file/{path}"), &text).await {
                                                        Ok(()) => {
                                                            save_status.set(Some("✓ Saved".to_string()));
                                                            file_content.set(text);
                                                        }
                                                        Err(e) => {
                                                            save_status.set(Some(format!("✗ Save failed: {e}")));
                                                        }
                                                    }
                                                });
                                            },
                                            IconCheck { size: 14 }
                                            " Save"
                                        }
                                    }
                                }
                                {status_msg}
                            }
                        }
                    }
                };

                rsx! {
                    div { style: "display:flex;align-items:center;gap:6px;margin-bottom:8px;color:var(--text-3);font-size:12px",
                        IconEye { size: 14 }
                        span { "{name}" }
                        button {
                            class: "btn btn-sm",
                            style: "margin-left:auto",
                            onclick: move |_| {
                                copy_to_clipboard(&content);
                            },
                            IconCopy { size: 12 }
                        }
                    }
                    {mode_toggle}
                    {body}
                }
            }
            None => rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconFile { size: 40 } }
                    p { "Select a file from the tree to view its contents." }
                }
            },
        }
    };

    let bc = breadcrumb;
    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconFolder { size: 20 } " Workspace" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        current_dir.set(String::new());
                        selected_file.set(None);
                        file_content.set(String::new());
                        edit_content.set(String::new());
                        viewer_mode.set(ViewerMode::View);
                        save_status.set(None);
                        resource.restart();
                    },
                    "Refresh"
                }
            }
            div { class: "workspace-split",
                div { class: "workspace-tree",
                    // Breadcrumb above the tree
                    div { class: "workspace-toolbar",
                        span { class: "breadcrumb", "{bc}" }
                    }
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
                                    edit_content.set(String::new());
                                    viewer_mode.set(ViewerMode::View);
                                    save_status.set(None);
                                    resource.restart();
                                },
                                span { class: "icon", IconArrowUp { size: 16 } }
                                span { ".." }
                            }
                        })
                    }
                    {tree_content}
                }
                div { class: "workspace-viewer",
                    {viewer_content}
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
