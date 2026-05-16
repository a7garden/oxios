//! Interactive first-run setup wizard.
//!
//! Detects existing credentials (oxi auth store, config.toml, environment variables).
//! If none found, runs an interactive wizard that stores the API key
//! in `~/.oxi/auth.json` (shared with oxi CLI if installed).

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

// ── Constants ───────────────────────────────────────────────────────────────

const TOTAL_STEPS: usize = 5;

const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
    "workspace/programs",
];

// ── Provider catalog ────────────────────────────────────────────────────────

/// A model choice presented during model selection.
struct ModelChoice {
    id: &'static str,
    label: &'static str,
}

/// Provider catalog entry.
struct ProviderInfo {
    id: &'static str,
    display_name: &'static str,
    env_var_name: &'static str,
    /// Secondary env var (e.g. Google also accepts GEMINI_API_KEY).
    alt_env_var_name: Option<&'static str>,
    suggested_model: &'static str,
    model_choices: &'static [ModelChoice],
    /// Whether this provider needs an API key (ollama does not).
    needs_key: bool,
    /// Whether this provider needs a base URL prompt (custom does).
    needs_base_url: bool,
    /// Whether this provider prompts for a raw model ID only (openrouter, ollama, custom).
    manual_model_only: bool,
    /// Optional footnote printed after provider selection.
    footnote: Option<&'static str>,
}

/// Static provider catalog.
static PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "anthropic",
        display_name: "Anthropic (Claude)",
        env_var_name: "ANTHROPIC_API_KEY",
        alt_env_var_name: None,
        suggested_model: "claude-sonnet-4-20250514",
        model_choices: &[
            ModelChoice {
                id: "claude-sonnet-4-20250514",
                label: "claude-sonnet-4-20250514 (recommended)",
            },
            ModelChoice {
                id: "claude-haiku-4-20250514",
                label: "claude-haiku-4-20250514 (fast, cheap)",
            },
        ],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: false,
        footnote: None,
    },
    ProviderInfo {
        id: "openai",
        display_name: "OpenAI (GPT)",
        env_var_name: "OPENAI_API_KEY",
        alt_env_var_name: None,
        suggested_model: "gpt-4o",
        model_choices: &[
            ModelChoice {
                id: "gpt-4o",
                label: "gpt-4o (recommended)",
            },
            ModelChoice {
                id: "gpt-4o-mini",
                label: "gpt-4o-mini (fast, cheap)",
            },
        ],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: false,
        footnote: None,
    },
    ProviderInfo {
        id: "google",
        display_name: "Google (Gemini)",
        env_var_name: "GOOGLE_API_KEY",
        alt_env_var_name: Some("GEMINI_API_KEY"),
        suggested_model: "gemini-2.0-flash",
        model_choices: &[
            ModelChoice {
                id: "gemini-2.0-flash",
                label: "gemini-2.0-flash (recommended)",
            },
            ModelChoice {
                id: "gemini-2.5-pro-preview-05-06",
                label: "gemini-2.5-pro-preview-05-06",
            },
        ],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: false,
        footnote: None,
    },
    ProviderInfo {
        id: "deepseek",
        display_name: "DeepSeek",
        env_var_name: "DEEPSEEK_API_KEY",
        alt_env_var_name: None,
        suggested_model: "deepseek-chat",
        model_choices: &[
            ModelChoice {
                id: "deepseek-chat",
                label: "deepseek-chat (recommended)",
            },
            ModelChoice {
                id: "deepseek-reasoner",
                label: "deepseek-reasoner",
            },
        ],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: false,
        footnote: None,
    },
    ProviderInfo {
        id: "xai",
        display_name: "xAI (Grok)",
        env_var_name: "XAI_API_KEY",
        alt_env_var_name: None,
        suggested_model: "grok-4-1-fast",
        model_choices: &[
            ModelChoice {
                id: "grok-4-1-fast",
                label: "grok-4-1-fast (fast, cheap)",
            },
            ModelChoice {
                id: "grok-4.3",
                label: "grok-4.3 (flagship)",
            },
            ModelChoice {
                id: "grok-code-fast-1",
                label: "grok-code-fast-1 (code)",
            },
        ],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: false,
        footnote: Some("Note: For SuperGrok subscription, use the API key from console.x.ai"),
    },
    ProviderInfo {
        id: "groq",
        display_name: "Groq",
        env_var_name: "GROQ_API_KEY",
        alt_env_var_name: None,
        suggested_model: "llama-3.3-70b-versatile",
        model_choices: &[
            ModelChoice {
                id: "llama-3.3-70b-versatile",
                label: "llama-3.3-70b-versatile (recommended)",
            },
        ],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: false,
        footnote: None,
    },
    ProviderInfo {
        id: "openrouter",
        display_name: "OpenRouter",
        env_var_name: "OPENROUTER_API_KEY",
        alt_env_var_name: None,
        suggested_model: "auto",
        model_choices: &[],
        needs_key: true,
        needs_base_url: false,
        manual_model_only: true,
        footnote: Some(
            "Enter model in provider/model format (e.g. anthropic/claude-sonnet-4-20250514)",
        ),
    },
    ProviderInfo {
        id: "ollama",
        display_name: "Ollama (local)",
        env_var_name: "",
        alt_env_var_name: None,
        suggested_model: "llama3",
        model_choices: &[],
        needs_key: false,
        needs_base_url: false,
        manual_model_only: true,
        footnote: Some("Make sure Ollama is running at http://127.0.0.1:11434"),
    },
    ProviderInfo {
        id: "custom",
        display_name: "Custom (OpenAI-compatible)",
        env_var_name: "",
        alt_env_var_name: None,
        suggested_model: "",
        model_choices: &[],
        needs_key: false,
        needs_base_url: true,
        manual_model_only: true,
        footnote: None,
    },
];

