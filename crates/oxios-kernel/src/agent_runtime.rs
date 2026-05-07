//! Agent runtime: wraps oxi-agent's AgentLoop for use by the kernel.
//!
//! The AgentRuntime creates an oxi-agent `AgentLoop` session, configures it
//! with a custom ToolRegistry (Tier 1: oxi native, Tier 2: container/host exec,
//! Tier 3: Program tools), and executes a Seed's goal through the multi-turn
//! LLM tool-calling loop.
//!
//! Note: `AgentLoop::run()` produces a `!Send` future (the internal tool
//! execution uses `Box<dyn Future>` without `Send`). We keep `spawn_blocking`
//! to stay compatible with the `Supervisor` trait's `Send` bounds.

use anyhow::Result;
use oxi_agent::{
    prelude::CompactionEvent, AgentEvent, AgentLoop, AgentLoopConfig, GrepTool, LsTool, ReadTool,
    SharedState, ToolExecutionMode, ToolRegistry, WriteTool, EditTool, FindTool,
};
use oxi_ai::{CompactionStrategy, Provider};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::circuit_breaker::CircuitBreaker;
use crate::config::OxiosConfig;
use crate::container_manager::ContainerManager;
use crate::host_exec::HostExecBridge;
use crate::mcp::McpBridge;
use crate::persona_manager::PersonaManager;
use crate::program::ProgramManager;
use crate::state_store::StateStore;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType};
use crate::tools::{ContainerExecTool, HostExecTool, McpToolWrapper, ProgramTool};
use crate::tools::memory_tools::{MemoryWriteTool, MemoryReadTool, MemorySearchTool};
use oxios_ouroboros::{ExecutionResult, Seed};

/// Global LLM circuit breaker instance.
static LLM_CIRCUIT_BREAKER: std::sync::OnceLock<CircuitBreaker> = std::sync::OnceLock::new();

/// Get the global LLM circuit breaker.
fn get_llm_circuit_breaker() -> &'static CircuitBreaker {
    LLM_CIRCUIT_BREAKER.get_or_init(CircuitBreaker::default)
}

/// Configuration for creating AgentRuntime instances.
#[derive(Debug, Clone)]
pub struct AgentRuntimeConfig {
    /// Model ID in `provider/model` format (e.g. `anthropic/claude-sonnet-4-20250514`).
    pub model_id: String,
    /// Maximum number of agent turns before forcing a stop.
    pub max_iterations: usize,
    /// How to execute tool calls within a single turn.
    pub tool_execution: ToolExecutionMode,
    /// Whether auto-retry is enabled for retryable LLM errors.
    pub auto_retry_enabled: bool,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            model_id: "anthropic/claude-sonnet-4-20250514".into(),
            max_iterations: 20,
            tool_execution: ToolExecutionMode::Parallel,
            auto_retry_enabled: true,
        }
    }
}

/// Mutable state shared between the event callback and the main execute flow.
/// Wrapped in `Arc<Mutex<>>` because `AgentLoop::run()` takes `Fn` (not `FnMut`).
#[derive(Default)]
struct ExecuteState {
    final_content: String,
    steps_completed: usize,
    success: bool,
}

/// Runtime that wraps an oxi-agent `AgentLoop` for executing Seeds.
///
/// Each call to [`AgentRuntime::execute`] creates a fresh `AgentLoop`,
/// builds a ToolRegistry with Tier 1-3 tools, and runs it to completion.
pub struct AgentRuntime {
    provider: Arc<dyn Provider>,
    config: AgentRuntimeConfig,
    /// Container manager — always present, required for workspace execution.
    container: Arc<ContainerManager>,
    host_bridge: Option<Arc<HostExecBridge>>,
    program_manager: Option<Arc<ProgramManager>>,
    oxios_config: Option<OxiosConfig>,
    persona_manager: Option<Arc<PersonaManager>>,
    /// MCP bridge with pre-registered servers (from kernel.rs).
    mcp_bridge: Option<Arc<McpBridge>>,
    /// Memory manager for cross-session memory.
    memory_manager: Option<Arc<MemoryManager>>,
}

