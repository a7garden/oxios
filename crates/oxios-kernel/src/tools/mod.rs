//! Oxios-specific agent tools.
//!
//! These tools replace oxi-agent's BashTool with purpose-specific execution tools:
//! - `exec_tool` — unified workspace/host command execution (Phase 1: alongside existing, Phase 2: primary)
//! - `program_tool` — Program-defined tools with automatic routing

pub mod exec_tool;
pub mod host_exec_tool;
pub mod mcp_tool;
pub mod memory_tools;
pub mod program_tool;

pub use exec_tool::ExecTool;
pub use host_exec_tool::HostExecTool;
pub use mcp_tool::McpToolWrapper;
pub use memory_tools::{MemoryReadTool, MemorySearchTool, MemoryWriteTool};
pub use program_tool::ProgramTool;