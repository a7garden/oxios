//! Code Workspace module — session management, change tracking, checkpoints.

pub mod change_tracker;
pub mod checkpoint;
pub mod session;

pub use change_tracker::{ChangeAction, ChangeTracker, FileChange};
pub use checkpoint::{Checkpoint, CheckpointManager};
pub use session::{CodeSession, CodeSessionState, TodoItem, TodoStatus};
