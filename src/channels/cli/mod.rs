//! Interactive CLI channel for Oxios (RFC-026: merged from channels/oxios-cli).
//!
//! Internal module of the oxios binary, feature-gated by the `cli` feature.

pub mod channel;
pub mod commands;
pub mod format;
pub mod interactive;
pub mod plugin;
pub mod session;

pub use channel::{CliChannel, CliChannelHandle};
pub use commands::MetaCommand;
pub use format::CliFormatter;
pub use interactive::InteractiveLoop;
pub use plugin::CliPlugin;
pub use session::Session;
