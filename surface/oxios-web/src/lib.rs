//! Web dashboard surface for Oxios.
//!
//! Provides an HTTP server (axum) that serves as the primary
//! control interface. Implements [`Surface`](oxios_gateway::Surface)
//! for direct kernel access and optionally registers a channel
//! with the gateway for message routing.

#![warn(missing_docs)]

pub mod api_docs;
pub mod channel;
pub mod error;
pub mod format;
pub mod middleware;
pub mod persona_routes;
pub mod plugin;
pub mod routes;
pub mod server;

pub use channel::{WebChannel, WebChannelHandle};
pub use plugin::WebSurface;
pub use server::AppState;
