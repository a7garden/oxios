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
            "space",
            "agent",
            "a2a_delegate",
            "a2a_send",
            "a2a_query",
            "persona",
            "program",
            "cron",
            "security",
            "budget",
            "resource",
            "mcp",
            "browser",
            "knowledge",
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
        // Build a minimal KernelHandle for testing
        let state_store = Arc::new(
            crate::state_store::StateStore::new(std::path::PathBuf::from(
                "/tmp/oxios-test-workspace",
            ))
            .unwrap(),
        );

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
                Arc::new(crate::program::ProgramManager::new(
                    std::path::PathBuf::from("/tmp/oxios-test/programs"),
                )),
                Arc::new(
                    crate::skill::SkillStore::new(std::path::PathBuf::from(
                        "/tmp/oxios-test/skills",
                    ))
                    .unwrap(),
                ),
                Arc::new(crate::host_tools::HostToolValidator::new(vec![], vec![])),
            ),
            crate::McpApi::new(Arc::new(crate::mcp::McpBridge::new())),
            crate::InfraApi::new(
                Arc::new(
                    crate::git_layer::GitLayer::new(
                        std::path::PathBuf::from("/tmp/oxios-test"),
                        false,
                    )
                    .unwrap(),
                ),
                Arc::new(crate::scheduler::AgentScheduler::new(5, 60, 300)),
                Arc::new(crate::cron::CronScheduler::new(state_store.clone(), 60)),
                Arc::new(crate::resource_monitor::ResourceMonitor::new(60, 60)),
                crate::event_bus::EventBus::new(256),
                crate::OxiosConfig::default(),
                std::time::Instant::now(),
            ),
            crate::SpaceApi::new(
                Arc::new(
                    crate::space::SpaceManager::new(
                        state_store.clone(),
                        crate::event_bus::EventBus::new(256),
                    )
                    .await
                    .unwrap(),
                ),
                crate::event_bus::EventBus::new(256),
            ),
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
            crate::KnowledgeApi::new(
                std::path::PathBuf::from("/tmp/oxios-test/knowledge"),
                Arc::new(crate::memory::MemoryManager::new(state_store.clone())),
                Arc::new(crate::engine::OxiEngineProvider::new("anthropic/claude-sonnet-4")),
                "anthropic/claude-sonnet-4".to_string(),
            ),
            Arc::new(crate::kernel_handle::KnowledgeLens::new(
                Arc::new(oxios_markdown::KnowledgeBase::new(
                    std::path::PathBuf::from("/tmp/oxios-test/knowledge"),
                ).unwrap()),
                Arc::new(crate::memory::MemoryManager::new(state_store.clone())),
            ).unwrap()),
        ));

        let bridge = OxiosKernelBridge::new(kernel);

        let names = bridge.tool_names();
        // 6 always-on + 17 kernel domain = 23 ... plus knowledge = 24
        assert_eq!(names.len(), 24, "expected 24 tools, got {:?}", names);
    }
}
