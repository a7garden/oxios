//! Provisioner (RFC-041 Phase 4).
//!
//! Executes [`SkillInstallSpec`]s as a **privileged kernel operation** — the
//! same pattern the clawhub/skills_sh installers use (`tokio::process::Command`
//! directly), NOT the agent-sandbox `ExecTool`. Per D8, `ExecTool`'s allowlist
//! and metacharacter blocking defend against *untrusted agent command strings*;
//! here the command is kernel-constructed from a trusted registry spec, so those
//! defenses are redundant. The security gate is user consent at the API layer
//! + audit logging + registry-derived command (no free text → no injection).
//!
//! Covers the package-manager install kinds (`brew`/`node`/`bun`/`cargo`/
//! `pip`/`go`/`uv`). The `download` kind fetches an archive and extracts it.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;
use tokio::process::Command;

use crate::skill::{InstallKind, SkillInstallSpec};

/// Outcome of an install attempt.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutput {
    /// Whether the command exited successfully.
    pub success: bool,
    /// Human-readable description of what was run, e.g. `brew install gh`.
    pub command: String,
    /// Combined stdout/stderr (truncated for log surface).
    pub output: String,
    /// Exit code, when available.
    pub exit_code: Option<i32>,
}

/// Build the command line for a single spec (no spawn). Used by dry-run + tests.
///
/// Returns `None` when the spec lacks the field its kind needs (e.g. a `brew`
/// spec without `formula`) — the provisioner skips such specs.
pub fn build_command(spec: &SkillInstallSpec) -> Option<(String, Vec<String>)> {
    let (bin, args): (&str, Vec<String>) = match spec.kind {
        InstallKind::Brew => {
            let formula = spec
                .formula
                .as_deref()
                .context("brew spec missing formula")
                .ok()?;
            ("brew", vec!["install".into(), formula.into()])
        }
        InstallKind::Node => {
            let pkg = spec
                .package
                .as_deref()
                .context("node spec missing package")
                .ok()?;
            ("npm", vec!["install".into(), "-g".into(), pkg.into()])
        }
        InstallKind::Bun => {
            let pkg = spec
                .package
                .as_deref()
                .context("bun spec missing package")
                .ok()?;
            ("bun", vec!["install".into(), "-g".into(), pkg.into()])
        }
        InstallKind::Cargo => {
            let pkg = spec
                .package
                .as_deref()
                .context("cargo spec missing package")
                .ok()?;
            ("cargo", vec!["install".into(), pkg.into()])
        }
        InstallKind::Pip => {
            let pkg = spec
                .package
                .as_deref()
                .context("pip spec missing package")
                .ok()?;
            ("pip", vec!["install".into(), "--user".into(), pkg.into()])
        }
        InstallKind::Go => {
            let module = spec
                .module
                .as_deref()
                .context("go spec missing module")
                .ok()?;
            ("go", vec!["install".into(), module.into()])
        }
        InstallKind::Uv => {
            let pkg = spec
                .package
                .as_deref()
                .context("uv spec missing package")
                .ok()?;
            ("uv", vec!["tool".into(), "install".into(), pkg.into()])
        }
        InstallKind::Download => {
            // Download is fetch+extract, not a single command — handled in
            // `install_download`. No command line here.
            return None;
        }
    };
    Some((bin.to_string(), args))
}

/// Format a command for display.
fn fmt_cmd(bin: &str, args: &[String]) -> String {
    let mut s = String::from(bin);
    for a in args {
        s.push(' ');
        s.push_str(a);
    }
    s
}

