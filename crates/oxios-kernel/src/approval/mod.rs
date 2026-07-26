//! Tool approval mode system (RFC-035).

pub mod policy;
pub mod resolver;

pub use policy::{ApprovalConfig, ApprovalMode, ToolPolicy, DEFAULT_TOOL_POLICIES};
pub use resolver::{ExecPolicyResolver, GlobalResolver, ToolPolicyResolver};