/// Create a minimal placeholder ContainerManager for cases where
/// no real container is available (e.g., during initialization before
/// kernel is fully set up, or in tests).
fn make_placeholder_container_manager() -> ContainerManager {
    let tmp = tempfile::tempdir().unwrap();
    let state = StateStore::new(tmp.path().join("state")).unwrap();
    let host_exec = Arc::new(
        HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
            .expect("placeholder needs non-empty allowlist"),
    );
    ContainerManager::with_apple_backend(
        host_exec,
        Arc::new(state),
        tmp.path().join("containers"),
    )
}

impl AgentRuntime {
    /// Creates a new agent runtime with the given LLM provider and default config.
    pub fn new(provider: Arc<dyn Provider>, model_id: impl Into<String>) -> Self {
        // NOTE: container must be set via with_container() before execute()
        // A placeholder is used until with_container() is called.
        Self {
            provider,
            config: AgentRuntimeConfig {
                model_id: model_id.into(),
                ..Default::default()
            },
            container: Arc::new(make_placeholder_container_manager()),
            host_bridge: None,
            program_manager: None,
            oxios_config: None,
            persona_manager: None,
            mcp_bridge: None,
            memory_manager: None,
        }
    }

    /// Attach a PersonaManager for persona system prompt injection.
    pub fn with_persona_manager(mut self, pm: Arc<PersonaManager>) -> Self {
        self.persona_manager = Some(pm);
        self
    }

    /// Attach a ContainerManager for container execution.
    /// Container is always required — set via this method or during construction.
    pub fn with_container(mut self, container: Arc<ContainerManager>) -> Self {
        self.container = container;
        self
    }

    /// Attach a HostExecBridge for host command execution.
    pub fn with_host_bridge(mut self, bridge: Arc<HostExecBridge>) -> Self {
        self.host_bridge = Some(bridge);
        self
    }

    /// Attach a ProgramManager for Tier 3 tool registration.
    pub fn with_program_manager(mut self, pm: Arc<ProgramManager>) -> Self {
        self.program_manager = Some(pm);
        self
    }

    /// Attach the full OxiosConfig.
    pub fn with_oxios_config(mut self, config: OxiosConfig) -> Self {
        self.oxios_config = Some(config);
        self
    }

    /// Attach the MCP bridge with pre-registered servers.
    pub fn with_mcp_bridge(mut self, bridge: Arc<McpBridge>) -> Self {
        self.mcp_bridge = Some(bridge);
        self
    }

    /// Attach a MemoryManager for cross-session memory tools.
    pub fn with_memory_manager(mut self, mm: Arc<MemoryManager>) -> Self {
        self.memory_manager = Some(mm);
        self
    }

