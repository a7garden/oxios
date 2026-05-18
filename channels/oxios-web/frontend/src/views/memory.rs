//! Memory view with search functionality and create modal.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum SearchMode {
    Keyword,
    Semantic,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct MemorySearchResult {
    name: String,
    category: String,
    #[allow(dead_code)]
    content: String,
    #[serde(default)]
    score: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CreateMemoryRequest {
    name: String,
    category: String,
    #[allow(dead_code)]
    content: String,
}

// ---------------------------------------------------------------------------
// Create Memory Modal
// ---------------------------------------------------------------------------

#[component]
fn CreateMemoryModal(
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| String::new());
    let mut category = use_signal(|| "general".to_string());
    let mut content = use_signal(|| String::new());
    let mut is_creating = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "modal-overlay", onclick: move |_| on_close(()),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h3 { "Create Memory Entry" }
                    button { class: "btn-close", onclick: move |_| on_close(()),
                        IconX { size: 18 }
                    }
                }
                div { class: "modal-body",
                    if let Some(err) = error_msg() {
                        div { class: "message message-error", "{err}" }
                    }
                    div { class: "form-field",
                        label { "Name" }
                        input {
                            class: "input",
                            placeholder: "Entry name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value().clone())
                        }
                    }
                    div { class: "form-field",
                        label { "Category" }
                        select {
                            class: "select",
                            value: "{category}",
                            onchange: move |e| category.set(e.value().clone()),
                            option { value: "general", "General" }
                            option { value: "conversation", "Conversation" }
                            option { value: "code", "Code" }
                            option { value: "documentation", "Documentation" }
                            option { value: "context", "Context" }
                        }
                    }
                    div { class: "form-field",
                        label { "Content" }
                        textarea {
                            class: "textarea",
                            rows: 8,
                            placeholder: "Memory content...",
                            value: "{content}",
                            oninput: move |e| content.set(e.value().clone())
                        }
                    }
                }
                div { class: "modal-footer",
                    button { class: "btn", onclick: move |_| on_close(()), "Cancel" }
                    button {
                        class: "btn btn-primary",
                        disabled: is_creating(),
                        onclick: move |_| {
                            if name().trim().is_empty() {
                                error_msg.set(Some("Name is required".to_string()));
                                return;
                            }
                            if content().trim().is_empty() {
                                error_msg.set(Some("Content is required".to_string()));
                                return;
                            }
                            error_msg.set(None);
                            is_creating.set(true);
                            let req = CreateMemoryRequest {
                                name: name().trim().to_string(),
                                category: category(),
                                content: content().clone(),
                            };
                            spawn(async move {
                                match api::post_json::<(), _>("/api/memory", &req).await {
                                    Ok(_) => {
                                        is_creating.set(false);
                                        on_created(());
                                        on_close(());
                                    }
                                    Err(e) => {
                                        error_msg.set(Some(format!("Error: {e}")));
                                        is_creating.set(false);
                                    }
                                }
                            });
                        },
                        if is_creating() { "Creating..." } else { "Create" }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main Memory View
// ---------------------------------------------------------------------------

#[component]
pub fn MemoryView() -> Element {
    let mut resource = use_resource(|| async move {
        api::fetch_paginated::<api::MemoryListItem>("/api/memory").await
    });

    let mut selected_name = use_signal(|| None::<String>);
    let mut entry_content = use_signal(String::new);

    // Search state
    let mut search_query = use_signal(|| String::new());
    let mut search_mode = use_signal(|| SearchMode::Keyword);
    let mut search_results = use_signal(|| Option::<Vec<MemorySearchResult>>::None);
    let mut is_searching = use_signal(|| false);
    let mut search_error = use_signal(|| Option::<String>::None);

    // Modal state
    let mut show_create_modal = use_signal(|| false);

    // Load detail when an entry is selected
    let mut load_detail = move |name: String| {
        selected_name.set(Some(name.clone()));
        spawn(async move {
            match api::fetch_json::<api::MemoryDetail>(&format!("/api/memory/{name}")).await {
                Ok(m) => entry_content.set(m.content),
                Err(e) => entry_content.set(format!("Error: {e}")),
            }
        });
    };

    let mode_toggle_keyword = if search_mode() == SearchMode::Keyword {
        "toggle-btn toggle-btn-active"
    } else {
        "toggle-btn"
    };

    let mode_toggle_semantic = if search_mode() == SearchMode::Semantic {
        "toggle-btn toggle-btn-active"
    } else {
        "toggle-btn"
    };

    // List content - either search results or default list
    let list_content: Element = if let Some(results) = search_results() {
        if results.is_empty() {
            rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconSearch { size: 40 } }
                    p { "No results found for your search." }
                }
            }
        } else {
            let items: Vec<Element> = results.iter().map(|result| {
                let name = result.name.clone();
                let category = result.category.clone();
                let score = result.score;
                let load_name = name.clone();
                rsx! {
                    div {
                        class: "item-card",
                        key: "{name}",
                        onclick: move |_| load_detail(load_name.clone()),
                        div { class: "item-title", "{name}" }
                        div { class: "item-subtitle",
                            "{category}"
                            if let Some(s) = score {
                                span { style: "color:var(--text-3);margin-left:8px", "score: {s:.2}" }
                            }
                        }
                    }
                }
            }).collect();
            rsx! { div { class: "item-list", {items.into_iter()} } }
        }
    } else {
        match &(resource.value())() {
            Some(Ok(entries)) if entries.is_empty() => rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconMemory { size: 40 } }
                    p { "No memory entries yet. Agent conversations generate daily memory summaries." }
                }
            },
            Some(Ok(entries)) => {
                let items: Vec<Element> = entries.iter().map(|entry| {
                    let name = entry.name.clone();
                    let category = entry.category.clone();
                    let load_name = name.clone();
                    rsx! {
                        div {
                            class: "item-card",
                            key: "{name}",
                            onclick: move |_| load_detail(load_name.clone()),
                            div { class: "item-title", "{name}" }
                            div { class: "item-subtitle", "{category}" }
                        }
                    }
                }).collect();
                rsx! { div { class: "item-list", {items.into_iter()} } }
            },
            Some(Err(e)) => rsx! {
                div { class: "error-box", { format!("Error: {e}") } }
            },
            None => rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconLoading { size: 40 } }
                    p { "Loading memory..." }
                }
            },
        }
    };

    let detail: Element = {
        let sel = selected_name();
        match sel {
            Some(_) => {
                let text = entry_content().clone();
                rsx! {
                    div { class: "detail-view",
                        h3 { "Memory Entry" }
                        pre { "{text}" }
                    }
                }
            }
            None => rsx! {
                div { class: "empty-state",
                    div { class: "empty-icon", IconFile { size: 40 } }
                    p { "Select an entry from the list to view its content." }
                }
            },
        }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconMemory { size: 20 } " Memory" }
                button { class: "btn btn-sm", onclick: move |_| show_create_modal.set(true), IconPlus { size: 14 } " New" }
                button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
            }
            div { class: "search-toolbar",
                input {
                    class: "input input-sm",
                    placeholder: "Search memory...",
                    value: "{search_query}",
                    oninput: move |e| search_query.set(e.value().clone()),
                    onkeydown: move |e| {
                        if e.key().to_string() == "Enter" {
                            let query = search_query().trim().to_string();
                            if query.is_empty() {
                                search_error.set(Some("Enter a search query".to_string()));
                                return;
                            }
                            search_error.set(None);
                            search_results.set(None);
                            is_searching.set(true);
                            let mode = search_mode();
                            let endpoint = match mode {
                                SearchMode::Keyword => "/api/memory/search",
                                SearchMode::Semantic => "/api/memory/semantic",
                            };
                            spawn(async move {
                                #[derive(serde::Serialize)]
                                struct SearchPayload { query: String }
                                match api::post_json::<Vec<MemorySearchResult>, _>(endpoint, &SearchPayload { query }).await {
                                    Ok(results) => {
                                        search_results.set(Some(results));
                                        is_searching.set(false);
                                    }
                                    Err(e) => {
                                        search_error.set(Some(format!("Search error: {e}")));
                                        is_searching.set(false);
                                    }
                                }
                            });
                        }
                    }
                }
                button {
                    class: "btn btn-sm",
                    disabled: is_searching(),
                    onclick: move |_| {
                        let query = search_query().trim().to_string();
                        if query.is_empty() {
                            search_error.set(Some("Enter a search query".to_string()));
                            return;
                        }
                        search_error.set(None);
                        search_results.set(None);
                        is_searching.set(true);
                        let mode = search_mode();
                        let endpoint = match mode {
                            SearchMode::Keyword => "/api/memory/search",
                            SearchMode::Semantic => "/api/memory/semantic",
                        };
                        spawn(async move {
                            #[derive(serde::Serialize)]
                            struct SearchPayload { query: String }
                            match api::post_json::<Vec<MemorySearchResult>, _>(endpoint, &SearchPayload { query }).await {
                                Ok(results) => {
                                    search_results.set(Some(results));
                                    is_searching.set(false);
                                }
                                Err(e) => {
                                    search_error.set(Some(format!("Search error: {e}")));
                                    is_searching.set(false);
                                }
                            }
                        });
                    },
                    IconSearch { size: 14 }
                    " Search"
                }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        search_query.set(String::new());
                        search_results.set(None);
                        search_error.set(None);
                    },
                    IconX { size: 14 }
                    " Clear"
                }
                div { class: "search-toggle",
                    div {
                        class: "{mode_toggle_keyword}",
                        onclick: move |_| search_mode.set(SearchMode::Keyword),
                        "Keyword"
                    }
                    div {
                        class: "{mode_toggle_semantic}",
                        onclick: move |_| search_mode.set(SearchMode::Semantic),
                        "Semantic"
                    }
                }
            }
            if let Some(err) = search_error() {
                div { class: "message message-error", "{err}" }
            }
            div { class: "workspace-split",
                div { class: "workspace-tree",
                    {list_content}
                }
                div { class: "workspace-viewer",
                    {detail}
                }
            }
        }
        if show_create_modal() {
            CreateMemoryModal {
                on_close: move |_| show_create_modal.set(false),
                on_created: move |_| resource.restart(),
            }
        }
    }
}