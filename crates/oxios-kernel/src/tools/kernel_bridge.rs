//! KernelToolProvider bridge — plugs oxios kernel tools into oxi-sdk agent builder.
//!
//! Implements [`oxi_sdk::KernelToolProvider`] so that oxios kernel tools
//! (exec, memory, browser, etc.) can be registered into the SDK's
//! `AgentBuilder` via `.kernel_tools()`.

use std::sync::Arc;

use oxi_sdk::SearchCache;
use oxi_sdk::ToolRegistry;
use oxi_sdk::{
    KernelToolContext as SdkKernelToolContext, KernelToolProvider as SdkKernelToolProvider,
};

use crate::tools::registration::register_always_on;
use crate::KernelHandle;

/// Bridges all oxios kernel tools into the oxi-sdk agent builder.
pub struct OxiosKernelBridge {
    kernel_handle: Arc<KernelHandle>,
    search_cache: Arc<SearchCache>,
}

impl OxiosKernelBridge {
    /// Create a new bridge with the given kernel handle.
    pub fn new(kernel_handle: Arc<KernelHandle>) -> Self {
        Self {
            kernel_handle,
            search_cache: Arc::new(SearchCache::new()),
        }
    }

    /// Create a new bridge with a pre-built search cache.
    pub fn with_cache(kernel_handle: Arc<KernelHandle>, search_cache: Arc<SearchCache>) -> Self {
        Self {
            kernel_handle,
            search_cache,
        }
    }
}

impl SdkKernelToolProvider for OxiosKernelBridge {
    fn tool_names(&self) -> Vec<&str> {
        vec![
            // Always-on file tools
            "read",
            "write",
            "edit",
            "grep",
            "find",
            "ls",
            // Kernel domain
            "exec",
            "memory_read",
            "memory_write",
            "memory_search",
            "project",
            "agent",
            "a2a_delegate",
            "a2a_send",
            "a2a_query",
            "persona",
            "cron",
            "security",
            "budget",
            "resource",
            "mcp",
            "browser",
            "knowledge",
            // Marketplace (ClawHub)
            "marketplace",
        ]
    }

    fn register_tools(&self, registry: &ToolRegistry, context: &SdkKernelToolContext) {
        // 1. Always-on file tools + web search
        register_always_on(registry, Arc::clone(&self.search_cache));

        // 2. Kernel domain tools via KernelHandle
        crate::tools::kernel::register_all_kernel_tools(
            registry,
            &self.kernel_handle,
            &context.agent_id,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `tool_names()` returns the expected number of tool names.
    #[tokio::test]
    async fn test_tool_names_length() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().to_path_buf();

        // Build a minimal KernelHandle for testing
        let state_store =
            Arc::new(crate::state_store::StateStore::new(base.join("workspace")).unwrap());

        let kernel = Arc::new(crate::KernelHandle::new(
            crate::StateApi::new(state_store.clone()),
            crate::AgentApi::new(
                Arc::new(crate::supervisor::NoOpSupervisor),
                Arc::new(crate::budget::BudgetManager::new()),
                Arc::new(crate::memory::MemoryManager::new(state_store.clone())),
                None,
            ),
            crate::SecurityApi::new(
                Arc::new(parking_lot::Mutex::new(crate::auth::AuthManager::new())),
                Arc::new(crate::audit_trail::AuditTrail::new(100)),
                Arc::new(parking_lot::Mutex::new(
                    crate::access_manager::AccessManager::new(),
                )),
                state_store.clone(),
            ),
            crate::PersonaApi::new(Arc::new(crate::persona_manager::PersonaManager::new())),
            crate::ExtensionApi::new(
                Arc::new(
                    crate::skill::SkillManager::new(
                        base.join("skills"),
                        base.join("share/skills"),
                    ),
                ),
            ),
            crate::McpApi::new(Arc::new(crate::mcp::McpBridge::new())),
            crate::InfraApi::new(
                Arc::new(crate::git_layer::GitLayer::new(base.join("git"), false).unwrap()),
                Arc::new(crate::scheduler::AgentScheduler::new(5, 60, 300)),
                Arc::new(crate::cron::CronScheduler::new(state_store.clone(), 60)),
                Arc::new(crate::resource_monitor::ResourceMonitor::new(60, 60)),
                crate::event_bus::EventBus::new(256),
                crate::OxiosConfig::default(),
                std::time::Instant::now(),
            ),
            None,
            crate::ExecApi::new(
                Arc::new(crate::config::ExecConfig::default()),
                Arc::new(parking_lot::Mutex::new(
                    crate::access_manager::AccessManager::new(),
                )),
            ),
            crate::BrowserApi::default(),
            crate::A2aApi::new(Arc::new(crate::a2a::A2AProtocol::new(
                crate::event_bus::EventBus::new(256),
            ))),
            crate::EngineApi::new(
                Arc::new(parking_lot::RwLock::new(crate::OxiosConfig::default())),
                base.join("config.toml"),
                Arc::new(crate::RoutingStats::new()),
            ),
            Arc::new(oxios_markdown::KnowledgeBase::new(base.join("knowledge")).unwrap()),
            Arc::new(
                crate::kernel_handle::KnowledgeLens::new(
                    Arc::new(
                        oxios_markdown::KnowledgeBase::new(base.join("knowledge_lens")).unwrap(),
                    ),
                    Arc::new(crate::memory::MemoryManager::new(state_store.clone())),
                )
                .unwrap(),
            ),
            crate::MarketplaceApi::new(
                Arc::new(crate::clawhub::ClawHubInstaller::new(
                    base.join("skills"),
                    base.join("workspace"),
                    None,
                )),
                Arc::new(crate::clawhub::ClawHubClient::new(None).expect("valid ClawHub client")),
            ),
        ));

        let bridge = OxiosKernelBridge::new(kernel);

        let names = bridge.tool_names();
        // 6 always-on + 17 kernel domain = 23 ... plus knowledge = 24
        assert_eq!(names.len(), 24, "expected 24 tools, got {:?}", names);
    }
}
