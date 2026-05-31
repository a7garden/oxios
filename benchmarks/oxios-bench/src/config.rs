//! Configuration for benchmark runs.

use std::path::PathBuf;

/// Global benchmark configuration.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Path to the `oxios` binary.
    pub oxios_bin: PathBuf,
    /// Directory for TOML task definitions.
    pub suites_dir: PathBuf,
    /// Directory for generated reports.
    pub reports_dir: PathBuf,
    /// Maximum concurrent tasks.
    pub parallel: usize,
    /// Global per-task timeout in seconds.
    pub timeout_secs: u64,
    /// Whether to compare against baseline for regressions.
    pub check_regressions: bool,
    /// Path to baseline report for regression comparison.
    pub baseline_path: Option<PathBuf>,
}

impl BenchConfig {
    /// Detect the oxios binary path.
    pub fn detect_oxios_bin() -> PathBuf {
        // Try current target first (most likely during development)
        let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/oxios");
        if local.exists() {
            return local;
        }

        let local_release =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/oxios");
        if local_release.exists() {
            return local_release;
        }

        // Fallback: assume it's in PATH
        PathBuf::from("oxios")
    }

    /// Default reports directory.
    pub fn default_reports_dir() -> PathBuf {
        PathBuf::from(".oxios-bench/reports")
    }
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            oxios_bin: Self::detect_oxios_bin(),
            suites_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("suites"),
            reports_dir: Self::default_reports_dir(),
            parallel: 1,
            timeout_secs: 120,
            check_regressions: false,
            baseline_path: None,
        }
    }
}