// ── Public API ──────────────────────────────────────────────────────────────

/// Check if the system is fully configured (model + credentials).
/// Returns `true` when onboarding should be skipped.
pub fn has_credentials(config: &OxiosConfig) -> bool {
    let Some(provider) = CredentialStore::provider_from_model(&config.engine.default_model)
    else {
        return false;
    };
    CredentialStore::has_credential(provider, config.api_key().as_deref())
}

/// Check if stdin is an interactive terminal.
pub fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

/// Run the first-time setup wizard.
///
/// Returns `Ok(true)` — onboarding completed, config written.
/// Returns `Ok(false)` — skipped (already configured, non-interactive, or cancelled).
pub fn run_onboarding(oxios_home: &Path, config: &mut OxiosConfig) -> anyhow::Result<bool> {
    // ── Re-run detection ──
    if !config.engine.default_model.is_empty() {
        if let Some(provider_id) =
            CredentialStore::provider_from_model(&config.engine.default_model)
        {
            if CredentialStore::has_credential(provider_id, config.api_key().as_deref()) {
                println!();
                println!(
                    "  Already configured as '{}'.",
                    config.engine.default_model
                );
                println!("  [K]eep / [M]odify / [R]eset?");
                print!("  > ");
                io::stdout().flush()?;
                let input = read_line();
                match input.trim().to_lowercase().as_str() {
                    "k" | "keep" | "" => {
                        return Ok(false);
                    }
                    "r" | "reset" => {
                        // Continue fresh — fall through to wizard
                    }
                    _ => {
                        // Continue with current model as default context
                    }
                }
            }
        }
    }

    // ── Need a terminal for interactive input ──
    if !is_interactive() {
        println!();
        println!("  Oxios requires initial setup but is not running in a terminal.");
        println!("  Please run `oxios` in an interactive shell.");
        println!();
        return Ok(false);
    }

    print_banner();

    let mut api_key: Option<String> = None;
    let mut skip_key_step = false;

    // ── Step 0 [auto]: Scan environment variables ──
    if let Some(detected_id) = scan_env_vars() {
        if let Some(provider) = PROVIDERS.iter().find(|p| p.id == detected_id) {
            let env_var = if !provider.env_var_name.is_empty() {
                provider.env_var_name
            } else {
                detected_id
            };
            println!();
            println!(
                "  [0/{}] Detected {} in environment. Use this provider?",
                TOTAL_STEPS, env_var
            );
            if prompt_confirm("  >", true) {
                // Check for existing auth.json credentials
                if let Ok(Some(token)) = oxi_sdk::load_token(provider.id) {
                    if !token.access_token.is_empty() {
                        println!();
                        println!(
                            "  Found existing credentials for {} in ~/.oxi/auth.json.",
                            provider.display_name
                        );
                        if prompt_confirm("  Use them?", true) {
                            skip_key_step = true;
                        }
                    }
                }

                // If not using existing credentials, grab key from env
                if !skip_key_step && provider.needs_key {
                    if let Some(env_key) = oxi_sdk::get_env_api_key(provider.id) {
                        api_key = Some(env_key);
                        skip_key_step = true;
                    }
                }

                // Model selection
                let model = prompt_model(provider)?;

                // Workspace
                let workspace = prompt_workspace()?;

                // Summary
                let key_preview = if skip_key_step {
                    let key = api_key.as_deref().unwrap_or("(from auth store)");
                    mask_key(key)
                } else {
                    let key = prompt_api_key(provider)?;
                    api_key = Some(key);
                    mask_key(api_key.as_deref().unwrap())
                };

                if !confirm_summary(provider, &model, &key_preview, &workspace) {
                    println!();
                    println!("  Setup cancelled.");
                    return Ok(false);
                }

                persist_config(
                    oxios_home,
                    config,
                    provider.id,
                    api_key.as_deref().unwrap_or(""),
                    &model,
                    &workspace,
                )?;
                print_success(oxios_home, &model);
                return Ok(true);
            }
        }
    }

    // ── Step 1: Provider selection ──
    let selected_provider = prompt_provider(PROVIDERS)?;

    if let Some(note) = selected_provider.footnote {
        println!("  {}", note);
    }

    // ── Step 2: Check existing credentials / API key input ──

    // Check for existing auth.json credentials for the selected provider
    if let Ok(Some(token)) = oxi_sdk::load_token(selected_provider.id) {
        if !token.access_token.is_empty() {
            println!();
            println!(
                "  Found existing credentials for {} in ~/.oxi/auth.json.",
                selected_provider.display_name
            );
            if prompt_confirm("  Use them?", true) {
                skip_key_step = true;
            }
        }
    }

    if !skip_key_step {
        if selected_provider.needs_key {
            api_key = Some(prompt_api_key(selected_provider)?);
        }
        if selected_provider.needs_base_url {
            // For custom providers, prompt for base URL and optional key
            let base_url = prompt_base_url()?;
            if !selected_provider.needs_key {
                print!(
                    "\n  [2/{}] API key (optional, press Enter to skip): ",
                    TOTAL_STEPS
                );
                io::stdout().flush()?;
                let key_input = read_line();
                if !key_input.trim().is_empty() {
                    api_key = Some(key_input.trim().to_string());
                }
            }
            // Store base_url for custom provider — we'll handle this in persist
            // by noting the model will use the custom provider format
            let _ = base_url; // base_url will be part of the custom provider setup
        }
    }

    // ── Step 3: Model selection ──
    let model = prompt_model(selected_provider)?;

    // ── Step 4: Workspace ──
    let workspace = prompt_workspace()?;

    // ── Summary and confirm ──
    let key_preview = if skip_key_step {
        mask_key("(from auth store)")
    } else {
        mask_key(api_key.as_deref().unwrap_or("(none)"))
    };

    if !confirm_summary(selected_provider, &model, &key_preview, &workspace) {
        println!();
        println!("  Setup cancelled.");
        return Ok(false);
    }

    // ── Step 5: Write ──
    persist_config(
        oxios_home,
        config,
        selected_provider.id,
        api_key.as_deref().unwrap_or(""),
        &model,
        &workspace,
    )?;
    print_success(oxios_home, &model);
    Ok(true)
}