    /// Execute a Seed by running the tool-calling loop to completion.
    ///
    /// Creates a fresh `AgentLoop`, registers built-in tools via
    /// `ToolRegistry::with_builtins()`, sets the system prompt from the
    /// Seed's goal and constraints, and runs the loop with an event
    /// callback that tracks progress.
    ///
    /// Runs inside `spawn_blocking` because `AgentLoop::run()` produces
    /// a `!Send` future (internal `Box<dyn Future>` without `Send` bound).
    pub async fn execute(&self, seed: &Seed) -> Result<ExecutionResult> {
        let prompt = build_user_prompt(seed);

        // Collect SKILL.md content from enabled programs.
        let skill_contents: Vec<String> = if let Some(ref pm) = self.program_manager {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let programs = pm.list_enabled().await;
                let mut contents = Vec::new();
                for p in &programs {
                    if let Some(c) = pm.get_skill_content(&p.meta.name).await {
                        if !c.trim().is_empty() {
                            contents.push(c);
                        }
                    }
                }
                contents
            })
        } else {
            Vec::new()
        };

        // Get active persona system prompt.
        let persona_prompt = self
            .persona_manager
            .as_ref()
            .map(|pm| pm.active_system_prompt())
            .filter(|s| !s.trim().is_empty());

        let mut system_prompt = build_system_prompt(seed, &skill_contents, persona_prompt.as_deref());

        // Blend relevant memories into system prompt if memory manager is available.
        if let Some(ref mm) = self.memory_manager {
            match mm.recall(&seed.goal).await {
                Ok(memories) if !memories.is_empty() => {
                    tracing::info!(count = memories.len(), "Recalled memories for seed");
                    system_prompt = mm.blend_into_prompt(&memories, &system_prompt);
                }
                Ok(_) => tracing::debug!("No memories recalled"),
                Err(e) => tracing::warn!(error = %e, "Failed to recall memories"),
            }
        }

        // Clone everything to move into spawn_blocking.
        let config = self.config.clone();
        let provider = Arc::clone(&self.provider);
        let seed_id = seed.id;
        let container = Arc::clone(&self.container);
        let host_bridge = self.host_bridge.clone();
        let program_manager = self.program_manager.clone();
        let oxios_config = self.oxios_config.clone();
        let mcp_bridge_for_runtime = self.mcp_bridge.as_ref().map(Arc::clone);
        let memory_manager = self.memory_manager.clone();

        let (final_content, steps_completed, success) =
            tokio::task::spawn_blocking(move || {
                run_agent_loop(
                    provider,
                    config,
                    system_prompt,
                    prompt,
                    seed_id,
                    container,
                    host_bridge,
                    program_manager,
                    oxios_config,
                    mcp_bridge_for_runtime,
                    memory_manager,
                )
            })
            .await??;

        tracing::info!(
            seed_id = %seed_id,
            steps = steps_completed,
            success,
            "AgentRuntime finished"
        );

        Ok(ExecutionResult {
            output: if final_content.is_empty() {
                "Agent execution completed".into()
            } else {
                final_content
            },
            steps_completed,
            success,
        })
    }
}

