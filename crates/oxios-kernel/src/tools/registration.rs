//! CSpace → Tool Registry mapping.
//!
//! This module bridges the capability system and the agent's tool registry.
//! Given an agent's [`CSpace`], it walks the capabilities and registers
//! exactly the set of tools the agent is authorised to use.
//!
//! # Registration tiers
//!
//! | Tier | Tools | Condition |
//! |------|-------|-----------|
//! | Always-on | `ReadTool`, `WriteTool`, `EditTool`, `GrepTool`, `FindTool`, `LsTool`, `WebSearchTool`, `GetSearchResultsTool` | Every agent gets these |
//! | CSpace-driven | `ExecTool`, `BrowserTool`, kernel domain tools, MCP, A2A, etc. | Only if a matching capability with sufficient rights exists |
//!
//! # Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use oxi_agent::{ToolRegistry, SearchCache};
//! use oxios_kernel::capability::template::CapabilityTemplate;
//! use oxios_kernel::tools::registration::register_tools_from_cspace;
//!
//! let registry = ToolRegistry::new();
//! let cspace = CapabilityTemplate::standard().build();
//! let cache = Arc::new(SearchCache::new());
//! register_tools_from_cspace(&registry, &kernel, &cspace, cache, agent_id);
//! ```

use std::sync::Arc;

use oxi_agent::{
    EditTool, FindTool, GetSearchResultsTool, GrepTool, LsTool, ReadTool, SearchCache,
    ToolRegistry, WebSearchTool, WriteTool,
};

use crate::capability::{CSpace, ResourceRef, Rights};
use crate::tools::kernel::*;
use crate::tools::{
    A2aDelegateTool, A2aQueryTool, A2aSendTool, ExecTool,
    MemoryReadTool, MemorySearchTool, MemoryWriteTool,
};
use crate::types::AgentId;
use crate::KernelHandle;

#[cfg(feature = "browser")]
use crate::tools::BrowserTool;

/// Register the always-on tool set into a [`ToolRegistry`].
///
/// Every agent receives these tools regardless of its capability space.
/// This consists of file-system tools (read, write, edit, grep, find, ls)
/// and web search tools.
///
/// This helper is also useful for unit tests that need a basic tool set
/// without constructing a full CSpace.
pub fn register_always_on(registry: &ToolRegistry, search_cache: Arc<SearchCache>) {
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());
    registry.register(WebSearchTool::new(search_cache.clone()));
    registry.register(GetSearchResultsTool::new(search_cache));
}

