//! Web dashboard HTTP API server (RFC-026: merged from surface/oxios-web).
//!
//! This module is internal to the oxios binary. It is gated at the parent
//! (`#[cfg(feature = "web")] mod api;` in `src/main.rs`), so individual
//! sub-modules do not need their own feature gates.

pub mod api_docs;
pub mod bridge;
pub mod error;
pub mod middleware;
pub mod persona_routes;
pub mod plugin;
pub mod quota;
pub mod routes;
pub mod server;

pub use plugin::WebSurface;
