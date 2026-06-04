//! Storage abstractions for the memory subsystem.
//!
//! The traits here are *abstract* — concrete implementations live
//! in `oxios-kernel` (e.g., `StateStore` impls `MemoryStorage`).

pub mod storage;

pub use storage::{MemoryGit, MemoryStorage};