// ── Helper functions (private) ──────────────────────────────────────────────

/// Scan environment variables for known provider API keys.
/// Returns the provider ID if a key is found.
fn scan_env_vars() -> Option<&'static str> {
    for provider in PROVIDERS {
        if provider.env_var_name.is_empty() {
            continue;
        }
        if let Ok(val) = std::env::var(provider.env_var_name) {
            if !val.is_empty() {
                return Some(provider.id);
            }
        }
        if let Some(alt) = provider.alt_env_var_name {
            if let Ok(val) = std::env::var(alt) {
                if !val.is_empty() {
                    return Some(provider.id);
                }
            }
        }
    }
    None
}

/// Step 1: Prompt the user to select a provider.
fn prompt_provider(catalog: &[ProviderInfo]) -> anyhow::Result<&ProviderInfo> {
    println!();
    println!("  [1/{}] Select an LLM provider:", TOTAL_STEPS);
    println!();
    for (i, provider) in catalog.iter().enumerate() {
        let mut suffix = String::new();
        if !provider.env_var_name.is_empty() {
            let primary_set = std::env::var(provider.env_var_name)
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            let alt_set = provider
                .alt_env_var_name
                .map(|alt| {
                    std::env::var(alt)
                        .map(|v| !v.is_empty())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if primary_set || alt_set {
                suffix = " (key detected)".to_string();
            }
        }
        println!("    {}) {}{}", i + 1, provider.display_name, suffix);
    }
    println!();
    loop {
        print!("  > ");
        io::stdout().flush()?;
        let input = read_line();
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= catalog.len() => {
                return Ok(&catalog[n - 1]);
            }
            _ => {
                println!(
                    "  Please enter a number between 1 and {}.",
                    catalog.len()
                );
                continue;
            }
        }
    }
}

/// Step 2: Prompt for API key, with preflight validation.
fn prompt_api_key(provider: &ProviderInfo) -> anyhow::Result<String> {
    println!();
    println!(
        "  [2/{}] Enter your {} API key:",
        TOTAL_STEPS, provider.display_name
    );
    loop {
        print!("  API key: ");
        io::stdout().flush()?;
        let input = read_line();
        let key = input.trim();
        if key.is_empty() {
            println!("  API key is required. (Ctrl+C to cancel)");
            continue;
        }

        // Preflight validation
        if validate_key(provider.id, key) {
            return Ok(key.to_string());
        } else {
            println!("  Key rejected by provider. Re-enter?");
            if !prompt_confirm("  >", false) {
                // User doesn't want to retry — accept the key anyway
                return Ok(key.to_string());
            }
        }
    }
}

/// Prompt for a custom base URL.
fn prompt_base_url() -> anyhow::Result<String> {
    println!();
    println!("  [2/{}] Enter base URL:", TOTAL_STEPS);
    loop {
        print!("  Base URL: ");
        io::stdout().flush()?;
        let input = read_line();
        let url = input.trim();
        if url.is_empty() {
            println!("  Base URL is required. (Ctrl+C to cancel)");
            continue;
        }
        return Ok(url.to_string());
    }
}

/// Step 3: Prompt for model selection.
fn prompt_model(provider: &ProviderInfo) -> anyhow::Result<String> {
    println!();
    println!("  [3/{}] Select a model:", TOTAL_STEPS);
    println!();

    if provider.manual_model_only {
        // OpenRouter, Ollama, Custom — just prompt for model ID
        if provider.id == "openrouter" {
            println!("  Enter model ID in provider/model format.");
            print!("  Model: ");
        } else {
            print!("  Model [{}]: ", provider.suggested_model);
        }
        io::stdout().flush()?;
        let input = read_line();
        let model_name = if input.trim().is_empty() {
            provider.suggested_model.to_string()
        } else {
            input.trim().to_string()
        };

        // Auto-prepend provider/ prefix if not already present (skip for openrouter)
        if provider.id == "openrouter" {
            return Ok(model_name);
        }
        return Ok(if model_name.contains('/') {
            model_name
        } else {
            format!("{}/{}", provider.id, model_name)
        });
    }

    // Providers with model_choices
    for (i, choice) in provider.model_choices.iter().enumerate() {
        println!("    {}) {}", i + 1, choice.label);
    }
    let manual_idx = provider.model_choices.len() + 1;
    println!("    {}) Enter model ID manually", manual_idx);
    println!();

    loop {
        print!("  > ");
        io::stdout().flush()?;
        let input = read_line();
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= provider.model_choices.len() => {
                let choice = &provider.model_choices[n - 1];
                return Ok(format!("{}/{}", provider.id, choice.id));
            }
            Ok(n) if n == manual_idx => {
                print!("  Model ID: ");
                io::stdout().flush()?;
                let manual_input = read_line();
                let model_name = manual_input.trim();
                if model_name.is_empty() {
                    println!("  Model ID cannot be empty.");
                    continue;
                }
                // Auto-prepend provider/ prefix if not already present
                return Ok(if model_name.contains('/') {
                    model_name.to_string()
                } else {
                    format!("{}/{}", provider.id, model_name)
                });
            }
            _ => {
                println!(
                    "  Please enter a number between 1 and {}.",
                    manual_idx
                );
                continue;
            }
        }
    }
}

