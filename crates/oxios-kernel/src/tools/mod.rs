//! Oxios-specific agent tools.
//!
//! These tools replace oxi-agent's BashTool with purpose-specific execution tools:
//! - `exec_tool` — unified workspace/host command execution
//! - `program_tool` — Program-defined tools with automatic routing

pub mod exec_tool;
pub mod mcp_tool;
pub mod memory_tools;
pub mod program_tool;

#[cfg(feature = "browser")]
pub mod browser;

pub use exec_tool::ExecTool;
pub use mcp_tool::McpToolWrapper;
pub use memory_tools::{MemoryReadTool, MemorySearchTool, MemoryWriteTool};
pub use program_tool::ProgramTool;

#[cfg(feature = "browser")]
pub use browser::{BrowserTool, BrowserBackend, CdpBackend, LightpandaProcess, LightpandaConfig};