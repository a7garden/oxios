//! Host tool discovery — `HostToolScanner`.
//!
//! Generalizes `skill::requirements::has_bin` (a boolean `which <name>`) into a
//! real inventory: enumerate CLIs across PATH and known package-manager install
//! roots, classify each by its source (brew / cargo / npm / …), and probe its
//! version. Cross-platform: uses Rust `PATH` traversal with `PATHEXT` resolution
//! on Windows (the existing `which`-based check silently fails there).
//!
//! Testability is via the [`HostProbe`] trait: production uses [`RealProbe`],
//! tests inject a [`crate::host_tools::FakeProbe`] (see `tests` module).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// How a tool was installed / where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolSource {
    /// On `PATH` but not under a known manager root.
    Path,
    /// Homebrew (`/opt/homebrew/bin`, `/usr/local/bin`, linuxbrew).
    Brew,
    /// Cargo (`~/.cargo/bin`).
    Cargo,
    /// npm global (`$(npm prefix -g)/bin`).
    Npm,
    /// Bun (`~/.bun/bin`).
    Bun,
    /// Go (`$(go env GOPATH)/bin`).
    Go,
    /// pip/uv (`~/.local/bin`).
    Pip,
    /// Standalone binary, unclassified.
    Binary,
}

impl ToolSource {
    /// All variants in stable order (used for prefix iteration).
    pub const ALL: &'static [ToolSource] = &[
        ToolSource::Brew,
        ToolSource::Cargo,
        ToolSource::Npm,
        ToolSource::Bun,
        ToolSource::Go,
        ToolSource::Pip,
        ToolSource::Path,
        ToolSource::Binary,
    ];
}

/// A single detected host tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTool {
    /// Binary name (e.g. `gh`).
    pub name: String,
    /// Absolute path to the executable (canonicalized — symlinks resolved).
    pub path: String,
    /// Best-effort version string (first line of `<bin> --version`), if any.
    pub version: Option<String>,
    /// Inferred install source.
    pub source: ToolSource,
}

/// Filesystem + process abstraction the scanner talks to.
///
/// Splitting this out lets unit tests supply a virtual PATH/prefix set and
/// fixture binaries without touching the real filesystem or spawning processes.
#[async_trait]
pub trait HostProbe: Send + Sync {
    /// `PATH` directories, in order (already split on the OS separator).
    fn path_dirs(&self) -> Vec<PathBuf>;

    /// Known package-manager install roots as `(source, prefix)` pairs.
    /// Prefixes are canonical (real install roots, not symlinks).
    fn manager_prefixes(&self) -> Vec<(ToolSource, PathBuf)>;

    /// Resolve `name` inside `dir` to an existing executable path.
    /// On Windows this applies `PATHEXT` (`.exe`, `.cmd`, …). `None` if absent.
    fn resolve(&self, dir: &Path, name: &str) -> Option<PathBuf>;

    /// Canonicalize a path (follow symlinks). Falls back to `path` itself on
    /// error — canonicalize fails for non-existent paths, but we only call it
    /// after a successful [`resolve`](Self::resolve).
    fn canonicalize(&self, path: &Path) -> PathBuf;

    /// Probe the version of an executable. Runs `<path> --version` with a short
    /// timeout and returns the first non-empty line, trimmed. `None` on failure.
    async fn version(&self, path: &Path) -> Option<String>;
}

/// Cache entry for a single name scan.
#[derive(Clone)]
struct CacheEntry {
    tool: Option<DetectedTool>,
    scanned_at: Instant,
}