/// Prompt for workspace directory.
fn prompt_workspace() -> anyhow::Result<String> {
    let default_workspace = dirs::home_dir()
        .map(|h| format!("{}/.oxios/workspace", h.display()))
        .unwrap_or_else(|| "~/.oxios/workspace".to_string());

    println!();
    println!("  [4/{}] Workspace path:", TOTAL_STEPS);
    println!("  Press Enter to use the default.");
    print!("  Workspace [{}]: ", default_workspace);
    io::stdout().flush()?;

    let input = read_line();
    let workspace = if input.trim().is_empty() {
        default_workspace
    } else {
        input.trim().to_string()
    };
    Ok(crate::config::expand_home(&workspace)
        .to_string_lossy()
        .to_string())
}

/// Step 4: Show summary and confirm.
fn confirm_summary(
    provider: &ProviderInfo,
    model: &str,
    key_preview: &str,
    workspace: &str,
) -> bool {
    println!();
    println!("  ┌─────────────────────────────────────────────┐");
    println!("  │            Configuration Summary             │");
    println!("  ├─────────────────────────────────────────────┤");
    println!(
        "  │  Provider:  {:<32}│",
        provider.display_name
    );
    println!("  │  Model:     {:<32}│", model);
    println!("  │  Key:       {:<32}│", key_preview);
    println!("  │  Workspace: {:<32}│", truncate_str(workspace, 32));
    println!("  └─────────────────────────────────────────────┘");
    println!();
    println!("  [5/{}] Write configuration?", TOTAL_STEPS);
    prompt_confirm("  >", true)
}