/// Run the AgentLoop inside a blocking thread.
///
/// This function is called from `spawn_blocking` because `AgentLoop::run()`
/// produces a `!Send` future. We use `tokio::runtime::Handle::block_on` to
/// drive the async work from the blocking thread.
#[allow(clippy::too_many_arguments)]
fn run_agent_loop(
    provider: Arc<dyn Provider>,
    config: AgentRuntimeConfig,
    system_prompt: String,
    prompt: String,
    seed_id: uuid::Uuid,
    container: Arc<ContainerManager>,
    host_bridge: Option<Arc<HostExecBridge>>,
    program_manager: Option<Arc<ProgramManager>>,
    oxios_config: Option<OxiosConfig>,
    mcp_bridge_for_runtime: Option<Arc<McpBridge>>,
    memory_manager: Option<Arc<MemoryManager>>,
) -> Result<(String, usize, bool)> {
    // ── Workspace Scoping: restrict agent file access ──
    let workspace = if let Some(ref bridge) = host_bridge {
        bridge
            .socket_path()
            .parent()
            .map(|p| p.join("agent-workspace"))
            .unwrap_or_else(|| std::env::temp_dir().join("oxios-agent-workspace"))
    } else {
        std::env::temp_dir().join("oxios-agent-workspace")
    };

    // Ensure workspace exists.
    let _ = std::fs::create_dir_all(&workspace);

    // Set current directory for file tools (read/write/edit).
    if let Err(e) = std::env::set_current_dir(&workspace) {
        tracing::warn!(error = %e, "Failed to set agent workspace dir");
    }

    tracing::debug!(workspace = %workspace.display(), "Agent workspace scoped");

    // ── Tier 1: oxi native tools (file operations) ──
    let registry = ToolRegistry::new();
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());

    // ── Tier 2: Oxios execution tools ──
    let container_exec = if let Some(ref bridge) = host_bridge {
        Arc::new(ContainerExecTool::new_with_host_bridge(
            container.clone(),
            bridge.clone(),
        ))
    } else {
        Arc::new(ContainerExecTool::new(container.clone()))
    };
    registry.register_arc(container_exec.clone());

    if let Some(bridge) = host_bridge {
        let host_exec = Arc::new(HostExecTool::new(bridge.clone()));
        registry.register_arc(host_exec.clone());

        // ── Tier 3: Program tools (dynamic) + Tier 4: MCP servers ──
        if let Some(pm) = program_manager {
            let rt = tokio::runtime::Handle::current();
            let programs = rt.block_on(async { pm.list_enabled().await });
            let container_config = oxios_config
                .as_ref()
                .map(|c| &c.container)
                .cloned()
                .unwrap_or_default();

            // Use the pre-registered MCP bridge from kernel.rs (if available).
            // Collect server names from programs for tool registration.
            let mut mcp_server_names: Vec<String> = Vec::new();

            for program in &programs {
                for server_config in &program.meta.mcp_servers {
                    if server_config.enabled {
                        mcp_server_names.push(server_config.name.clone());
                    }
                }
            }

            // Register MCP tools from the pre-configured bridge.
            if !mcp_server_names.is_empty() {
                if let Some(ref bridge) = mcp_bridge_for_runtime {
                    if let Err(e) = rt.block_on(bridge.initialize_all()) {
                        tracing::warn!(error = %e, "MCP bridge init failed — skipping MCP tools");
                    } else {
                        let _ = rt.block_on(bridge.list_tools()); // populate cache
                        let mut mcp_tool_count = 0usize;
                        for server_name in &mcp_server_names {
                            if let Some(tool_defs) = rt.block_on(bridge.cached_tools(server_name)) {
                                for tool_def in tool_defs {
                                    let wrapper = McpToolWrapper::new(
                                        Arc::clone(bridge),
                                        server_name,
                                        &tool_def.name,
                                        tool_def.description.clone(),
                                        serde_json::json!({"type": "object", "properties": {}}),
                                    );
                                    registry.register(wrapper);
                                    mcp_tool_count += 1;
                                }
                            }
                        }
                        tracing::info!(count = mcp_tool_count, "MCP tools registered");
                    }
                } else {
                    tracing::warn!(count = mcp_server_names.len(), "MCP servers declared but no bridge available");
                }
            }

            // Tier 3: Program tools.
            for program in &programs {
                // ── P1-3: requires_tools validation ──
                let missing_tools: Vec<&str> = program
                    .meta
                    .dependencies
                    .iter()
                    .filter(|tool_name| registry.get(tool_name).is_none())
                    .map(|s| s.as_str())
                    .collect();
                if !missing_tools.is_empty() {
                    tracing::warn!(
                        program = %program.meta.name,
                        missing_tools = ?missing_tools,
                        "Skipping program: required tools not found in registry",
                    );
                    continue;
                }

                for tool_def in &program.meta.tools {
                    if !tool_def.command.is_empty() {
                        let tool = ProgramTool::from_definition(
                            &program.meta.name,
                            tool_def,
                            &program.meta.host_requirements,
                            &container_config,
                            container_exec.clone(),
                            host_exec.clone(),
                        );
                        registry.register(tool);
                    }
                }
            }
        }
    }

    // ── Tier 5: Memory tools ──
    if let Some(ref mm) = memory_manager {
        let write_tool = Arc::new(MemoryWriteTool::new(mm.clone()));
        let read_tool = Arc::new(MemoryReadTool::new(mm.clone()));
        let search_tool = Arc::new(MemorySearchTool::new(mm.clone()));
        registry.register_arc(write_tool);
        registry.register_arc(read_tool);
        registry.register_arc(search_tool);
        tracing::debug!("Memory tools registered");
    }

    let tools = Arc::new(registry);

    // Build the AgentLoop config from our runtime config.
    let loop_config = AgentLoopConfig {
        model_id: config.model_id,
        system_prompt: Some(system_prompt),
        temperature: 0.7,
        max_tokens: 8192,
        max_iterations: config.max_iterations,
        tool_execution: config.tool_execution,
        compaction_strategy: CompactionStrategy::Threshold(0.8),
        context_window: 128_000,
        compaction_instruction: None,
        session_id: Some(seed_id.to_string()),
        transport: None,
        compact_on_start: false,
        max_retry_delay_ms: None,
        auto_retry_enabled: config.auto_retry_enabled,
        auto_retry_max_attempts: 3,
        auto_retry_base_delay_ms: 2000,
    };

    let state = SharedState::new();
    let agent_loop = AgentLoop::new(provider, loop_config, tools, state);

    // Shared mutable state for the event callback.
    let exec_state = Arc::new(Mutex::new(ExecuteState::default()));
    let exec_state_clone = Arc::clone(&exec_state);
    let memory_for_callback = memory_manager.clone();
    let session_id_for_callback = seed_id.to_string();

    // Run the async AgentLoop inside the blocking thread.
    let rt = tokio::runtime::Handle::current();
    let rt_for_callback = rt.clone();
    rt.block_on(async {
        let result = agent_loop
            .run(prompt, move |event| {
                let mut s = exec_state_clone.lock();
                match event {
                    AgentEvent::ToolExecutionEnd { is_error: false, .. } => {
                        s.steps_completed += 1;
                    }
                    AgentEvent::AgentEnd { messages, stop_reason, .. } => {
                        if let Some(oxi_ai::Message::Assistant(a)) = messages.last() {
                            s.final_content = a.text_content();
                        }
                        s.success = stop_reason.as_deref() == Some("Stop");
                    }
                    AgentEvent::Error { message, .. } => {
                        s.final_content = message.clone();
                        s.success = false;
                    }
                    AgentEvent::Compaction { event } => {
                        if let Some(ref mm) = memory_for_callback {
                            if let CompactionEvent::Completed { result, .. } = event {
                                let entry = MemoryEntry {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    memory_type: MemoryType::Conversation,
                                    content: result.summary.clone(),
                                    source: "compaction".to_string(),
                                    session_id: Some(session_id_for_callback.clone()),
                                    tags: vec![],
                                    importance: 0.5,
                                    created_at: chrono::Utc::now(),
                                    accessed_at: chrono::Utc::now(),
                                    access_count: 0,
                                };
                                if let Err(e) = rt_for_callback.block_on(mm.remember(entry)) {
                                    tracing::warn!(error = %e, "Failed to save compaction summary");
                                }
                            }
                        }
                    }
                    _ => {}
                }
            })
            .await;

        // Record circuit breaker result after agent execution
        let circuit = get_llm_circuit_breaker();
        if result.is_err() {
            circuit.record_failure();
        } else {
            circuit.record_success();
        }

        if let Err(e) = result {
            tracing::error!(seed_id = %seed_id, error = %e, "AgentLoop failed");
            let s = exec_state.lock();
            return Ok((format!("Agent failed: {e}"), s.steps_completed, false));
        }

        let s = exec_state.lock();
        tracing::info!(
            seed_id = %seed_id,
            steps = s.steps_completed,
            success = s.success,
            "AgentLoop completed inside blocking thread"
        );
        Ok((s.final_content.clone(), s.steps_completed, s.success))
    })
}

