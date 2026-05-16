//! Interactive first-run setup wizard.
//!
//! Detects existing credentials (oxi auth store, config.toml).
//! If none found, runs an interactive wizard that stores the API key
//! in `~/.oxi/auth.json` (shared with oxi CLI if installed).

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use std::io::{self, Write};

const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
    "workspace/programs",
];

/// Check if any credentials exist (config.toml, oxi auth store, or env vars).
/// Returns true if credentials are found (onboarding should be skipped).
pub fn has_credentials(config: &OxiosConfig) -> bool {
    let provider = CredentialStore::provider_from_model(&config.engine.default_model);
    CredentialStore::has_credential(provider, config.api_key().as_deref())
}

/// Run the onboarding wizard if no credentials are configured.
///
/// Returns `Ok(true)` if onboarding completed (new credentials stored).
/// Returns `Ok(false)` if skipped (already configured).
pub fn run_onboarding(
    oxios_home: &std::path::Path,
    config: &mut OxiosConfig,
) -> anyhow::Result<bool> {
    let provider = CredentialStore::provider_from_model(&config.engine.default_model);

    // Check if already configured
    if CredentialStore::has_credential(provider, config.api_key().as_deref()) {
        return Ok(false);
    }

    print_banner();
    print_intro();

    // 1. Ask provider
    let provider = prompt_provider()?;

    // 2. Check if oxi auth.json already has credentials for this provider
    if let Ok(Some(token)) = oxi_ai::oauth::load_token(provider) {
        if !token.access_token.is_empty() {
            println!();
            println!(
                "  ── Detected ~/.oxi/auth.json with '{}' credentials ──",
                provider
            );
            if prompt_bool("  Use existing credentials?", true) {
                // Update config with the provider's default model
                config.engine.default_model =
                    format!("{}/{}", provider, default_model_for(provider));
                write_config(oxios_home, config)?;
                print_success(oxios_home, &config.engine.default_model);
                return Ok(true);
            }
        }
    }

    // 3. Ask for API key
    print!("\n  Enter your {} API key: ", provider.to_uppercase());
    io::stdout().flush()?;
    let api_key = read_line();
    if api_key.trim().is_empty() {
        println!("  API key is required — setup cancelled.");
        return Ok(false);
    }

    // 4. Ask model
    let model_default = default_model_for(provider);
    print!("  Default model [{}]: ", model_default);
    io::stdout().flush()?;
    let model_input = read_line();
    let model = if model_input.trim().is_empty() {
        format!("{}/{}", provider, model_default)
    } else {
        format!("{}/{}", provider, model_input.trim())
    };

    // 5. Workspace
    let default_workspace = dirs::home_dir()
        .map(|h| format!("{}/.oxios/workspace", h.display()))
        .unwrap_or_else(|| "~/.oxios/workspace".to_string());
    print!("  Workspace [{}]: ", default_workspace);
    io::stdout().flush()?;
    let workspace = read_line();
    let workspace = if workspace.trim().is_empty() {
        default_workspace
    } else {
        workspace.trim().to_string()
    };
    let workspace = crate::config::expand_home(&workspace)
        .to_string_lossy()
        .to_string();

    // 6. Store credentials in ~/.oxi/auth.json
    print!("\n  Storing credentials... ");
    io::stdout().flush()?;
    CredentialStore::store(provider, api_key.trim())?;
    println!("done");

    // 7. Create workspace directories
    print!("  Creating workspace... ");
    io::stdout().flush()?;
    std::fs::create_dir_all(&workspace)?;
    for subdir in WORKSPACE_SUBDIRS {
        std::fs::create_dir_all(std::path::Path::new(&workspace).join(subdir))?;
    }
    println!("done");

    // 8. Update config
    config.engine.default_model = model;
    config.kernel.workspace = workspace;
    write_config(oxios_home, config)?;

    print_success(oxios_home, &config.engine.default_model);
    Ok(true)
}

fn default_model_for(provider: &str) -> &str {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514",
        "openai" => "gpt-4o",
        "google" => "gemini-2.0-flash",
        "deepseek" => "deepseek-chat",
        "groq" => "llama-3.3-70b-versatile",
        _ => "default",
    }
}

fn prompt_provider() -> anyhow::Result<&'static str> {
    println!();
    println!("  Select LLM provider:");
    println!("    1) Anthropic (Claude)");
    println!("    2) OpenAI");
    println!("    3) Google (Gemini)");
    println!("    4) DeepSeek");
    println!("    5) Groq");
    loop {
        print!("  Enter choice [1]: ");
        io::stdout().flush()?;
        let input = read_line();
        let choice = if input.trim().is_empty() {
            "1"
        } else {
            input.trim()
        };
        let provider = match choice {
            "1" => "anthropic",
            "2" => "openai",
            "3" => "google",
            "4" => "deepseek",
            "5" => "groq",
            _ => {
                println!("  Invalid choice — enter 1-5");
                continue;
            }
        };
        return Ok(provider);
    }
}

fn write_config(oxios_home: &std::path::Path, config: &OxiosConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(oxios_home)?;
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("failed to serialize config: {}", e))?;
    let config_path = oxios_home.join("config.toml");
    std::fs::write(&config_path, &toml_str)?;
    Ok(())
}

fn print_banner() {
    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║         ⬡  Oxios — First-Time Setup       ║");
    println!("  ╚═══════════════════════════════════════════╝");
}

fn print_intro() {
    println!();
    println!("  Welcome! This wizard configures your API credentials.");
    println!("  Press Ctrl+C at any time to cancel.");
}

fn print_success(oxios_home: &std::path::Path, model: &str) {
    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║             ✅  Setup Complete!            ║");
    println!("  ╚═══════════════════════════════════════════╝");
    println!();
    println!("    Config:  {}", oxios_home.join("config.toml").display());
    println!("    Model:   {}", model);
    println!();
    println!("  Next steps:");
    println!("    oxios              → start the daemon");
    println!("    oxios daemon install → install as system service");
    println!("    open http://127.0.0.1:4200");
    println!();
}

fn prompt_bool(prompt: &str, default: bool) -> bool {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    print!("{} {}: ", prompt, suffix);
    io::stdout().flush().unwrap_or_default();
    let input = read_line();
    if input.trim().is_empty() {
        return default;
    }
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn read_line() -> String {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap_or_default();
    buf.trim_end().to_string()
}
