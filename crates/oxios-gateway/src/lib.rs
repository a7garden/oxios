//! Channel-agnostic message gateway for Oxios.
//!
//! The Gateway routes messages between channels (Web, CLI, Telegram, etc.)
//! and the kernel. Channels are plugins that implement the [`Channel`] trait.

#![warn(missing_docs)]

pub mod channel;
pub mod gateway;
pub mod message;
pub mod plugin;

pub use channel::Channel;
pub use gateway::Gateway;
pub use message::{IncomingMessage, OutgoingMessage};
pub use plugin::{ChannelBundle, ChannelContext, ChannelPlugin};