/// Register tools into `registry` based on the agent's [`CSpace`].
///
/// First registers the always-on tier (file ops + web search), then walks
/// every capability in the CSpace and conditionally registers the
/// corresponding kernel tools.
///
/// # Arguments
///
/// * `registry` — The agent's tool registry to populate.
/// * `kernel` — Handle to the kernel for constructing tool instances.
/// * `cspace` — The agent's capability space (determines which tools are available).
/// * `search_cache` — Shared search cache for web search tools.
/// * `agent_id` — The agent's ID (used by A2A tools for routing).
///
/// # CSpace → Tool mapping
///
/// | ResourceRef | Required rights | Registered tools |
/// |-------------|----------------|-----------------|
/// | `Exec { .. }` | `EXECUTE` | `ExecTool` |
/// | `Browser` | `EXECUTE` | `BrowserTool` *(browser feature)* |
/// | `KernelDomain { "memory" }` | `READ` | `MemoryReadTool`, `MemorySearchTool` |
/// | `KernelDomain { "memory" }` | `WRITE` | `MemoryWriteTool` |
/// | `KernelDomain { "space" }` | any | `SpaceTool` |
/// | `KernelDomain { "agent" }` | any | `KernelAgentTool` |
/// | `KernelDomain { "a2a" }` | any | `A2aDelegateTool`, `A2aSendTool`, `A2aQueryTool` |
/// | `KernelDomain { "persona" }` | any | `PersonaTool` |
/// | `KernelDomain { "program" }` | any | `ProgramTool` |
/// | `KernelDomain { "cron" }` | any | `CronTool` |
/// | `KernelDomain { "security" }` | any | `SecurityTool` |
/// | `KernelDomain { "budget" }` | any | `BudgetTool` |
/// | `KernelDomain { "resource" }` | any | `ResourceTool` |
/// | `KernelDomain { "mcp" }` | any | `McpToolWrapper` |
/// | `Program { .. }` | — | *(not registered; surfaced via ToolRetriever)* |
pub fn register_tools_from_cspace(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    cspace: &CSpace,
    search_cache: Arc<SearchCache>,
    agent_id: AgentId,
) {
    // ── Tier 1: Always-on tools ─────────────────────────────────────
    register_always_on(registry, search_cache);

    // ── Tier 2: CSpace-driven tools ─────────────────────────────────
    for cap in cspace.iter() {
        match &cap.resource {
            // Command execution
            ResourceRef::Exec { .. } if cap.rights.contains(Rights::EXECUTE) => {
                registry.register(ExecTool::from_kernel(kernel));
            }

            // Headless browser
            ResourceRef::Browser if cap.rights.contains(Rights::EXECUTE) => {
                #[cfg(feature = "browser")]
                {
                    registry.register(BrowserTool::from_kernel(kernel));
                }
            }

            // Kernel domain tools
            ResourceRef::KernelDomain { domain } => match domain.as_str() {
                "memory" => {
                    if cap.rights.contains(Rights::READ) {
                        registry.register(MemoryReadTool::from_kernel(kernel));
                        registry.register(MemorySearchTool::from_kernel(kernel));
                    }
                    if cap.rights.contains(Rights::WRITE) {
                        registry.register(MemoryWriteTool::from_kernel(kernel));
                    }
                }
                "space" => registry.register(SpaceTool::from_kernel(kernel)),
                "agent" => registry.register(KernelAgentTool::from_kernel(kernel)),
                "a2a" => {
                    registry.register(A2aDelegateTool::from_kernel(kernel, agent_id));
                    registry.register(A2aSendTool::from_kernel(kernel, agent_id));
                    registry.register(A2aQueryTool::from_kernel(kernel));
                }
                "persona" => registry.register(PersonaTool::from_kernel(kernel)),
                "program" => { /* ProgramTools are registered individually by agent_runtime, not via CSpace */ }
                "cron" => registry.register(CronTool::from_kernel(kernel)),
                "security" => registry.register(SecurityTool::from_kernel(kernel)),
                "budget" => registry.register(BudgetTool::from_kernel(kernel)),
                "resource" => registry.register(ResourceTool::from_kernel(kernel)),
                "mcp" => { /* MCP tools are enumerated dynamically per agent */ }
                _ => {} // Unknown domain — silently skip
            },

            // Programs are not registered as separate tools.
            // ToolRetriever shows them in the capability index;
            // agents use exec to run program commands.
            ResourceRef::Program { .. } => {}

            // Space, Agent, Mcp resource refs are handled through
            // their respective KernelDomain registrations above
            // or through dedicated tool paths.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_always_on_registers_eight_tools() {
        let registry = ToolRegistry::new();
        let cache = Arc::new(SearchCache::new());
        register_always_on(&registry, cache);

        // The always-on set is: read, write, edit, grep, find, ls, web_search, get_search_results
        // ToolRegistry doesn't expose a count, but we can verify individual tool names.
        let tool_names = registry.names();
        assert!(tool_names.contains(&"read".to_string()), "read tool should be registered");
        assert!(tool_names.contains(&"write".to_string()), "write tool should be registered");
        assert!(tool_names.contains(&"edit".to_string()), "edit tool should be registered");
        assert!(tool_names.contains(&"grep".to_string()), "grep tool should be registered");
        assert!(tool_names.contains(&"find".to_string()), "find tool should be registered");
        assert!(tool_names.contains(&"ls".to_string()), "ls tool should be registered");
        assert!(tool_names.contains(&"web_search".to_string()), "web_search tool should be registered");
        assert!(
            tool_names.contains(&"get_search_results".to_string()),
            "get_search_results tool should be registered"
        );
    }
}