/// Attempt to validate an API key by creating a provider and doing a lightweight stream check.
fn validate_key(provider_id: &str, key: &str) -> bool {
    let oxi = oxi_sdk::OxiBuilder::new().with_builtins().build();

    // Try to resolve the suggested model for this provider
    let suggested = suggested_model(provider_id);
    let model_id = format!("{}/{}", provider_id, suggested);

    // Try to create a provider instance
    let provider_result = oxi.create_provider(provider_id);

    let provider = match provider_result {
        Ok(p) => p,
        Err(_) => {
            // Provider not known — can't validate, assume ok
            return true;
        }
    };

    // Temporarily set the env var so the provider can pick up the key
    let env_key = match oxi_ai::get_provider_env_key(provider_id) {
        Some(k) => k.to_string(),
        None => return true, // Can't validate without knowing the env var name
    };

    let orig = std::env::var(&env_key).ok();
    std::env::set_var(&env_key, key);

    let valid = (|| {
        let model = match oxi.resolve_model(&model_id) {
            Ok(m) => m,
            Err(_) => return true, // Model not in registry — can't validate
        };

        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return true,
        };

        rt.block_on(async {
            use oxi_ai::{Context, Message};

            let mut ctx = Context::new();
            ctx.add_message(Message::user("hi"));

            match provider.stream(&model, &ctx, None).await {
                Ok(mut stream) => {
                    // Try to get at least one event from the stream
                    use futures::StreamExt;
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        stream.next(),
                    )
                    .await
                    {
                        Ok(Some(_)) => true,
                        Ok(None) => true, // Stream ended — key was accepted
                        Err(_) => true,   // Timeout — can't confirm, assume ok
                    }
                }
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    // Auth errors indicate a bad key
                    !is_auth_error(&err_str)
                }
            }
        })
    })();

    // Restore original env var
    match orig {
        Some(v) => std::env::set_var(&env_key, v),
        None => std::env::remove_var(&env_key),
    }

    valid
}