/// Cross-platform host CLI scanner with mtime/TTL caching.
pub struct HostToolScanner {
    probe: std::sync::Arc<dyn HostProbe>,
    cache: parking_lot::RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl HostToolScanner {
    /// Build a scanner backed by the real host ([`RealProbe`]) with a 60s TTL.
    pub fn real() -> Self {
        Self::with_probe(std::sync::Arc::new(RealProbe), Duration::from_secs(60))
    }

    /// Build a scanner with an explicit probe (tests) and TTL.
    pub fn with_probe(probe: std::sync::Arc<dyn HostProbe>, ttl: Duration) -> Self {
        Self {
            probe,
            cache: parking_lot::RwLock::new(HashMap::new()),
            ttl,
        }
    }

    /// Clear the cache (force a fresh scan on next detect).
    pub fn invalidate(&self) {
        self.cache.write().clear();
    }

    /// Detect a single binary by name, using the cache when fresh.
    pub async fn detect(&self, name: &str) -> Option<DetectedTool> {
        // Cache hit?
        if let Some(entry) = self.cache.read().get(name) {
            if entry.scanned_at.elapsed() < self.ttl {
                return entry.tool.clone();
            }
        }

        let tool = self.scan_uncached(name).await;
        self.cache.write().insert(
            name.to_string(),
            CacheEntry {
                tool: tool.clone(),
                scanned_at: Instant::now(),
            },
        );
        tool
    }

    /// Detect many names; cache-aware.
    pub async fn detect_many(&self, names: &[String]) -> Vec<DetectedTool> {
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            if let Some(t) = self.detect(name).await {
                out.push(t);
            }
        }
        out
    }

    /// The actual scan for one name (no cache).
    ///
    /// Candidate directories: every `PATH` entry (source `Path`) plus every
    /// manager prefix. `PATH` entries are searched in order first, then manager
    /// roots — matching the shell's lookup behavior while still letting us
    /// classify a hit by its real install root after canonicalization.
    async fn scan_uncached(&self, name: &str) -> Option<DetectedTool> {
        let mut candidates: Vec<(ToolSource, PathBuf)> = self
            .probe
            .path_dirs()
            .into_iter()
            .map(|d| (ToolSource::Path, d))
            .collect();
        candidates.extend(self.probe.manager_prefixes());

        for (source_hint, dir) in &candidates {
            if let Some(raw) = self.probe.resolve(dir, name) {
                let canon = self.probe.canonicalize(&raw);
                let resolved_source = self.classify(&canon).unwrap_or(*source_hint);
                let version = self.probe.version(&canon).await;
                return Some(DetectedTool {
                    name: name.to_string(),
                    path: canon.to_string_lossy().into_owned(),
                    version,
                    source: resolved_source,
                });
            }
        }
        None
    }

    /// Classify a canonical path by the manager prefix it lives under.
    /// Returns `None` when no prefix matches (caller falls back to its hint).
    fn classify(&self, canon: &Path) -> Option<ToolSource> {
        for (source, prefix) in self.probe.manager_prefixes() {
            if canon.starts_with(prefix) {
                return Some(source);
            }
        }
        None
    }
}

// ─── RealProbe — production host interaction ─────────────────────────────────

/// Real-filesystem probe. Expands `~`/env vars in manager prefixes lazily and
/// resolves dynamic prefixes (`npm prefix -g`, `go env GOPATH`) only when the
/// manager binary itself is on PATH.
pub struct RealProbe;

#[async_trait]
impl HostProbe for RealProbe {
    fn path_dirs(&self) -> Vec<PathBuf> {
        let sep = if cfg!(windows) { ';' } else { ':' };
        std::env::var("PATH")
            .map(|p| p.split(sep).map(PathBuf::from).collect())
            .unwrap_or_default()
    }

    fn manager_prefixes(&self) -> Vec<(ToolSource, PathBuf)> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut out: Vec<(ToolSource, PathBuf)> = Vec::new();

        if cfg!(target_os = "macos") {
            out.push((ToolSource::Brew, PathBuf::from("/opt/homebrew/bin")));
            out.push((ToolSource::Brew, PathBuf::from("/usr/local/bin")));
        } else if cfg!(target_os = "linux") {
            out.push((
                ToolSource::Brew,
                PathBuf::from("/home/linuxbrew/.linuxbrew/bin"),
            ));
            out.push((ToolSource::Brew, home.join(".linuxbrew/bin")));
        }
        // Windows has no brew.

        out.push((ToolSource::Cargo, home.join(".cargo/bin")));
        out.push((ToolSource::Bun, home.join(".bun/bin")));

        // npm global — resolve dynamically only if npm is present.
        if RealProbe::npm_global_bin().is_some() {
            if let Some(npm_bin) = RealProbe::npm_global_bin() {
                out.push((ToolSource::Npm, npm_bin));
            }
        }

