//! In-process communication channels (RFC-026: merged from channels/oxios-{cli,telegram}).
//!
//! Sub-modules are feature-gated individually.

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "telegram")]
pub mod telegram;
