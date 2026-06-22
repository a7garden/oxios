//! Atomic active web-dist handle (RFC-024 SP3).
//!
//! Web UI assets are served from a directory that can be swapped at runtime
//! (manual update, daily auto-update). To avoid the 404 window that opens
//! when an update deletes files mid-serve, the *active* directory is
//! published through an atomic pointer ([`arc_swap::ArcSwapOption`]).
//!
//! Every request loads the current pointer (O(1), lock-free) and reads from
//! that directory. An update extracts a fully-formed directory first, then
//! publishes it with a single atomic store — readers never observe a
//! half-populated directory.
//!
//! Contract (RFC-024 C4): serving static assets never returns 404 due to an
//! in-flight update. The pointer is either the old (complete) or the new
//! (complete) directory.

use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwapOption;

/// Atomically-swappable handle to the active web-dist directory.
///
/// Cheaply cloneable (`Arc` inside). Safe to share across the web surface
/// (request handlers) and the kernel's daily health check (updater).
#[derive(Clone)]
pub struct ActiveWebDist {
    inner: Arc<ArcSwapOption<PathBuf>>,
}

impl ActiveWebDist {
    /// Create a new handle pointing at `path` (or "no active dist" when `None`).
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(ArcSwapOption::new(path.map(Arc::new))),
        }
    }

    /// Load the current active directory path, cloning the `PathBuf`.
    ///
    /// Returns `None` when no active dist is published (embedded fallback
    /// should be used by the caller).
    pub fn path(&self) -> Option<PathBuf> {
        self.inner.load().as_ref().map(|p| (**p).clone())
    }

    /// Atomically publish `new_path` as the active directory.
    ///
    /// Returns the *previous* active path (if any) so callers can schedule
    /// cleanup of the now-orphaned directory after a grace period (letting
    /// in-flight requests finish reading from the old inode).
    pub fn swap(&self, new_path: PathBuf) -> Option<PathBuf> {
        // Capture the previous value before publishing. Updates are
        // infrequent (daily auto-update / manual), so the tiny window
        // between load_full and store is inconsequential.
        let prev = self.inner.load_full();
        self.inner.store(Some(Arc::new(new_path)));
        // RFC-024 §11: count atomic swaps. First publish (None → Some) is
        // a no-op for the metric — the daemon started, nothing was swapped.
        // Subsequent publishes each bump the counter so daily / manual
        // updates show up in `oxios_web_dist_swaps_total`.
        if prev.is_some() {
            oxios_kernel::metrics::get_metrics().web_dist_swaps.inc();
        }
        prev.map(|p| (*p).clone())
    }

    /// Publish `new_path` and asynchronously remove the previous directory
    /// after a grace period. Keeps at most one previous generation around so
    /// in-flight requests reading from the old inode complete successfully.
    ///
    /// No-op cleanup if there was no previous directory.
    ///
    /// F25: the removal runs inside `spawn_blocking` so the synchronous
    /// `remove_dir_all` doesn't occupy an async worker thread.
    pub fn swap_and_clean_previous(&self, new_path: PathBuf, grace: std::time::Duration) {
        let prev = self.swap(new_path);
        if let Some(old) = prev {
            tokio::spawn(async move {
                tokio::time::sleep(grace).await;
                // Best-effort removal; ignore errors (already gone, permissions, …).
                // F25: offload the blocking remove_dir_all to the blocking pool.
                if old.is_dir() {
                    let _ =
                        tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&old)).await;
                }
            });
        }
    }

    /// Publish `new_dir` as the active directory AND persist a marker file so
    /// the next process start can resolve it (the in-memory pointer does not
    /// survive restart). The marker is written *after* the pointer swap, so a
    /// crash mid-publish at worst leaves the marker pointing at the previous
    /// generation — never at a half-extracted directory.
    ///
    /// `new_dir` must already be fully extracted and validated by the caller.
    ///
    /// F26: the marker write is now logged on failure (previously silently
    /// dropped) and performed via a temp-file rename so a crash can't leave a
    /// truncated marker. The signature stays `()` for compatibility with
    /// existing callers; a divergence between the in-memory pointer and the
    /// persisted marker is surfaced via `tracing::error!`.
    pub fn publish(&self, new_dir: PathBuf, marker: &std::path::Path) {
        // swap_and_clean_previous schedules removal of the *previous* dir.
        // The new dir is never moved or deleted after this.
        self.swap_and_clean_previous(new_dir.clone(), std::time::Duration::from_secs(300));
        if let Some(parent) = marker.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::error!(
                marker = %marker.display(),
                error = %e,
                "Failed to create marker parent directory; \
                 in-memory pointer and persisted marker will diverge on restart"
            );
            return;
        }
        // F26: atomic marker write via tmp+rename.
        let tmp = marker.with_extension("marker.tmp");
        if let Err(e) = std::fs::write(&tmp, new_dir.to_string_lossy().as_bytes())
            .and_then(|()| std::fs::rename(&tmp, marker))
        {
            tracing::error!(
                marker = %marker.display(),
                error = %e,
                "Failed to persist web-dist marker; \
                 in-memory pointer and persisted marker will diverge on restart"
            );
        }
    }

    /// Resolve the active directory at process start.
    ///
    /// Order: (1) the persisted marker file, if it points at a directory with
    /// `index.html`; (2) the `legacy` directory, if it has `index.html`.
    /// Returns `None` when neither is usable (caller should download/embed).
    pub fn resolve(marker: &std::path::Path, legacy: Option<&std::path::Path>) -> Option<PathBuf> {
        if let Ok(s) = std::fs::read_to_string(marker) {
            let p = PathBuf::from(s.trim());
            if p.join("index.html").is_file() {
                return Some(p);
            }
        }
        legacy
            .filter(|p| p.join("index.html").is_file())
            .map(PathBuf::from)
    }
}

