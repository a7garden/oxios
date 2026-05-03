//! Oxios kernel: supervisor, event bus, state store, container, garden.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, container
//! management, and persistent state management.

#![warn(missing_docs)]

pub mod agent_runtime;
pub mod config;
pub mod container;
pub mod event_bus;
pub mod garden;
pub mod host_exec;
pub mod state_store;
pub mod supervisor;
pub mod types;

pub use agent_runtime::AgentRuntime;
pub use config::OxiosConfig;
pub use container::{
    AppleBackend, ContainerBackend, ContainerStats, ContainerStatus, ExecResult, GardenStartConfig,
};
pub use event_bus::{EventBus, KernelEvent};
pub use garden::{GardenInfo, GardenManager};
pub use host_exec::HostExecBridge;
pub use state_store::StateStore;
pub use types::{AgentId, AgentInfo, AgentStatus};
