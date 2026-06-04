//! Oxios-specific agent tools.
//!
//! These tools replace oxi-agent's BashTool with purpose-specific execution tools:
//! - `exec_tool` — unified workspace/host command execution

pub mod a2a_tools;
pub mod builtin;
pub mod exec_tool;
pub mod gated_tool;
pub mod kernel_bridge;
pub mod mcp_tool;
pub mod memory_tools;
pub mod registration;
pub mod retrieval;
pub mod tool_types;

#[cfg(feature = "browser")]
pub mod browser;

pub use a2a_tools::{A2aDelegateTool, A2aQueryTool, A2aSendTool};
pub use exec_tool::ExecTool;
pub use builtin::{
    BudgetTool, CronTool, KernelAgentTool, KnowledgeTool, PersonaTool, ProjectTool, ResourceTool,
    SecurityTool,
};
pub use mcp_tool::McpToolWrapper;
pub use memory_tools::{MemoryReadTool, MemorySearchTool, MemoryWriteTool};

#[cfg(feature = "browser")]
pub use browser::BrowserTool;

pub use kernel_bridge::OxiosKernelBridge;
