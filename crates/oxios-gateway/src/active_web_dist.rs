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
        prev.map(|p| (*p).clone())
    }

    /// Publish `new_path` and asynchronously remove the previous directory
    /// after a grace period. Keeps at most one previous generation around so
    /// in-flight requests reading from the old inode complete successfully.
    ///
    /// No-op cleanup if there was no previous directory.
    pub fn swap_and_clean_previous(&self, new_path: PathBuf, grace: std::time::Duration) {
        let prev = self.swap(new_path);
        if let Some(old) = prev {
            tokio::spawn(async move {
                tokio::time::sleep(grace).await;
                // Best-effort removal; ignore errors (already gone, permissions, …).
                if old.is_dir() {
                    let _ = std::fs::remove_dir_all(&old);
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
    pub fn publish(&self, new_dir: PathBuf, marker: &std::path::Path) {
        // swap_and_clean_previous schedules removal of the *previous* dir.
        // The new dir is never moved or deleted after this.
        self.swap_and_clean_previous(new_dir.clone(), std::time::Duration::from_secs(300));
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(marker, new_dir.to_string_lossy().as_bytes());
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
            .cloned()
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
}
