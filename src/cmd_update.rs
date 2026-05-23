//! `oxios update` — fetch binary and/or web UI from GitHub Releases.
//!
//! Release assets:
//!   - Binary: `oxios-macos-arm64` (macOS ARM64) with `.sha256` checksum
//!   - Web UI:  `web-dist.zip` (contains index.html + assets/)
//!
//! Installation paths:
//!   - Binary: `~/.cargo/bin/oxios` (overwrites current binary)
//!   - Web UI: `~/.oxios/web/dist/` (extracted over existing)

use anyhow::{Context, Result};
use console::style;
use sha2::Digest;
use std::io::Write;
use std::path::PathBuf;

/// Update oxios binary and/or web UI from GitHub Releases.
pub async fn run_update(
    web_only: bool,
    binary_only: bool,
    version: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    // ── Determine what to update ────────────────────────────────────────────
    let update_binary = !web_only;
    let update_web = !binary_only;

    println!();
    println!("  {} {}", style("⬡ Oxios Updater").bold(), style(format!("v{}", current)).dim());
    println!("  {}", "─".repeat(52));
    println!("  Current version:  {}", current);
    println!("  Update binary:    {}", if update_binary { "yes" } else { "no" });
    println!("  Update web UI:   {}", if update_web { "yes" } else { "no" });
    if let Some(v) = version {
        println!("  Target version:  {}", v);
    } else {
        println!("  Target version:  latest");
    }
    println!();

    // ── Fetch release info from GitHub ──────────────────────────────────────
    let owner = "a7garden";
    let repo = "oxios";
    let tag = version.map(|v| format!("v{}", v));

    let api_url = match &tag {
        Some(t) => format!(
            "https://api.github.com/repos/{}/{}/releases/tags/{}",
            owner, repo, t
        ),
        None => format!("https://api.github.com/repos/{}/{}/releases/latest", owner, repo),
    };

    println!("  Fetching release info from GitHub...");
    let client = reqwest::Client::builder()
        .user_agent("oxios/0.3")
        .build()
        .context("failed to create HTTP client")?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .context("failed to fetch release info (check network/GITHUB_TOKEN)")?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "GitHub API error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let release: serde_json::Value = resp.json().await.context("failed to parse release JSON")?;

    let tag_name = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .trim_start_matches('v');
    let html_url = release["html_url"].as_str().unwrap_or("");
    let body = release["body"].as_str().unwrap_or("No release notes.");

    println!("  Latest release:  {} ({})", style(tag_name).green().bold(), html_url);
    println!();

    if tag_name == current && !dry_run && !yes {
        println!("  {} Already on latest version ({}).", style("✓").green(), current);
        println!("  Use `--version X.Y.Z` to force a specific version.");
        return Ok(());
    }

    // ── Parse assets ─────────────────────────────────────────────────────────
    let assets: Vec<(String, String, u64)> = release["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|a| {
            Some((
                a["name"].as_str()?.to_string(),
                a["browser_download_url"].as_str()?.to_string(),
                a["size"].as_u64()?,
            ))
        })
        .collect();

    let web_asset = assets.iter().find(|(name, _, _)| name == "web-dist.zip");
    let binary_asset = assets.iter().find(|(name, _, _)| name == "oxios-macos-arm64");
    let checksum_asset = assets.iter().find(|(name, _, _)| name == "oxios-macos-arm64.sha256");

    println!("  Available assets:");
    for (name, _, size) in &assets {
        println!("    {}  ({} bytes)", name, format_size(*size));
    }
    println!();

    // ── Dry run ──────────────────────────────────────────────────────────────
    if dry_run {
        println!("  {} Dry run — no changes made.\n", style("⚠").yellow());
        if update_web {
            if let Some((name, _, size)) = web_asset {
                println!("  Would download: {} ({})", name, format_size(*size));
                println!("  Would extract to: ~/.oxios/web/dist/");
            } else {
                println!("  {} web-dist.zip not found in release.", style("✗").red());
            }
        }
        if update_binary {
            if let Some((name, _, size)) = binary_asset {
                println!("  Would download: {} ({})", name, format_size(*size));
                println!("  Would install to: ~/.cargo/bin/oxios");
            } else {
                println!("  {} oxios-macos-arm64 not found in release.", style("✗").red());
            }
        }
        return Ok(());
    }

    // ── Confirmation ─────────────────────────────────────────────────────────
    if !yes {
        println!("  {} Release notes:\n", style("📋").cyan());
        for line in body.lines().take(10) {
            println!("    {}", line);
        }
        if body.lines().count() > 10 {
            println!("    ... ({} more lines)", body.lines().count() - 10);
        }
        println!();

        print!("  Continue with update? [y/N] ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  Update cancelled.");
            return Ok(());
        }
    }

    // ── Download and install web UI ────────────────────────────────────────
    if update_web {
        if let Some((name, url, size)) = web_asset {
            print!("  Downloading {}... ", name);
            std::io::stdout().flush().ok();

            let bytes = download_file(&client, url, *size).await?;
            println!("done ({}).", format_size(bytes.len() as u64));

            let dest_dir = dest_web_dir()?;
            std::fs::create_dir_all(&dest_dir)
                .context(format!("failed to create {:?}", dest_dir))?;

            print!("  Extracting to {:?}... ", dest_dir);
            std::io::stdout().flush().ok();

            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor)
                .context("invalid zip file")?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let out_path = dest_dir.join(file.name());

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(p) = out_path.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    let mut out_file = std::fs::File::create(&out_path)?;
                    std::io::copy(&mut file, &mut out_file)?;
                }
            }
            println!("done.");
            println!("  {} Web UI updated to {} in {:?}.", style("✓").green(), tag_name, dest_dir);
        } else {
            println!("  {} web-dist.zip not found — skipping.", style("⚠").yellow());
        }
    }

    // ── Download and install binary ─────────────────────────────────────────
    if update_binary {
        if let Some((name, url, size)) = binary_asset {
            let expected_checksum = if let Some((cs_name, cs_url, _)) = checksum_asset {
                print!("  Verifying checksum from {}... ", cs_name);
                std::io::stdout().flush().ok();
                let cs_bytes = download_file(&client, cs_url, 256).await?;
                String::from_utf8_lossy(&cs_bytes)
                    .trim()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };

            print!("  Downloading {}... ", name);
            std::io::stdout().flush().ok();

            let bytes = download_file(&client, url, *size).await?;
            println!("done ({}).", format_size(bytes.len() as u64));

            if !expected_checksum.is_empty() {
                let digest = sha2::Sha256::digest(&bytes);
                let actual = format!("{:x}", digest);
                if actual != expected_checksum {
                    anyhow::bail!(
                        "Checksum mismatch!\n  Expected: {}\n  Actual:   {}",
                        expected_checksum, actual
                    );
                }
                println!("  {} Checksum verified.", style("✓").green());
            }

            let dest = dest_binary_path()?;
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(&dest, &bytes)
                .context(format!("failed to write binary to {:?}", dest))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest, perms)?;
            }

            println!("  {} Binary updated to {} at {:?}", style("✓").green(), tag_name, dest);
            println!();
            println!("  {} Run `{}` to restart with the new binary.",
                style("ℹ").cyan(), style("oxios restart").bold());
        } else {
            println!("  {} oxios-macos-arm64 not found — skipping.", style("⚠").yellow());
        }
    }

    println!();
    Ok(())
}

