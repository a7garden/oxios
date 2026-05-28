//! Channel-agnostic message gateway for Oxios.
//!
//! The Gateway routes messages between channels (Web, CLI, Telegram, etc.)
//! and the kernel. Channels are plugins that implement the [`Channel`] trait.
//!
//! Each channel runs as an independent background task, pushing messages
//! into a shared mpsc channel. The gateway dispatches them concurrently
//! with semaphore-bounded parallelism.

#![warn(missing_docs)]

pub mod channel;
pub mod error_classify;
pub mod format;
pub mod gateway;
pub mod message;
pub mod meta;
pub mod plugin;

pub use channel::Channel;
pub use error_classify::classify_error;
pub use format::ChannelFormatter;
pub use gateway::Gateway;
pub use message::{
    ErrorKind, IncomingMessage, OutgoingMessage, ResponseMeta, UserFacingError,
};
pub use plugin::{ChannelBundle, ChannelContext, ChannelPlugin};

/// Unified inbox type for the gateway.
///
/// Each channel pushes `(channel_name, incoming_message)` tuples
/// into the shared mpsc channel.
pub type GatewayInbox = (String, IncomingMessage);