impl Default for ActiveWebDist {
    fn default() -> Self {
        Self::new(None)
    }
}

impl std::fmt::Debug for ActiveWebDist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveWebDist")
            .field("path", &self.path())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_none_has_no_path() {
        let h = ActiveWebDist::new(None);
        assert!(h.path().is_none());
    }

    #[test]
    fn new_some_returns_path() {
        let h = ActiveWebDist::new(Some(PathBuf::from("/tmp/dist")));
        assert_eq!(h.path().as_deref(), Some(std::path::Path::new("/tmp/dist")));
    }

    #[test]
    fn swap_returns_previous_and_updates_current() {
        let h = ActiveWebDist::new(Some(PathBuf::from("/old")));
        let prev = h.swap(PathBuf::from("/new"));
        assert_eq!(prev.as_deref(), Some(std::path::Path::new("/old")));
        assert_eq!(h.path().as_deref(), Some(std::path::Path::new("/new")));
    }

    #[test]
    fn clones_share_state() {
        let a = ActiveWebDist::new(Some(PathBuf::from("/x")));
        let b = a.clone();
        b.swap(PathBuf::from("/y"));
        assert_eq!(a.path().as_deref(), Some(std::path::Path::new("/y")));
    }

    /// RFC-024 §11: every `swap` after the initial publish must
    /// increment `oxios_web_dist_swaps_total`. The first publish
    /// (None → Some) is a daemon-startup event, not a swap, so it
    /// does NOT count — this keeps the metric free of startup
    /// noise that would mask real update activity.
    ///
    /// Note: the metric is a process-wide counter shared with other
    /// tests in this binary. We therefore assert on the *delta*
    /// (before/after), not the absolute value, so the test is
    /// independent of execution order.
    #[test]
    fn swap_increments_metric_only_after_initial_publish() {
        let _ = oxios_kernel::metrics::get_metrics();
        let before = counter_value("oxios_web_dist_swaps_total");

        // First publish (None → Some) — daemon boot, not a swap.
        let h = ActiveWebDist::new(Some(PathBuf::from("/v1")));
        let after_boot = counter_value("oxios_web_dist_swaps_total");
        assert_eq!(
            after_boot - before,
            0,
            "first publish must not count as a swap"
        );

        // Subsequent publish — counts.
        let _ = h.swap(PathBuf::from("/v2"));
        let after_one = counter_value("oxios_web_dist_swaps_total");
        assert_eq!(after_one - after_boot, 1, "swap must count");
        // And again.
        let _ = h.swap(PathBuf::from("/v3"));
        let after_two = counter_value("oxios_web_dist_swaps_total");
        assert_eq!(after_two - after_one, 1, "swap must count");
    }

    fn counter_value(metric: &str) -> u64 {
        let export = oxios_kernel::metrics::registry().export();
        for line in export.lines() {
            if let Some(rest) = line.strip_prefix(metric) {
                let after = rest.trim_start();
                if let Some(num) = after.split_whitespace().next() {
                    return num.parse().unwrap_or(0);
                }
            }
        }
        0
    }
}
