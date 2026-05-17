//! Interactive first-run setup wizard.
//!
//! Uses oxi-sdk's provider/model catalog and env key detection.
//! No hardcoded provider lists — everything comes from oxi-ai.

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

/// Providers that don't need an API key (e.g., local models).
const NO_KEY_PROVIDERS: &[&str] = &[];

/// Providers to exclude from the interactive list (cloud gateways, regional variants).
/// Users who need these can set them up via config.toml directly.
const HIDDEN_PROVIDERS: &[&str] = &[
    "amazon-bedrock",      // requires AWS setup, not a simple API key
    "azure-openai-responses", // requires Azure deployment
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "google-vertex",       // requires ADC, not a simple API key
    "minimax-cn",
    "moonshotai-cn",
    "openai-codex",        // subset of openai
    "opencode-go",
    "vercel-ai-gateway",
    "xiaomi",
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
                    "r" | "reset" => { /* fall through to wizard */ }
                    _ => { /* modify — fall through */ }
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

    // ── Step 0 [auto]: Check for env vars and existing auth tokens ──
    let env_providers = oxi_sdk::get_all_env_keys();
    if !env_providers.is_empty() {
        // Find the first provider that also has models in the registry
        let detected = env_providers
            .keys()
            .find(|p| !oxi_sdk::get_provider_models(p).is_empty());

        if let Some(provider) = detected {
            println!();
            let keys = oxi_sdk::find_env_keys(provider);
            let var_name = keys
                .and_then(|k| k.first().copied())
                .unwrap_or(provider);
            println!(
                "  Detected {} in environment for '{}'.",
                var_name, provider
            );
            if prompt_confirm("  Use this provider?", true) {
                return finish_with_provider(oxios_home, config, provider);
            }
        }
    }

    // ── Step 1: Provider selection from oxi model DB ──
    let all_providers = oxi_sdk::get_providers();
    let visible: Vec<&str> = all_providers
        .iter()
        .copied()
        .filter(|p| !HIDDEN_PROVIDERS.contains(p))
        .collect();

    let provider = prompt_provider(&visible)?;
    finish_with_provider(oxios_home, config, provider)
}

// ── Core flow for a chosen provider ─────────────────────────────────────────

/// Complete onboarding for a given provider: key → model → workspace → write.
fn finish_with_provider(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider: &str,
) -> anyhow::Result<bool> {
    let mut api_key: Option<String> = None;
    let mut skip_key = false;

    // ── Check existing auth.json ──
    if let Ok(Some(token)) = oxi_sdk::load_token(provider) {
        if !token.access_token.is_empty() {
            println!();
            println!(
                "  Found existing credentials for '{}' in ~/.oxi/auth.json.",
                provider
            );
            if prompt_confirm("  Use them?", true) {
                skip_key = true;
            }
        }
    }

    // ── Step 2: API key ──
    if !skip_key && !NO_KEY_PROVIDERS.contains(&provider) {
        // Check if env var already has the key
        if let Some(env_key) = oxi_sdk::get_env_api_key(provider) {
            println!();
            println!("  Using {} key from environment.", provider);
            api_key = Some(env_key);
            skip_key = true;
        }

        if !skip_key {
            api_key = Some(prompt_api_key(provider)?);
        }
    }

    // ── Step 3: Model selection from oxi model DB ──
    let model = prompt_model(provider)?;

    // ── Step 4: Workspace ──
    let workspace = prompt_workspace()?;

    // ── Summary ──
    let key_preview = if skip_key {
        let key = api_key.as_deref().unwrap_or("(from auth store)");
        mask_key(key)
    } else {
        mask_key(api_key.as_deref().unwrap_or("(none)"))
    };

    if !confirm_summary(provider, &model, &key_preview, &workspace) {
        println!();
        println!("  Setup cancelled.");
        return Ok(false);
    }

    // ── Step 5: Write ──
    persist_config(
        oxios_home,
        config,
        provider,
        api_key.as_deref().unwrap_or(""),
        &model,
        &workspace,
    )?;
    print_success(oxios_home, &model);
    Ok(true)
}

// ── Prompt steps ─────────────────────────────────────────────────────────────

/// Step 1: Show providers from oxi model DB and let user pick one.
fn prompt_provider<'a>(providers: &[&'a str]) -> anyhow::Result<&'a str> {
    println!();
    println!("  [1/{}] Select an LLM provider:", TOTAL_STEPS);
    println!();

    // Show providers with "(key detected)" if env var is present
    for (i, provider) in providers.iter().enumerate() {
        let mut suffix = String::new();
        if oxi_sdk::has_env_key(provider) {
            suffix = " (key detected)".to_string();
        }
        let model_count = oxi_sdk::get_provider_models(provider).len();
        println!(
            "    {:>2}) {} [{} models]{}",
            i + 1,
            provider,
            model_count,
            suffix
        );
    }
    println!();

    loop {
        print!("  > ");
        io::stdout().flush()?;
        let input = read_line();
        // Accept number or provider name
        if let Ok(n) = input.trim().parse::<usize>() {
            if n >= 1 && n <= providers.len() {
                return Ok(providers[n - 1]);
            }
        }
        // Also accept provider name directly
        let name = input.trim();
        if let Some(&p) = providers.iter().find(|&&p| p == name) {
            return Ok(p);
        }
        println!(
            "  Enter a number between 1 and {}, or a provider name.",
            providers.len()
        );
    }
}

