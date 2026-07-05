//! Oxios sub-agent runner — in-process delegation via oxi-sdk 0.54.0+.
//!
//! Wraps [`oxi_sdk::SdkSubagentRunner`] (which wraps an `Oxi` instance)
//! and exposes it as an [`oxi_agent::SubagentRunner`] for the `subagent`
//! tool's in-process path. Each `run_isolated` call builds a fresh `Agent`
//! with an empty context (full isolation from the parent), runs it, and
//! returns only the final text + usage.
//!
//! # Security model
//!
//! The sub-agent built by `SdkSubagentRunner` has **zero tools** — it
//! calls `self.oxi.agent(config).build()` with no `.tool()` / `.coding_tools()`
//! / `.kernel_tools()` registration. A tool-less agent can only do pure
//! text generation: no file access, no bash, no network, no side effects.
//! This makes the sandbox-escape vector from RFC-035 §4.3 currently moot
//! for this runner.
//!
//! **Defense-in-depth upgrade path:** if a future `SdkSubagentRunner`
//! version starts honoring the `_tools` parameter or auto-registers
//! built-in tools, switch this module to delegate through
//! `AgentLifecycleManager::execute_directive` instead (RFC-035 Q2-B),
//! which inherits `allowed_tools`/`network_access`/`max_execution_time_secs`/
//! `access_manager` by construction. The "wrinkle" (ExecutionResult has
//! no token usage) is resolvable by sourcing usage from `AgentEvent::Usage`.
//!
//! # Depth safety
//!
//! The runner sets the forked agent's `subagent_depth` to `depth + 1`.
//! The SDK hardcodes the in-process max to 3 (`subagent.rs:649`), so
//! recursion is bounded without env vars (concurrent `set_var` is UB).

use std::sync::Arc;

use oxi_sdk::SdkSubagentRunner;

/// Oxios's in-process sub-agent runner.
///
/// Constructed once at boot from the [`OxiosEngine`]'s `Oxi` instance
/// and shared via `Arc` into every agent build path. When wired into
/// `AgentConfig.subagent_runner`, the `subagent` tool prefers this
/// in-process path over shelling out to the CLI binary.
#[derive(Clone)]
pub struct OxiosSubagentRunner {
    inner: SdkSubagentRunner,
}

impl std::fmt::Debug for OxiosSubagentRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxiosSubagentRunner")
            .field("inner", &"<SdkSubagentRunner>")
            .finish()
    }
}

impl OxiosSubagentRunner {
    /// Create from an [`oxi_sdk::Oxi`] engine instance.
    ///
    /// The `Oxi` instance is `Arc`-backed, so this is clone-cheap and
    /// safe to share across concurrent tasks.
    pub fn new(oxi: oxi_sdk::Oxi) -> Self {
        Self {
            inner: SdkSubagentRunner::new(oxi),
        }
    }

    /// Return an `Arc<dyn SubagentRunner>` suitable for wiring into
    /// `AgentConfig::subagent_runner`.
    pub fn into_trait_object(self) -> Arc<dyn oxi_agent::SubagentRunner> {
        Arc::new(self)
    }
}

#[async_trait::async_trait]
impl oxi_agent::SubagentRunner for OxiosSubagentRunner {
    async fn run_isolated(
        &self,
        agent_name: &str,
        task: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        tools: &[String],
        cwd: &std::path::Path,
        depth: u8,
    ) -> anyhow::Result<oxi_agent::ForkResult> {
        // Delegate to the SDK's reference implementation. The sub-agent
        // is tool-less (see Security model above), so no sandbox escape.
        self.inner
            .run_isolated(agent_name, task, system_prompt, model, tools, cwd, depth)
            .await
    }
}