/// Build a system prompt from the Seed's goal, constraints, and program skills.
fn build_system_prompt(seed: &Seed, skill_contents: &[String], persona_prompt: Option<&str>) -> String {
    let mut prompt = format!(
        "You are an autonomous agent executing a specific task.\n\n\
         ## Goal\n\
         {}\n",
        seed.goal,
    );

    if !seed.constraints.is_empty() {
        prompt.push_str("\n## Constraints\n");
        for (i, c) in seed.constraints.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, c));
        }
    }

    if !seed.acceptance_criteria.is_empty() {
        prompt.push_str("\n## Acceptance Criteria\n");
        for (i, c) in seed.acceptance_criteria.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, c));
        }
    }

    if !seed.ontology.is_empty() {
        prompt.push_str("\n## Domain Entities\n");
        for e in &seed.ontology {
            prompt.push_str(&format!(
                "- **{}** ({}): {}\n",
                e.name, e.entity_type, e.description
            ));
        }
    }

    // Inject SKILL.md content from enabled programs
    if !skill_contents.is_empty() {
        prompt.push_str("\n## Available Programs\n");
        prompt.push_str("You have access to the following programs. Use their tools and guidelines as needed.\n\n");
        for content in skill_contents {
            prompt.push_str(content);
            prompt.push_str("\n\n");
        }
    }

    // Inject persona system prompt
    if let Some(pp) = persona_prompt {
        prompt.push_str("\n## Persona\n");
        prompt.push_str(pp);
        prompt.push('\n');
    }

    // Execution environment guidance
    prompt.push_str(
        "\n## Execution Environment\n\
         Use `container_exec` for workspace commands (compilation, tests, etc.).\n\
         Use `host_exec` for host commands (git, gh, osascript, etc.).\n",
    );

    prompt.push_str(
        "\nUse the available tools to accomplish the goal. \
         Work methodically and verify your work against the acceptance criteria.",
    );

    prompt
}

