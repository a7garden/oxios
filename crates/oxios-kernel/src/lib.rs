//! Oxios kernel: supervisor, event bus, state store.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, and
//! persistent state management.

#![warn(missing_docs)]

pub mod config;
pub mod event_bus;
pub mod state_store;
pub mod supervisor;
pub mod types;

pub use config::OxiosConfig;
pub use event_bus::{EventBus, KernelEvent};
pub use state_store::StateStore;
pub use types::{AgentId, AgentInfo, AgentStatus};
