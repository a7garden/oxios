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

pub mod agent_tool;
pub mod budget_tool;
pub mod cron_tool;
pub mod knowledge_tool;
pub mod marketplace_tool;
pub mod persona_tool;
pub mod resource_tool;
pub mod security_tool;
pub mod space_tool;

pub use agent_tool::AgentTool as KernelAgentTool;
pub use budget_tool::BudgetTool;
pub use cron_tool::CronTool;
pub use knowledge_tool::KnowledgeTool;
pub use marketplace_tool::MarketplaceTool;
pub use persona_tool::PersonaTool;
pub use resource_tool::ResourceTool;
pub use security_tool::SecurityTool;
pub use space_tool::SpaceTool;

use crate::types::AgentId;
use crate::KernelHandle;
use oxi_sdk::ToolRegistry;

/// Register all kernel domain tools into the registry.
///
/// Called by [`super::kernel_bridge::OxiosKernelBridge`] during agent build.
/// This is the canonical list of kernel tools available in oxios agents.
pub fn register_all_kernel_tools(registry: &ToolRegistry, kernel: &KernelHandle, _agent_id: &str) {
    let agent_uuid = AgentId::new_v4();

    // ExecTool (stores Arc<KernelHandle>)
    registry.register(crate::tools::ExecTool::from_kernel(kernel));

    // Memory tools (each stores Arc<KernelHandle>)
    registry.register(crate::tools::MemoryReadTool::from_kernel(kernel));
    registry.register(crate::tools::MemorySearchTool::from_kernel(kernel));
    registry.register(crate::tools::MemoryWriteTool::from_kernel(kernel));

    // Kernel domain tools (take &KernelHandle)
    registry.register(SpaceTool::from_kernel(kernel));
    registry.register(KernelAgentTool::from_kernel(kernel));
    registry.register(PersonaTool::from_kernel(kernel));
    registry.register(CronTool::from_kernel(kernel));
    registry.register(SecurityTool::from_kernel(kernel));
    registry.register(BudgetTool::from_kernel(kernel));
    registry.register(ResourceTool::from_kernel(kernel));

    // A2A tools (each stores Arc<KernelHandle>)
    registry.register(crate::tools::A2aDelegateTool::from_kernel(
        kernel, agent_uuid,
    ));
    registry.register(crate::tools::A2aSendTool::from_kernel(kernel, agent_uuid));
    registry.register(crate::tools::A2aQueryTool::from_kernel(kernel));

    // MCP tool wrapper (stores Arc<KernelHandle>)
    registry.register(crate::tools::McpToolWrapper::from_kernel(
        kernel,
        "",
        "",
        "MCP tools via bridge".into(),
        serde_json::json!({"type": "object", "properties": {}}),
    ));

    // KnowledgeTool (markdown note management)
    registry.register(KnowledgeTool::from_kernel(kernel));

    // Browser (optional feature, stores Arc<KernelHandle>)
    #[cfg(feature = "browser")]
    {
        registry.register(crate::tools::BrowserTool::from_kernel(kernel));
    }

    // Marketplace (ClawHub — search, install, update)
    registry.register(MarketplaceTool::from_kernel(kernel));
}
