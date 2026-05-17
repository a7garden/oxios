//! Git version control: commit log, tags, verify integrity, restore.

use crate::api;
use crate::components::icons::*;
use dioxus::prelude::*;

/// Wrapper for backend log response.
#[derive(Debug, Clone, serde::Deserialize)]
struct GitLogResponse {
    entries: Vec<api::GitLogEntry>,
}

/// Wrapper for backend tags response.
#[derive(Debug, Clone, serde::Deserialize)]
struct GitTagsResponse {
    tags: Vec<api::GitTag>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RestoreRequest {
    commit: String,
    path: String,
}

#[component]
pub fn GitView() -> Element {
    let mut log_resource = use_resource(|| async move {
        api::fetch_json::<GitLogResponse>("/api/git/log").await
    });

    let mut tags_resource = use_resource(|| async move {
        api::fetch_json::<GitTagsResponse>("/api/git/tags").await
    });

    let mut active_tab = use_signal(|| String::from("log"));
    let mut verifying = use_signal(|| false);
    let mut verify_result = use_signal(|| None::<String>);

    let mut restore_commit = use_signal(String::new);
    let mut restore_path = use_signal(String::new);
    let mut restore_result = use_signal(|| None::<String>);

    let tab_class = |tab: &str, active: &str| -> String {
        if tab == active {
            "tab-btn tab-btn-active".to_string()
        } else {
            "tab-btn".to_string()
        }
    };

    let tab_bar: Element = rsx! {
        div { class: "tabs",
            button {
                class: "{tab_class(\"log\", &active_tab())}",
                onclick: move |_| active_tab.set("log".to_string()),
                IconGit { size: 14 } " Log"
            }
            button {
                class: "{tab_class(\"tags\", &active_tab())}",
                onclick: move |_| active_tab.set("tags".to_string()),
                IconTag { size: 14 } " Tags"
            }
        }
    };

    let log_content: Element = match &(log_resource.value())() {
        Some(Ok(resp)) if resp.entries.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconGit { size: 40 } }
                p { "No commits recorded." }
            }
        },
        Some(Ok(resp)) => {
            let rows: Vec<Element> = resp.entries.iter().map(|entry| {
                let short_hash = if entry.hash.len() >= 7 {
                    entry.hash[..7].to_string()
                } else {
                    entry.hash.clone()
                };
                let msg = entry.message.clone().unwrap_or_default();
                let author = entry.author.clone().unwrap_or_else(|| "—".to_string());
                let ts = entry.timestamp.clone().unwrap_or_else(|| "—".to_string());

                rsx! {
                    div { class: "git-entry", key: "{entry.hash}",
                        div { class: "git-hash", "{short_hash}" }
                        div { class: "git-info",
                            div { class: "git-msg", "{msg}" }
                            div { class: "git-meta", "{author} · {ts}" }
                        }
                    }
                }
            }).collect();
            rsx! { div { {rows.into_iter()} } }
        },
        Some(Err(e)) => rsx! {
            div { class: "empty-state", p { { format!("Error: {e}") } } }
        },
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading commit log..." }
            }
        },
    };

    let tags_content: Element = match &(tags_resource.value())() {
        Some(Ok(resp)) if resp.tags.is_empty() => rsx! {
            div { class: "empty-state",
                div { class: "empty-icon", IconTag { size: 40 } }
                p { "No tags found." }
            }
        },
        Some(Ok(resp)) => {
            let items: Vec<Element> = resp.tags.iter().map(|tag| {
                let name = tag.name.clone();
                let hash = tag.hash.clone().unwrap_or_else(|| "—".to_string());

                rsx! {
                    div { class: "git-entry", key: "{name}",
                        div { class: "git-hash", "—" }
                        div { class: "git-info",
                            div { class: "git-msg", "{name}" }
                            div { class: "git-meta", "hash: {hash}" }
                        }
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
                div { class: "empty-icon", IconLoading { size: 40 } }
                p { "Loading tags..." }
            }
        },
    };

    let verify_btn_text = if verifying() { "Verifying..." } else { "Verify" };

    let verify_section: Element = {
        let result_text = verify_result().unwrap_or_default();
        let result_class = if result_text.starts_with('✓') {
            "text-success"
        } else if result_text.starts_with('✗') {
            "text-danger"
        } else {
            ""
        };
        rsx! {
            div { class: "git-verify",
                h3 { IconShield { size: 16 } " Verify Integrity" }
                button {
                    class: "btn btn-sm",
                    disabled: verifying(),
                    onclick: move |_| {
                        verifying.set(true);
                        verify_result.set(None);
                        spawn(async move {
                            let result = api::post_json::<serde_json::Value, ()>("/api/git/verify", &()).await;
                            verifying.set(false);
                            match result {
                                Ok(resp) => {
                                    let valid = resp.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let msg = resp.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                    let text = if valid {
                                        format!("✓ Integrity verified: {}", msg)
                                    } else {
                                        format!("✗ Integrity check failed: {}", msg)
                                    };
                                    verify_result.set(Some(text));
                                }
                                Err(e) => {
                                    verify_result.set(Some(format!("✗ {e}")));
                                }
                            }
                        });
                    },
                    "{verify_btn_text}"
                }
                if !result_text.is_empty() {
                    p { class: "{result_class}", style: "margin-top:8px;font-size:12px;font-family:var(--font-mono)", "{result_text}" }
                }
            }
        }
    };

    let restore_result_text = restore_result().unwrap_or_default();
    let restore_class = if restore_result_text.starts_with('✓') {
        "text-success"
    } else if restore_result_text.starts_with('✗') {
        "text-danger"
    } else {
        ""
    };

    let restore_section: Element = rsx! {
        div { class: "git-restore",
            h3 { IconRefresh { size: 16 } " Restore File" }
            div { class: "form-row",
                input {
                    class: "form-input",
                    style: "flex:1",
                    placeholder: "Commit hash",
                    value: "{restore_commit}",
                    oninput: move |e| restore_commit.set(e.value()),
                }
                input {
                    class: "form-input",
                    style: "flex:2",
                    placeholder: "File path",
                    value: "{restore_path}",
                    oninput: move |e| restore_path.set(e.value()),
                }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        let commit = restore_commit();
                        let path = restore_path();
                        if commit.is_empty() || path.is_empty() {
                            restore_result.set(Some("✗ Commit and path are required.".to_string()));
                            return;
                        }
                        spawn(async move {
                            let body = RestoreRequest {
                                commit,
                                path,
                            };
                            match api::post_json::<serde_json::Value, _>("/api/git/restore", &body).await {
                                Ok(resp) => {
                                    let msg = resp.get("message").and_then(|v| v.as_str()).unwrap_or("Restored successfully");
                                    restore_result.set(Some(format!("✓ {}", msg)));
                                }
                                Err(e) => {
                                    restore_result.set(Some(format!("✗ {}", e)));
                                }
                            }
                        });
                    },
                    "Restore"
                }
            }
            if !restore_result_text.is_empty() {
                p { class: "{restore_class}", style: "margin-top:8px;font-size:12px;font-family:var(--font-mono)", "{restore_result_text}" }
            }
        }
    };

    let main_content: Element = if active_tab() == "log" {
        rsx! { {log_content} }
    } else {
        rsx! { {tags_content} }
    };

    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { IconGit { size: 20 } " Git" }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        log_resource.restart();
                        tags_resource.restart();
                    },
                    "Refresh"
                }
            }
            div { class: "panel-body",
                {tab_bar}
                {main_content}
                {verify_section}
                {restore_section}
            }
        }
    }
}