//! Web UI dist directory resolution and auto-download.
//!
//! This module runs before surfaces start to ensure the web UI is available.
//! If `~/.oxios/web/dist/index.html` doesn't exist, it downloads from GitHub Releases.
//!
//! The resolved path is passed to surfaces via [`SurfaceContext.web_dist`].
//! This avoids the race condition where the server starts listening before
//! the web UI is downloaded.

use anyhow::{Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

const GITHUB_REPO: &str = "a7garden/oxios";

/// Returns `~/.oxios/web/` (parent of the dist directory).
fn user_web_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxios").join("web"))
}

/// Returns `~/.oxios/web/dist/` path.
fn user_web_dist_dir() -> Option<PathBuf> {
    user_web_root().map(|r| r.join("dist"))
}

/// Returns the path to the active-dist marker file (`~/.oxios/web/.active`).
///
/// RFC-024 SP3: persists the path the in-memory atomic pointer last pointed
/// at, so a daemon restart resolves the same generation the previous process
/// was serving (the pointer itself does not survive restart).
pub fn active_marker_path() -> Option<PathBuf> {
    user_web_root().map(|r| r.join(".active"))
}

/// Result of ensuring web UI availability.
#[derive(Debug)]
pub enum WebDistResult {
    /// Web UI found in `~/.oxios/web/dist/`.
    UserDir(PathBuf),
    /// Web UI found in `workspace/web/dist/`.
    WorkspaceDir(PathBuf),
    /// Downloaded from GitHub Releases.
    Downloaded { path: PathBuf, version: String },
    /// No filesystem web UI — embedded assets will be used.
    ///
    /// Reserved for future use when the binary is built with `rust-embed`.
    /// Currently not constructed by `ensure_web_dist` (downloaded dist is preferred)
    /// but exposed so callers can match exhaustively.
    #[allow(dead_code)]
    Embedded,
    /// Download failed — embedded assets will be used as fallback.
    DownloadFailed { reason: String },
}

impl WebDistResult {
    /// Returns the version tag without the leading 'v' prefix (for display).
    pub fn version_display(&self) -> Option<&str> {
        match self {
            WebDistResult::Downloaded { version, .. } => Some(version.trim_start_matches('v')),
            _ => None,
        }
    }
}

/// Format bytes into human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Fetches the latest release tag from GitHub API.
async fn fetch_latest_release_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("oxios-web")
        .build()
        .context("failed to create HTTP client")?;
    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("failed to fetch GitHub release info")?
        .json()
        .await
        .context("failed to parse GitHub response")?;
    let tag = resp["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("tag_name not found in GitHub response"))?;
    Ok(tag.to_string())
}

/// Extract a `web-dist.zip` byte slice into `dest` (created if missing).
///
/// RFC-024 SP3: shared extraction used by both the startup download and the
/// daily health check so both land in a staging dir before an atomic publish.
/// Returns the number of files extracted. `dest` is cleared first if it
/// already exists (e.g. an interrupted prior run).
pub fn extract_zip_into(dest: &std::path::Path, bytes: &[u8]) -> Result<usize> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    std::fs::create_dir_all(dest)?;

    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).context("invalid zip file")?;
    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("zip read error")?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest.join(path),
            None => continue,
        };
        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent()
                && !p.exists()
            {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Path for a versioned staging directory under `~/.oxios/web/`.
pub fn staging_dir_for(version_tag: &str) -> Option<PathBuf> {
    let id = version_tag.trim_start_matches('v');
    user_web_root().map(|r| r.join(format!("dist-{id}")))
}

/// Downloads `web-dist.zip` from a GitHub release and extracts it into a
/// **fresh, versioned staging directory** (`~/.oxios/web/dist-<version>/`).
///
/// RFC-024 SP3: never deletes the canonical `dist/` here — the caller
/// publishes the staging dir atomically via the in-memory pointer + marker
/// so concurrent requests never observe a half-extracted directory.
async fn download_and_extract_web_dist(version_tag: &str) -> Result<PathBuf> {
    let web_root =
        user_web_root().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    // Versioned dir so multiple generations can coexist during the swap.
    let version_id = version_tag.trim_start_matches('v');
    let dist_dir = web_root.join(format!("dist-{version_id}"));

    let url =
        format!("https://github.com/{GITHUB_REPO}/releases/download/{version_tag}/web-dist.zip");

    let client = reqwest::Client::builder()
        .user_agent("oxios-web")
        .build()
        .context("failed to create HTTP client")?;

    // ── Download with progress bar ─────────────────────────────────────────
    let resp = client
        .get(&url)
        .send()
        .await
        .context("download request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Failed to download web-dist.zip: HTTP {}", resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner} {msg}  [{bar:>.dim}] {bytes}/{total_bytes} ({bytes_per_sec})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    let tag_label = style(version_tag).cyan().to_string();
    pb.set_message(format!("Downloading web UI {tag_label}"));

    let bytes = resp.bytes().await.context("failed to read response body")?;

    let ok = style("✓").green().to_string();
    let downloaded = style("Downloaded").green().to_string();
    let done_msg = format!(
        "  {} {} ({})",
        ok,
        downloaded,
        format_size(bytes.len() as u64)
    );
    pb.finish_with_message(done_msg);

    // ── Extract with progress ─────────────────────────────────────────────
    let reader = std::io::Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(reader).context("invalid zip file")?;
    let file_count = archive.len();

    let extract_pb = ProgressBar::new(file_count as u64);
    extract_pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner} {msg}  [{bar:>.dim}] {pos}/{len}")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    extract_pb.set_message("Extracting files".to_string());

    // Clear any pre-existing staging dir for this exact version (interrupted
    // prior run), then create fresh. The canonical `dist/` is left untouched.
    if dist_dir.exists() {
        std::fs::remove_dir_all(&dist_dir)?;
    }
    std::fs::create_dir_all(&dist_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dist_dir.join(path),
            None => continue,
        };
        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent()
                && !p.exists()
            {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
        extract_pb.inc(1);
    }

    let ok = style("✓").green().to_string();
    let done_msg = format!("  {ok} {file_count} files extracted");
    extract_pb.finish_with_message(done_msg);

    tracing::info!(
        path = ?dist_dir,
        version = %version_tag,
        "Web UI downloaded and extracted"
    );

    Ok(dist_dir)
}

