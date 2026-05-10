//! Interactive CLI channel for Oxios.
//!
//! Provides an interactive terminal interface that plugs into the
//! gateway via the [`Channel`](oxios_gateway::Channel) trait.

#![warn(missing_docs)]

pub mod channel;
pub mod commands;
pub mod interactive;
pub mod plugin;
pub mod session;

pub use channel::{CliChannel, CliChannelHandle};
pub use commands::MetaCommand;
pub use interactive::InteractiveLoop;
pub use plugin::CliPlugin;
pub use session::Session;