/// Run a package-manager install spec. Spawns the command directly (D8).
pub async fn install_manager(spec: &SkillInstallSpec) -> Result<InstallOutput> {
    let (bin, args) = build_command(spec)
        .context("install spec is incomplete (missing formula/package/module)")?;
    let cmd_str = fmt_cmd(&bin, &args);
    tracing::info!(command = %cmd_str, "provisioning: running install");

    let output = Command::new(&bin)
        .args(&args)
        .output()
        .await
        .with_context(|| format!("failed to spawn `{cmd_str}`"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stderr.is_empty() {
        stdout
    } else if stdout.is_empty() {
        stderr
    } else {
        format!("{stdout}\n--- stderr ---\n{stderr}")
    };
    let success = output.status.success();
    Ok(InstallOutput {
        success,
        command: cmd_str,
        output: combined,
        exit_code: output.status.code(),
    })
}

/// Fetch a `download` spec's archive and extract it into `target_dir`.
///
/// Honors `strip_components`. Uses reqwest + tar/flate2 (same crates the
/// clawhub installer relies on). Returns an [`InstallOutput`] describing the
/// result.
pub async fn install_download(spec: &SkillInstallSpec) -> Result<InstallOutput> {
    let url = spec.url.as_deref().context("download spec missing url")?;
    let target = spec
        .target_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let strip = spec.strip_components.unwrap_or(0) as usize;

    tracing::info!(url = %url, target = %target.display(), "provisioning: downloading archive");
    let resp = reqwest::get(url)
        .await
        .context("fetching download archive")?;
    let bytes = resp
        .bytes()
        .await
        .context("reading download archive body")?;

    // Determine archive type by extension and extract.
    std::fs::create_dir_all(&target).with_context(|| format!("creating {}", target.display()))?;
    let is_gz = url.ends_with(".gz") || url.ends_with(".tgz");
    let is_tar = url.ends_with(".tar") || url.ends_with(".tar.gz") || url.ends_with(".tgz");

    let archive_bytes: &[u8] = &bytes;
    let decoder;
    if is_tar && is_gz {
        decoder = flate2::read::GzDecoder::new(archive_bytes);
        let mut tar = tar::Archive::new(decoder);
        tar.set_overwrite(true);
        unpack_with_strip(&mut tar, &target, strip)?;
    } else if is_tar {
        let mut tar = tar::Archive::new(archive_bytes);
        tar.set_overwrite(true);
        unpack_with_strip(&mut tar, &target, strip)?;
    } else {
        // Plain file: write the (possibly gunzipped) bytes to target/<basename>.
        let name = url.rsplit('/').next().unwrap_or("download");
        std::fs::write(target.join(name), &bytes).context("writing downloaded file")?;
        let _ = archive_bytes; // silence unused borrow on non-tar path
    }

    Ok(InstallOutput {
        success: true,
        command: format!("download+extract {url} → {}", target.display()),
        output: format!("extracted to {}", target.display()),
        exit_code: None,
    })
}

/// Unpack a tar archive, dropping the first `strip` path components of each entry.
fn unpack_with_strip<R: std::io::Read>(
    tar: &mut tar::Archive<R>,
    target: &std::path::Path,
    strip: usize,
) -> Result<()> {
    for entry in tar.entries()? {
        let mut entry = entry.context("reading tar entry")?;
        let path = entry.path()?.into_owned();
        let mut comps = path.components();
        for _ in 0..strip {
            comps.next();
        }
        let stripped: PathBuf = comps.collect();
        if stripped.as_os_str().is_empty() {
            continue;
        }
        // Security: reject absolute / parent-traversal paths.
        if stripped.is_absolute() || stripped.components().any(|c| c.as_os_str() == "..") {
            anyhow::bail!("unsafe entry path in archive: {}", stripped.display());
        }
        let entry_type = entry.header().entry_type();
        anyhow::ensure!(
            entry_type.is_file() || entry_type.is_dir(),
            "unsupported archive entry type for {}",
            stripped.display()
        );
        let destination = target.join(&stripped);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        entry
            .unpack(&destination)
            .with_context(|| format!("unpacking {} to {}", stripped.display(), target.display()))?;
    }
    Ok(())
}

/// Run the first applicable spec from a list (the first whose kind's manager is
/// available, else the first spec). Package-manager specs spawn directly;
/// `download` specs fetch+extract.
pub async fn install(specs: &[SkillInstallSpec]) -> Result<InstallOutput> {
    anyhow::ensure!(!specs.is_empty(), "no install specs provided");
    // Prefer the first spec that has a manager binary on PATH, else take [0].
    let pick = specs
        .iter()
        .find(|s| manager_available(s.kind))
        .or_else(|| specs.first())
        .context("no install spec")?;
    match pick.kind {
        InstallKind::Download => install_download(pick).await,
        _ => install_manager(pick).await,
    }
}

/// Whether the package manager for an install kind is detectable on PATH.
fn manager_available(kind: InstallKind) -> bool {
    let bin = match kind {
        InstallKind::Brew => "brew",
        InstallKind::Node => "npm",
        InstallKind::Bun => "bun",
        InstallKind::Cargo => "cargo",
        InstallKind::Pip => "pip3",
        InstallKind::Go => "go",
        InstallKind::Uv => "uv",
        InstallKind::Download => return true, // uses reqwest, no manager needed
    };
    which_sync(bin).is_some()
}

/// Sync PATH lookup (private — mirrors scanner's bootstrap check).
fn which_sync(name: &str) -> Option<PathBuf> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    let path = std::env::var("PATH").ok()?;
    for dir in path.split(sep) {
        let p = PathBuf::from(dir);
        if cfg!(windows) {
            for ext in ["exe", "cmd", "bat"] {
                let c = p.join(format!("{name}.{ext}"));
                if c.is_file() {
                    return Some(c);
                }
            }
        } else if p.join(name).is_file() {
            return Some(p.join(name));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(
        kind: InstallKind,
        formula: Option<&str>,
        package: Option<&str>,
        module: Option<&str>,
    ) -> SkillInstallSpec {
        SkillInstallSpec {
            kind,
            formula: formula.map(String::from),
            package: package.map(String::from),
            module: module.map(String::from),
            url: None,
            archive: None,
            extract: None,
            strip_components: None,
            target_dir: None,
            os: vec![],
        }
    }

    #[test]
    fn builds_brew_command() {
        let s = spec(InstallKind::Brew, Some("gh"), None, None);
        let (bin, args) = build_command(&s).unwrap();
        assert_eq!(bin, "brew");
        assert_eq!(args, vec!["install", "gh"]);
    }

    #[test]
    fn builds_cargo_command() {
        let s = spec(InstallKind::Cargo, None, Some("ripgrep"), None);
        let (bin, args) = build_command(&s).unwrap();
        assert_eq!(bin, "cargo");
        assert_eq!(args, vec!["install", "ripgrep"]);
    }

    #[test]
    fn builds_bun_command() {
        let s = spec(InstallKind::Bun, None, Some("tsx"), None);
        let (bin, args) = build_command(&s).unwrap();
        assert_eq!(bin, "bun");
        assert_eq!(args, vec!["install", "-g", "tsx"]);
    }

    #[test]
    fn incomplete_spec_returns_none() {
        // brew without formula → None
        let s = spec(InstallKind::Brew, None, None, None);
        assert!(build_command(&s).is_none());
    }

    #[test]
    fn download_has_no_command() {
        let s = spec(InstallKind::Download, None, None, None);
        assert!(build_command(&s).is_none());
    }

    #[test]
    fn unpack_applies_strip_components() {
        use std::io::Cursor;

        let mut builder = tar::Builder::new(Vec::new());
        let contents = b"hello";
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "tool-v1/bin/tool", Cursor::new(contents))
            .unwrap();
        let bytes = builder.into_inner().unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let mut archive = tar::Archive::new(Cursor::new(bytes));
        unpack_with_strip(&mut archive, tmp.path(), 1).unwrap();

        assert_eq!(
            std::fs::read(tmp.path().join("bin/tool")).unwrap(),
            contents
        );
        assert!(!tmp.path().join("tool-v1/bin/tool").exists());
    }

    #[test]
    fn fmt_cmd_joins_args() {
        assert_eq!(
            fmt_cmd("brew", &["install".into(), "gh".into()]),
            "brew install gh"
        );
    }
}
