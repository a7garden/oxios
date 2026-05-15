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
