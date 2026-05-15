//! Browser API — browser backend facade.

use std::sync::Arc;

#[cfg(feature = "browser")]
use crate::tools::OxibrowserBackend;

/// Browser management system calls.
///
/// When the `browser` feature is enabled, wraps [`OxibrowserBackend`].
/// Otherwise, this is a zero-sized placeholder.
#[cfg(feature = "browser")]
pub struct BrowserApi {
    backend: Arc<OxibrowserBackend>,
}

#[cfg(feature = "browser")]
impl BrowserApi {
    /// Create a new BrowserApi.
    pub fn new(backend: Arc<OxibrowserBackend>) -> Self {
        Self { backend }
    }

    /// Browser backend reference.
    pub fn backend(&self) -> &Arc<OxibrowserBackend> {
        &self.backend
    }
}

/// Default (no-op) construction for `from_subsystems`.
/// This is used by the deprecated constructor which has no browser backend available.
///
/// **Panics** if the `browser` feature is enabled, because a real backend is required.
/// Use [`KernelHandle::new()`] instead.
#[cfg(feature = "browser")]
impl Default for BrowserApi {
    fn default() -> Self {
        panic!("BrowserApi::default() called with browser feature enabled — use KernelHandle::new() with a real BrowserApi");
    }
}

/// Zero-sized browser placeholder when the `browser` feature is disabled.
#[cfg(not(feature = "browser"))]
pub struct BrowserApi;

#[cfg(not(feature = "browser"))]
impl Default for BrowserApi {
    fn default() -> Self {
        BrowserApi
    }
}