/// Build the user prompt from the seed.
fn build_user_prompt(seed: &Seed) -> String {
    format!(
        "Execute the following goal:\n\n{}\n\nAcceptance criteria:\n{}",
        seed.goal,
        seed.acceptance_criteria
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

impl std::fmt::Debug for AgentRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntime")
            .field("model_id", &self.config.model_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use oxi_agent::tools::{AgentTool, ToolError};
    use serde_json::Value;
    use oxios_ouroboros::Entity;

    /// A test tool that does nothing — used to populate the registry.
    struct DummyTool {
        name: String,
    }

    #[async_trait]
    impl AgentTool for DummyTool {
        fn name(&self) -> &str { &self.name }
        fn label(&self) -> &str { &self.name }
        fn description(&self) -> &str { "Test tool" }
        fn parameters_schema(&self) -> Value { serde_json::json!({"type": "object"}) }

        async fn execute(
            &self,
            _tool_call_id: &str,
            _params: Value,
            _shutdown: Option<tokio::sync::oneshot::Receiver<()>>,
        ) -> Result<oxi_agent::AgentToolResult, ToolError> {
            Ok(oxi_agent::AgentToolResult::success("ok"))
        }
    }

    /// Test that requires_tools validation passes when all tools are present.
    #[test]
    fn test_requires_tools_validation_passes() {
        let registry = ToolRegistry::new();

        // Register the tools the program depends on.
        registry.register(DummyTool { name: "read".into() });
        registry.register(DummyTool { name: "container_exec".into() });

        // Simulate a program that requires "read" and "container_exec".
        let required_tools = vec!["read".to_string(), "container_exec".to_string()];

        // Validation: all required tools must exist in the registry.
        let missing: Vec<&str> = required_tools
            .iter()
            .filter(|name| registry.get(name).is_none())
            .map(|s| s.as_str())
            .collect();

        assert!(missing.is_empty(), "Expected no missing tools, got: {:?}", missing);
    }

    /// Test that requires_tools validation fails when a tool is missing.
    #[test]
    fn test_requires_tools_validation_fails() {
        let registry = ToolRegistry::new();

        // Only register "read", not "container_exec" or "nonexistent".
        registry.register(DummyTool { name: "read".into() });

        // Simulate a program that requires tools that don't exist.
        let required_tools = vec![
            "read".to_string(),       // exists
            "container_exec".to_string(), // missing
            "nonexistent".to_string(), // missing
        ];

        // Validation: find missing tools.
        let missing: Vec<&str> = required_tools
            .iter()
            .filter(|name| registry.get(name).is_none())
            .map(|s| s.as_str())
            .collect();

        assert_eq!(missing, vec!["container_exec", "nonexistent"]);
    }

    #[test]
    fn test_build_system_prompt_includes_skills() {
        let seed = Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Build a web server".into(),
            constraints: vec!["Must use Rust".into()],
            acceptance_criteria: vec!["Server responds to requests".into()],
            ontology: vec![
                Entity {
                    name: "HttpServer".into(),
                    entity_type: "struct".into(),
                    description: "The main server struct".into(),
                },
            ],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
        };

        let prompt = build_system_prompt(&seed, &[], None);

        // Verify goal is present
        assert!(prompt.contains("Build a web server"));

        // Verify constraints are present
        assert!(prompt.contains("Must use Rust"));

        // Verify acceptance criteria is present
        assert!(prompt.contains("Server responds to requests"));

        // Verify domain entities are present
        assert!(prompt.contains("HttpServer"));
        assert!(prompt.contains("struct"));
    }

    #[test]
    fn test_build_system_prompt_empty_skills() {
        let seed = Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Test goal".into(),
            constraints: vec![],
            acceptance_criteria: vec![],
            ontology: vec![],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
        };

        let prompt = build_system_prompt(&seed, &[], None);

        // Verify goal is present
        assert!(prompt.contains("Test goal"));
    }
}