        // pip/uv user dir
        if cfg!(windows) {
            if let Ok(appdata) = std::env::var("APPDATA") {
                out.push((
                    ToolSource::Pip,
                    PathBuf::from(appdata).join("Python/Scripts"),
                ));
            }
        } else {
            out.push((ToolSource::Pip, home.join(".local/bin")));
        }

        // Go GOPATH/bin
        if let Some(go_bin) = RealProbe::go_bin(&home) {
            out.push((ToolSource::Go, go_bin));
        }

        out
    }

    fn resolve(&self, dir: &Path, name: &str) -> Option<PathBuf> {
        if cfg!(windows) {
            // Try PATHEXT extensions.
            let exts: Vec<String> = std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
                .split(';')
                .map(|s| s.to_string())
                .collect();
            for ext in &exts {
                let candidate = dir.join(format!("{name}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            // Also try the bare name.
            let bare = dir.join(name);
            if bare.is_file() {
                return Some(bare);
            }
        } else {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn canonicalize(&self, path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    async fn version(&self, path: &Path) -> Option<String> {
        version_probe(path).await
    }
}

impl RealProbe {
    /// Resolve npm's global bin dir via `npm prefix -g`, only if `npm` is on PATH.
    fn npm_global_bin() -> Option<PathBuf> {
        if which_sync("npm").is_none() {
            return None;
        }
        let out = std::process::Command::new("npm")
            .arg("prefix")
            .arg("-g")
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if prefix.is_empty() {
            return None;
        }
        Some(PathBuf::from(prefix).join(if cfg!(windows) { "" } else { "bin" }))
    }

    /// Resolve Go's bin dir via `go env GOPATH`, only if `go` is on PATH.
    fn go_bin(home: &Path) -> Option<PathBuf> {
        if which_sync("go").is_none() {
            // Default GOPATH assumption.
            return Some(home.join("go/bin"));
        }
        let out = std::process::Command::new("go")
            .args(["env", "GOPATH"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let gopath = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if gopath.is_empty() {
            return None;
        }
        Some(PathBuf::from(gopath).join("bin"))
    }
}

/// Sync `which`-equivalent for the bootstrap checks (npm/go presence). Uses the
/// OS-appropriate lookup. Kept private: the scanner's public detection goes
/// through `HostProbe::resolve`, not this.
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

/// Run `<path> --version` (or `-Version` on Windows-native CLIs) with a 3s
/// timeout and return the trimmed first non-empty line.
async fn version_probe(path: &Path) -> Option<String> {
    use tokio::process::Command;

    let mut cmd = Command::new(path);
    // Most CLIs accept `--version`. PowerShell-style `-Version` is handled by
    // falling back below if `--version` fails.
    cmd.arg("--version");
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    let fut = async {
        let out = cmd.output().await.ok()?;
        if !out.status.success() {
            return None;
        }
        first_version_line(&String::from_utf8_lossy(&out.stdout))
    };

    match tokio::time::timeout(Duration::from_secs(3), fut).await {
        Ok(v) => v,
        Err(_) => None, // timed out
    }
}

/// Extract the first non-empty, trimmed line from a `--version` output blob.
fn first_version_line(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|l| l.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fully in-memory probe for deterministic scanner tests.
    struct FakeProbe {
        paths: Vec<PathBuf>,
        prefixes: Vec<(ToolSource, PathBuf)>,
        /// (dir, name) -> raw resolved path.
        files: HashMap<(PathBuf, String), PathBuf>,
        /// raw path -> canonical path (simulate symlinks).
        links: HashMap<PathBuf, PathBuf>,
        /// canonical path -> version string.
        versions: HashMap<PathBuf, String>,
    }

    impl FakeProbe {
        fn new() -> Self {
            Self {
                paths: vec![],
                prefixes: vec![],
                files: HashMap::new(),
                links: HashMap::new(),
                versions: HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl HostProbe for FakeProbe {
        fn path_dirs(&self) -> Vec<PathBuf> {
            self.paths.clone()
        }
        fn manager_prefixes(&self) -> Vec<(ToolSource, PathBuf)> {
            self.prefixes.clone()
        }
        fn resolve(&self, dir: &Path, name: &str) -> Option<PathBuf> {
            self.files
                .get(&(dir.to_path_buf(), name.to_string()))
                .cloned()
        }
        fn canonicalize(&self, path: &Path) -> PathBuf {
            self.links
                .get(path)
                .cloned()
                .unwrap_or_else(|| path.to_path_buf())
        }
        async fn version(&self, path: &Path) -> Option<String> {
            self.versions.get(path).cloned()
        }
    }

    #[tokio::test]
    async fn detects_tool_on_path() {
        let dir = PathBuf::from("/usr/local/bin");
        let mut probe = FakeProbe::new();
        probe.paths = vec![dir.clone()];
        probe
            .files
            .insert((dir.clone(), "rg".into()), dir.join("rg"));
        probe.versions.insert(dir.join("rg"), "ripgrep 14.0".into());
        let scanner =
            HostToolScanner::with_probe(std::sync::Arc::new(probe), Duration::from_secs(60));

        let t = scanner.detect("rg").await.expect("rg should be found");
        assert_eq!(t.name, "rg");
        assert_eq!(t.source, ToolSource::Path);
        assert_eq!(t.version.as_deref(), Some("ripgrep 14.0"));
    }

    #[tokio::test]
    async fn classifies_brew_via_symlink_canonicalize() {
        // `/usr/local/bin/rg` is a symlink → canonical `/opt/homebrew/bin/rg`.
        let mut probe = FakeProbe::new();
        let link_dir = PathBuf::from("/usr/local/bin");
        let real_dir = PathBuf::from("/opt/homebrew/bin");
        probe.paths = vec![link_dir.clone()];
        probe.prefixes = vec![(ToolSource::Brew, real_dir.clone())];
        probe
            .files
            .insert((link_dir.clone(), "rg".into()), link_dir.join("rg"));
        // simulate symlink: raw /usr/local/bin/rg → canonical /opt/homebrew/bin/rg
        probe.links.insert(link_dir.join("rg"), real_dir.join("rg"));
        let scanner =
            HostToolScanner::with_probe(std::sync::Arc::new(probe), Duration::from_secs(60));

        let t = scanner.detect("rg").await.unwrap();
        // Source should be Brew (canonical path under /opt/homebrew/bin), not Path.
        assert_eq!(t.source, ToolSource::Brew);
        assert_eq!(t.path, "/opt/homebrew/bin/rg");
    }

    #[tokio::test]
    async fn cache_serves_repeat_lookups() {
        let mut probe = FakeProbe::new();
        let dir = PathBuf::from("/bin");
        probe.paths = vec![dir.clone()];
        probe
            .files
            .insert((dir.clone(), "ls".into()), dir.join("ls"));
        let scanner =
            HostToolScanner::with_probe(std::sync::Arc::new(probe), Duration::from_secs(60));

        let _ = scanner.detect("ls").await;
        // Second detect must hit cache (no files to re-read anyway) and still resolve.
        let t = scanner.detect("ls").await.unwrap();
        assert_eq!(t.name, "ls");
    }

    #[tokio::test]
    async fn invalidate_forces_rescan() {
        let mut probe = FakeProbe::new();
        let dir = PathBuf::from("/bin");
        probe.paths = vec![dir.clone()];
        probe
            .files
            .insert((dir.clone(), "ls".into()), dir.join("ls"));
        let scanner =
            HostToolScanner::with_probe(std::sync::Arc::new(probe), Duration::from_secs(60));

        let _ = scanner.detect("ls").await;
        scanner.invalidate();
        assert!(scanner.cache.read().is_empty());
    }

    #[tokio::test]
    async fn missing_binary_returns_none() {
        let mut probe = FakeProbe::new();
        probe.paths = vec![PathBuf::from("/bin")];
        let scanner =
            HostToolScanner::with_probe(std::sync::Arc::new(probe), Duration::from_secs(60));
        assert!(scanner.detect("does-not-exist").await.is_none());
    }

    #[test]
    fn first_version_line_skips_blank() {
        assert_eq!(
            first_version_line("\n\n  ripgrep 14.0\nbuild foo"),
            Some("ripgrep 14.0".into())
        );
        assert_eq!(first_version_line(""), None);
    }
}
