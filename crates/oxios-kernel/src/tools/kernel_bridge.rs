//! KernelToolProvider bridge — plugs oxios kernel tools into oxi-sdk agent builder.
//!
//! Implements [`oxi_sdk::KernelToolProvider`] so that oxios kernel tools
//! (exec, memory, browser, etc.) can be registered into the SDK's
//! `AgentBuilder` via `.kernel_tools()`.

use std::sync::Arc;

use oxi_agent::SearchCache;
use oxi_sdk::ToolRegistry;
use oxi_sdk::{
    KernelToolContext as SdkKernelToolContext,
    KernelToolProvider as SdkKernelToolProvider,
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
            "read", "write", "edit", "grep", "find", "ls",
            // Kernel domain
            "exec", "memory_read", "memory_write", "memory_search",
            "space", "agent", "a2a_delegate", "a2a_send", "a2a_query",
            "persona", "program", "cron", "security", "budget", "resource", "mcp",
            "browser",
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
    #[test]
    fn test_tool_names_length() {
        // Build a minimal KernelHandle for testing
        let state_store = Arc::new(
            oxios_kernel::state_store::StateStore::new(std::path::PathBuf::from("/tmp/oxios-test-workspace"))
                .unwrap(),
        );

        let kernel = Arc::new(oxios_kernel::KernelHandle::new(
            oxios_kernel::StateApi::new(state_store.clone()),
            oxios_kernel::AgentApi::new(
                Arc::new(oxios_kernel::supervisor::NoOpSupervisor),
                Arc::new(oxios_kernel::budget::BudgetManager::new()),
                Arc::new(oxios_kernel::memory::MemoryManager::new(state_store.clone())),
            ),
            oxios_kernel::SecurityApi::new(
                Arc::new(parking_lot::Mutex::new(oxios_kernel::auth::AuthManager::new())),
                Arc::new(oxios_kernel::audit_trail::AuditTrail::new(100)),
                Arc::new(parking_lot::Mutex::new(
                    oxios_kernel::access_manager::AccessManager::new(),
                )),
                state_store.clone(),
            ),
            oxios_kernel::PersonaApi::new(Arc::new(
                oxios_kernel::persona_manager::PersonaManager::new(),
            )),
            oxios_kernel::ExtensionApi::new(
                Arc::new(oxios_kernel::program::ProgramManager::new(std::path::PathBuf::from(
                    "/tmp/oxios-test/programs",
                ))),
                Arc::new(
                    oxios_kernel::skill::SkillStore::new(std::path::PathBuf::from(
                        "/tmp/oxios-test/skills",
                    ))
                    .unwrap(),
                ),
                Arc::new(oxios_kernel::host_tools::HostToolValidator::new(vec![], vec![])),
            ),
            oxios_kernel::McpApi::new(Arc::new(oxios_kernel::mcp::McpBridge::new())),
            oxios_kernel::InfraApi::new(
                Arc::new(
                    oxios_kernel::git_layer::GitLayer::new(
                        std::path::PathBuf::from("/tmp/oxios-test"),
                        false,
                    )
                    .unwrap(),
                ),
                Arc::new(oxios_kernel::scheduler::AgentScheduler::new(5, 60, 300)),
                Arc::new(oxios_kernel::cron::CronScheduler::new(state_store.clone(), 60)),
                Arc::new(oxios_kernel::resource_monitor::ResourceMonitor::new(60, 60)),
                Arc::new(oxios_kernel::event_bus::EventBus::new(256)),
                oxios_kernel::OxiosConfig::default(),
                std::time::Instant::now(),
            ),
            oxios_kernel::SpaceApi::new(
                Arc::new(
                    oxios_kernel::space::SpaceManager::new(state_store.clone(), Arc::new(
                        oxios_kernel::event_bus::EventBus::new(256),
                    ))
                    .unwrap(),
                ),
                Arc::new(oxios_kernel::event_bus::EventBus::new(256)),
            ),
            oxios_kernel::ExecApi::new(
                Arc::new(oxios_kernel::config::ExecConfig::default()),
                Arc::new(parking_lot::Mutex::new(
                    oxios_kernel::access_manager::AccessManager::new(),
                )),
            ),
            oxios_kernel::BrowserApi::default(),
            oxios_kernel::A2aApi::new(Arc::new(oxios_kernel::a2a::A2AProtocol::new(Arc::new(
                oxios_kernel::event_bus::EventBus::new(256),
            )))),
        ));

        let bridge = OxiosKernelBridge::new(kernel);

        let names = bridge.tool_names();
        // 6 always-on + 12 kernel domain = 18 tools
        assert_eq!(names.len(), 18, "expected 18 tools, got {:?}", names);
    }
}