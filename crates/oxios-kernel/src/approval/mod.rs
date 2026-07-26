//! Tool approval mode system (RFC-035).

pub mod blacklist;
pub mod gate;
pub mod policy;
pub mod resolver;

pub use blacklist::{default_blacklist_rules, ArgMatcher, BlacklistRule, SecurityBlacklist};
pub use gate::{default_tool_policy_map, ApprovalDecision, ApprovalGate, ToolCall};
pub use policy::{ApprovalConfig, ApprovalMode, ToolPolicy, DEFAULT_TOOL_POLICIES};
pub use resolver::{ExecPolicyResolver, GlobalResolver, ToolPolicyResolver};
