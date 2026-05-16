//! Kernel tools — AgentTool wrappers for KernelHandle API domains.
//!
//! These tools expose kernel system calls to the agent's tool-calling loop.
//! Each tool wraps a specific domain API and uses an action-based parameter
//! schema to dispatch operations.
//!
//! ## Tools
//!
//! - [`SpaceTool`] — Space management (list, get, create, archive, merge, restore)
//! - [`AgentTool`] — Agent lifecycle (list, kill, budget)
//! - [`PersonaTool`] — Persona management (list, set_active, get)
//! - [`CronTool`] — Cron scheduling (list, add, remove, trigger)
//! - [`SecurityTool`] — Security audit (verify_chain, query_audit, audit_count)
//! - [`BudgetTool`] — Budget management (check, set, reserve, reset)
//! - [`ResourceTool`] — Resource monitoring (snapshot, history, overloaded)

pub mod space_tool;
pub mod agent_tool;
pub mod persona_tool;
pub mod cron_tool;
pub mod security_tool;
pub mod budget_tool;
pub mod resource_tool;

pub use space_tool::SpaceTool;
pub use agent_tool::AgentTool as KernelAgentTool;
pub use persona_tool::PersonaTool;
pub use cron_tool::CronTool;
pub use security_tool::SecurityTool;
pub use budget_tool::BudgetTool;
pub use resource_tool::ResourceTool;

use crate::types::AgentId;
use crate::KernelHandle;
use oxi_agent::ToolRegistry;

/// Register all kernel domain tools into the registry.
///
/// Called by [`super::kernel_bridge::OxiosKernelBridge`] during agent build.
/// This is the canonical list of kernel tools available in oxios agents.
pub fn register_all_kernel_tools(registry: &ToolRegistry, kernel: &KernelHandle, agent_id: &str) {
    let agent_uuid = AgentId::new_v4();

    // ExecTool
    registry.register(crate::tools::ExecTool::from_kernel(kernel.clone()));


    // Memory tools
    registry.register(crate::tools::MemoryReadTool::from_kernel(kernel.clone()));
    registry.register(crate::tools::MemorySearchTool::from_kernel(kernel.clone()));
    registry.register(crate::tools::MemoryWriteTool::from_kernel(kernel.clone()));


    // Kernel domain tools
    registry.register(SpaceTool::from_kernel(kernel));
    registry.register(KernelAgentTool::from_kernel(kernel));
    registry.register(PersonaTool::from_kernel(kernel));
    registry.register(CronTool::from_kernel(kernel));
    registry.register(SecurityTool::from_kernel(kernel));
    registry.register(BudgetTool::from_kernel(kernel));
    registry.register(ResourceTool::from_kernel(kernel));

    // A2A tools
    registry.register(crate::tools::A2aDelegateTool::from_kernel(kernel.clone(), agent_uuid));
    registry.register(crate::tools::A2aSendTool::from_kernel(kernel.clone(), agent_uuid));
    registry.register(crate::tools::A2aQueryTool::from_kernel(kernel.clone()));

    // MCP tool wrapper (singleton — dynamic MCP tools are handled via bridge)
    registry.register(crate::tools::McpToolWrapper::from_kernel(
        kernel.clone(),
        "",
        "",
        "MCP tools via bridge".into(),
        serde_json::json!({"type": "object", "properties": {}}),
    ));

    // ProgramTool (dynamic — actual tool instances come from ProgramManager)
    registry.register(crate::tools::ProgramTool::from_kernel(kernel));

    // Browser (optional feature)
    #[cfg(feature = "browser")]
    {
        registry.register(crate::tools::BrowserTool::from_kernel(kernel.clone()));
    }
}
