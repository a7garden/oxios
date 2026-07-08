//! Interactive terminal (PTY-bridged WebSocket). RFC-038.
//!
//! Provides:
//! - [`PtyManager`]: in-kernel session registry, GC tick, access control.
//! - [`PtySession`]: per-session state (PTY handle, principal, lifecycle).
//! - [`PtyError`]: error type for manager operations.
//!
//! The kernel exposes this through [`crate::kernel_handle::PtyApi`].
pub mod config;
pub mod error;
pub mod manager;
pub mod session;

pub use config::PtyConfigSnapshot;
pub use error::PtyError;
pub use manager::PtyManager;
pub use session::{PtySessionId, PtySessionInfo, PtySessionState, PtySize};
