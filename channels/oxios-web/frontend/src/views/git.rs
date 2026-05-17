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

/// Restore form state.
#[derive(Default)]
struct RestoreForm {
    commit: String,
    path: String,
}

#[derive(Default)]
struct RestoreResult {
    success: bool,
    message: String,
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
    let mut verify_result = use_signal(|| Option::<(bool, String)>::None);

    let mut restore_form = use_signal(|| RestoreForm::default());
    let mut restore_result = use_signal(|| Option::<RestoreResult>::None);

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
                        div { class: "git-hash", "—".to_string() }
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

    let verify_section: Element = {
        let (v_class, v_text) = match verify_result() {
            Some((true, ref msg)) => ("text-success", format!("✓ Integrity verified: {}", msg)),
            Some((false, ref msg)) => ("text-danger", format!("✗ Integrity check failed: {}", msg)),
            None => ("", ""),
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
                                    let msg = resp.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    verify_result.set(Some((valid, msg)));
                                }
                                Err(e) => {
                                    verify_result.set(Some((false, e)));
                                }
                            }
                        });
                    },
                    if verifying() { "Verifying..." } else { "Verify" }
                }
                if !v_text.is_empty() {
                    p { class: "{v_class}", style: "margin-top:8px;font-size:12px;font-family:var(--font-mono)", "{v_text}" }
                }
            }
        }
    };

    let restore_section: Element = {
        let f = restore_form();
        let commit_val = f.commit.clone();
        let path_val = f.path.clone();

        let result_text = restore_result().map(|r| {
            if r.success {
                format!("✓ {}", r.message)
            } else {
                format!("✗ {}", r.message)
            }
        });

        rsx! {
            div { class: "git-restore",
                h3 { IconRefresh { size: 16 } " Restore File" }
                div { class: "form-row",
                    input {
                        class: "form-input",
                        style: "flex:1",
                        placeholder: "Commit hash",
                        value: "{commit_val}",
                        oninput: move |e| {
                            let mut f = restore_form();
                            f.commit = e.value();
                            restore_form.set(f);
                        }
                    }
                    input {
                        class: "form-input",
                        style: "flex:2",
                        placeholder: "File path",
                        value: "{path_val}",
                        oninput: move |e| {
                            let mut f = restore_form();
                            f.path = e.value();
                            restore_form.set(f);
                        }
                    }
                    button {
                        class: "btn btn-sm",
                        onclick: move |_| {
                            let f = restore_form();
                            if f.commit.is_empty() || f.path.is_empty() {
                                restore_result.set(Some(RestoreResult {
                                    success: false,
                                    message: "Commit and path are required.".to_string(),
                                }));
                                return;
                            }
                            spawn(async move {
                                let body = serde_json::json!({
                                    "commit": f.commit,
                                    "path": f.path,
                                });
                                let result = api::post_json::<serde_json::Value, _>("/api/git/restore", &body).await;
                                match result {
                                    Ok(resp) => {
                                        let msg = resp.get("message").and_then(|v| v.as_str()).unwrap_or("Restored successfully").to_string();
                                        restore_result.set(Some(RestoreResult { success: true, message: msg }));
                                    }
                                    Err(e) => {
                                        restore_result.set(Some(RestoreResult { success: false, message: e }));
                                    }
                                }
                            });
                        },
                        "Restore"
                    }
                }
                if let Some(text) = result_text {
                    p {
                        class: if text.starts_with('✓') { "text-success" } else { "text-danger" },
                        style: "margin-top:8px;font-size:12px;font-family:var(--font-mono)",
                        "{text}"
                    }
                }
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
                button { class: "btn btn-sm", onclick: move |_| {
                    log_resource.restart();
                    tags_resource.restart();
                }, "Refresh" }
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