/// Ensures the web UI is available, downloading from GitHub if needed.
///
/// Resolution order (RFC-024 SP3, marker-aware):
///  1. `~/.oxios/web/.active` marker → generation last served (survives restart)
///  2. `~/.oxios/web/dist/index.html` — legacy / user override
///  3. `workspace/web/dist/index.html` — bundled / dev mode
///  4. Download from GitHub Releases into a fresh versioned staging dir,
///     then publish via marker so restarts resolve it.
///
/// Returns a [`WebDistResult`] describing what happened. The returned path
/// is the directory the caller should publish as the active dist.
pub async fn ensure_web_dist(workspace: &Path) -> WebDistResult {
    let marker = active_marker_path();
    let legacy = user_web_dist_dir();

    // 1. Marker (RFC-024): generation the previous process was serving.
    if let Some(m) = marker.as_ref()
        && let Some(p) = oxios_gateway::ActiveWebDist::resolve(m, legacy.as_deref())
    {
        tracing::info!(path = ?p, "Serving web UI from active marker");
        return WebDistResult::UserDir(p);
    }

    // 2. ~/.oxios/web/dist/ (legacy / user override)
    if let Some(ref dist) = legacy
        && dist.join("index.html").is_file()
    {
        tracing::info!(path = ?dist, "Serving web UI from ~/.oxios/web/dist/");
        return WebDistResult::UserDir(dist.clone());
    }

    // 3. workspace/web/dist/ (bundled / dev)
    let workspace_dist = workspace.join("web").join("dist");
    if workspace_dist.join("index.html").is_file() {
        tracing::info!(path = ?workspace_dist, "Serving web UI from workspace (web/dist/)");
        return WebDistResult::WorkspaceDir(workspace_dist);
    }

    // 4. Auto-download from GitHub Releases (with bounded retry so a transient
    //    network blip or rate-limit doesn't strand the daemon serving 503
    //    until a manual `oxios update --web-only`). Each attempt retries the
    //    full tag-lookup + download pair.
    tracing::info!("No web UI found locally, downloading from GitHub Releases...");
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_reason = String::from("unknown error");
    for attempt in 1..=MAX_ATTEMPTS {
        let outcome = match fetch_latest_release_tag().await {
            Ok(tag) => match download_and_extract_web_dist(&tag).await {
                Ok(path) => Some((tag, path)),
                Err(e) => {
                    last_reason = e.to_string();
                    None
                }
            },
            Err(e) => {
                last_reason = e.to_string();
                None
            }
        };

        if let Some((tag, path)) = outcome {
            // Publish the freshly-extracted staging dir so restarts
            // resolve it via the marker.
            if let Some(m) = marker.as_ref() {
                let _ = std::fs::write(m, path.to_string_lossy().as_bytes());
            }
            return WebDistResult::Downloaded { path, version: tag };
        }

        if attempt < MAX_ATTEMPTS {
            tracing::warn!(
                attempt,
                max = MAX_ATTEMPTS,
                reason = %last_reason,
                "Web UI download failed, retrying"
            );
            tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
        } else {
            tracing::warn!(reason = %last_reason, "Web UI download failed (no retries left)");
        }
    }
    WebDistResult::DownloadFailed {
        reason: last_reason,
    }
}