/// Show changelog / release notes for a given version (or latest).
pub async fn run_changelog(version: Option<&str>) -> Result<()> {
    let owner = "a7garden";
    let repo = "oxios";
    let api_url = match version {
        Some(v) => format!(
            "https://api.github.com/repos/{}/{}/releases/tags/v{}",
            owner, repo, v
        ),
        None => format!("https://api.github.com/repos/{}/{}/releases/latest", owner, repo),
    };

    let client = reqwest::Client::builder()
        .user_agent("oxios/0.3")
        .build()
        .context("failed to create HTTP client")?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .context("failed to fetch release info")?;

    if !resp.status().is_success() {
        anyhow::bail!("Release not found: {}", resp.status());
    }

    let release: serde_json::Value = resp.json().await.context("failed to parse release JSON")?;
    let tag = release["tag_name"].as_str().unwrap_or("?").trim_start_matches('v');
    let body = release["body"].as_str().unwrap_or("(no release notes)");
    let date = release["published_at"].as_str().unwrap_or("?");

    println!();
    println!("  {} v{}  ({})", style("⬡ Oxios").bold(), style(tag).green().bold(), date);
    println!("  {}", "─".repeat(55));
    println!();
    println!("{}", body);
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dest_web_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".oxios").join("web").join("dist"))
}

fn dest_binary_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".cargo").join("bin").join("oxios"))
}

async fn download_file(client: &reqwest::Client, url: &str, _expected_size: u64) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .context("download request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: {}", resp.status());
    }

    let bytes = resp
        .bytes()
        .await
        .context("failed to read response body")?;

    Ok(bytes.to_vec())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}