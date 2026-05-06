//! Web dashboard channel for Oxios.
//!
//! Provides an HTTP server (axum) that serves as the primary
//! user interface. Implements the [`Channel`] trait so it plugs
//! directly into the gateway.

#![warn(missing_docs)]

pub mod channel;
pub mod middleware;
pub mod persona_routes;
pub mod routes;
pub mod server;

pub use channel::{WebChannel, WebChannelHandle};
pub use server::{AppState, WebServer};