/// Step 2: Prompt for API key with retry.
fn prompt_api_key(provider: &str) -> anyhow::Result<String> {
    println!();
    println!("  [2/{}] Enter your {} API key:", TOTAL_STEPS, provider);
    loop {
        print!("  API key: ");
        io::stdout().flush()?;
        let input = read_line();
        let key = input.trim();
        if key.is_empty() {
            println!("  API key is required. (Ctrl+C to cancel)");
            continue;
        }
        return Ok(key.to_string());
    }
}

/// Step 3: Model selection from oxi model DB.
///
/// Shows models for the provider from the built-in catalog.
/// User picks by number or enters manually.
fn prompt_model(provider: &str) -> anyhow::Result<String> {
    let models = oxi_sdk::get_provider_models(provider);

    println!();
    println!("  [3/{}] Select a model for {}:", TOTAL_STEPS, provider);
    println!();

    if models.is_empty() {
        // Unknown provider — manual entry
        println!("  No built-in models for this provider.");
        print!("  Enter model ID: ");
        io::stdout().flush()?;
        let input = read_line();
        let model = input.trim().to_string();
        if model.is_empty() {
            anyhow::bail!("Model ID is required.");
        }
        return Ok(if model.contains('/') {
            model
        } else {
            format!("{}/{}", provider, model)
        });
    }

    // Show up to 8 models (skip duplicates with "latest" aliases — prefer dated versions)
    let mut shown = Vec::new();
    for entry in models.iter() {
        // Skip "latest" aliases to keep the list short
        if entry.name.contains("latest") {
            continue;
        }
        shown.push(entry);
        if shown.len() >= 8 {
            break;
        }
    }

    for (i, entry) in shown.iter().enumerate() {
        let ctx = if entry.context_window >= 1_000_000 {
            format!("{}M ctx", entry.context_window / 1_000_000)
        } else {
            format!("{}K ctx", entry.context_window / 1000)
        };
        let reasoning = if entry.reasoning { " reasoning" } else { "" };
        println!("    {:>2}) {:<40} {:>8}{}", i + 1, entry.name, ctx, reasoning);
    }
    let manual_idx = shown.len() + 1;
    println!("    {:>2}) Enter model ID manually", manual_idx);
    println!();

    loop {
        print!("  > ");
        io::stdout().flush()?;
        let input = read_line();
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= shown.len() => {
                let entry = shown[n - 1];
                return Ok(format!("{}/{}", provider, entry.id));
            }
            Ok(n) if n == manual_idx => {
                print!("  Model ID: ");
                io::stdout().flush()?;
                let manual = read_line();
                let model = manual.trim();
                if model.is_empty() {
                    println!("  Model ID cannot be empty.");
                    continue;
                }
                return Ok(if model.contains('/') {
                    model.to_string()
                } else {
                    format!("{}/{}", provider, model)
                });
            }
            _ => {
                println!(
                    "  Enter a number between 1 and {}.",
                    manual_idx
                );
                continue;
            }
        }
    }
}

/// Step 4: Workspace path.
fn prompt_workspace() -> anyhow::Result<String> {
    let default_workspace = dirs::home_dir()
        .map(|h| format!("{}/.oxios/workspace", h.display()))
        .unwrap_or_else(|| "~/.oxios/workspace".to_string());

    println!();
    println!("  [4/{}] Workspace path (Enter for default):", TOTAL_STEPS);
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

/// Step 5: Summary confirmation.
fn confirm_summary(
    provider: &str,
    model: &str,
    key_preview: &str,
    workspace: &str,
) -> bool {
    println!();
    println!("  ┌─────────────────────────────────────────────┐");
    println!("  │            Configuration Summary             │");
    println!("  ├─────────────────────────────────────────────┤");
    println!("  │  Provider:  {:<32}│", provider);
    println!("  │  Model:     {:<32}│", model);
    println!("  │  Key:       {:<32}│", key_preview);
    println!("  │  Workspace: {:<32}│", truncate_str(workspace, 32));
    println!("  └─────────────────────────────────────────────┘");
    println!();
    println!("  [5/{}] Write configuration?", TOTAL_STEPS);
    prompt_confirm("  >", true)
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// Write everything to disk.
fn persist_config(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider: &str,
    api_key: &str,
    model: &str,
    workspace: &str,
) -> anyhow::Result<()> {
    print!("\n  Saving configuration... ");
    io::stdout().flush()?;

    if !api_key.is_empty() {
        CredentialStore::store(provider, api_key)?;
    }

    std::fs::create_dir_all(workspace)?;
    for subdir in WORKSPACE_SUBDIRS {
        std::fs::create_dir_all(Path::new(workspace).join(subdir))?;
    }

    config.engine.default_model = model.to_string();
    config.kernel.workspace = workspace.to_string();
    write_config(oxios_home, config)?;

    println!("done");
    Ok(())
}

fn write_config(oxios_home: &Path, config: &OxiosConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(oxios_home)?;
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    std::fs::write(oxios_home.join("config.toml"), &toml_str)?;
    Ok(())
}

// ── UI ───────────────────────────────────────────────────────────────────────

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

// ── IO helpers ───────────────────────────────────────────────────────────────

fn read_line() -> String {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap_or_default();
    buf.trim_end().to_string()
}

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

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return key.to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