/// Check if an error string looks like an authentication failure.
fn is_auth_error(err_str: &str) -> bool {
    let lower = err_str.to_lowercase();
    lower.contains("401")
        || lower.contains("403")
        || lower.contains("invalid api key")
        || lower.contains("unauthorized")
        || lower.contains("incorrect api key")
        || lower.contains("authentication")
        || lower.contains("invalid x-api-key")
}

/// Human-readable provider name.
#[allow(dead_code)]
fn provider_display(provider_id: &str) -> &'static str {
    match PROVIDERS.iter().find(|p| p.id == provider_id) {
        Some(p) => p.display_name,
        None => "unknown",
    }
}

/// Suggested model ID (without provider prefix) for each provider.
fn suggested_model(provider_id: &str) -> &'static str {
    PROVIDERS
        .iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.suggested_model)
        .unwrap_or("default")
}

/// Mask a key for display: show first 4 and last 4 chars.
fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return key.to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

/// Truncate a string to max_len, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Persist configuration: store key, create workspace, update config.
fn persist_config(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider_id: &str,
    api_key: &str,
    model: &str,
    workspace: &str,
) -> anyhow::Result<()> {
    print!("\n  Saving configuration... ");
    io::stdout().flush()?;

    // Store key via CredentialStore (only if we have a key)
    if !api_key.is_empty() {
        CredentialStore::store(provider_id, api_key)?;
    }

    // Create workspace directories
    std::fs::create_dir_all(workspace)?;
    for subdir in WORKSPACE_SUBDIRS {
        std::fs::create_dir_all(Path::new(workspace).join(subdir))?;
    }

    // Update config
    config.engine.default_model = model.to_string();
    config.kernel.workspace = workspace.to_string();
    write_config(oxios_home, config)?;

    println!("done");
    Ok(())
}

/// Write config to disk.
fn write_config(oxios_home: &Path, config: &OxiosConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(oxios_home)?;
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    std::fs::write(oxios_home.join("config.toml"), &toml_str)?;
    Ok(())
}

// ── UI ──────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║       ⬡  Oxios — First-time Setup        ║");
    println!("  ╚═══════════════════════════════════════════╝");
    println!();
    println!("  This wizard will configure your API credentials.");
    println!("  Press Ctrl+C at any time to cancel.");
}

fn print_success(oxios_home: &Path, model: &str) {
    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║             Setup Complete!               ║");
    println!("  ╚═══════════════════════════════════════════╝");
    println!();
    println!(
        "    Config:   {}",
        oxios_home.join("config.toml").display()
    );
    println!("    Model:    {}", model);
    println!();
    println!("  Next steps:");
    println!("    oxios               → start the daemon");
    println!("    oxios daemon install → register as system service");
    println!("    open http://127.0.0.1:4200");
    println!();
}

// ── IO helpers ──────────────────────────────────────────────────────────────

/// Read a line from stdin, trimmed.
fn read_line() -> String {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap_or_default();
    buf.trim_end().to_string()
}

/// Yes/No prompt. Returns `default` on empty input.
fn prompt_confirm(prompt: &str, default: bool) -> bool {
    let suffix = if default { " [Y/n]" } else { " [y/N]" };
    print!("{}{} ", prompt, suffix);
    io::stdout().flush().unwrap_or_default();
    let input = read_line();
    if input.trim().is_empty() {
        return default;
    }
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}
