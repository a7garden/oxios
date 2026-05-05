//! Oxios-specific agent tools.
//!
//! These tools replace oxi-agent's BashTool with purpose-specific execution tools:
//! - `container_exec` — workspace command execution (container or local)
//! - `host_exec` — host (macOS) command execution with security allowlist
//! - `program_tool` — Program-defined tools with automatic routing

pub mod container_exec;
pub mod host_exec_tool;
pub mod mcp_tool;
pub mod program_tool;

pub use container_exec::ContainerExecTool;
pub use host_exec_tool::HostExecTool;
pub use mcp_tool::McpToolWrapper;
pub use program_tool::ProgramTool;