//! Agent runtime: wraps oxi-agent's AgentLoop for use by the kernel.
//!
//! The AgentRuntime creates an oxi-agent `AgentLoop` session, configures it
//! with a custom ToolRegistry based on the agent's CSpace (capability space),
//! and executes a Seed's goal through the multi-turn LLM tool-calling loop.
//!
//! # Architecture
//!
//! All tool access goes through `KernelHandle` — the single syscall-table-like
//! path for agent OS control. The runtime:
//!
//! 1. Resolves the agent's CSpace from persona/role/hint
//! 2. Registers tools via `register_tools_from_cspace()`
//! 3. Optionally queries `ToolRetriever` for semantic capability hints
//! 4. Runs the agent loop with the assembled tool set
//!
//! Note: `AgentLoop::run()` produces a `!Send` future (the internal tool
//! execution uses `Box<dyn Future>` without `Send`). We keep `spawn_blocking`
//! to stay compatible with the `Supervisor` trait's `Send` bounds.

use anyhow::Result;
use oxi_agent::{prelude::CompactionEvent, SearchCache};
use oxi_sdk::ToolExecutionMode;
use oxi_sdk::{AgentEvent, AgentLoop, AgentLoopConfig, SharedState, ToolRegistry};
use oxi_sdk::{CompactionStrategy, Provider};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::capability::resolve::resolve_cspace;
use crate::circuit_breaker::CircuitBreaker;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType};
use crate::persona_manager::PersonaManager;
use crate::tools::registration::register_tools_from_cspace;
use crate::types::AgentId;
use crate::KernelHandle;
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
    /// Space ID for scoped memory and workspace.
    pub space_id: Option<uuid::Uuid>,
    /// Bound project paths. AgentRuntime sets CWD to paths[0].
    pub project_paths: Vec<std::path::PathBuf>,
    /// Scratch workspace directory for temp files.
    pub workspace_dir: Option<std::path::PathBuf>,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            model_id: "anthropic/claude-sonnet-4-20250514".into(),
            max_iterations: 20,
            tool_execution: ToolExecutionMode::Parallel,
            auto_retry_enabled: true,
            space_id: None,
            project_paths: Vec::new(),
            workspace_dir: None,
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

/// Bundled context for `run_agent_loop()`.
///
/// All kernel access goes through `kernel_handle`. The CSpace determines
/// which tools are registered for this agent.
struct AgentLoopContext {
    provider: Arc<dyn Provider>,
    config: AgentRuntimeConfig,
    system_prompt: String,
    prompt: String,
    seed_id: uuid::Uuid,
    agent_id: AgentId,
    kernel_handle: Arc<KernelHandle>,
    cspace: crate::capability::CSpace,
    /// Persona prompt for system prompt blending.
    #[allow(dead_code)]
    persona_prompt: Option<String>,
}

/// Runtime that wraps an oxi-agent `AgentLoop` for executing Seeds.
///
/// Each call to [`AgentRuntime::execute`] creates a fresh `AgentLoop`,
/// builds a ToolRegistry based on the agent's CSpace, and runs it to completion.
///
/// All OS-level access goes through `KernelHandle` — the single syscall-table
/// for agent control.
pub struct AgentRuntime {
    provider: Arc<dyn Provider>,
    config: AgentRuntimeConfig,
    /// Single path to all kernel services.
    kernel_handle: Arc<KernelHandle>,
    /// Persona manager for system prompt injection.
    persona_manager: Option<Arc<PersonaManager>>,
    /// Semantic tool retriever for capability discovery.
    tool_retriever: Option<Arc<crate::tools::retrieval::ToolRetriever>>,
}

impl AgentRuntime {
    /// Creates a new agent runtime with kernel access.
    ///
    /// All tool access goes through `kernel_handle`.
    pub fn new(
        provider: Arc<dyn Provider>,
        model_id: impl Into<String>,
        kernel_handle: Arc<KernelHandle>,
    ) -> Self {
        Self {
            provider,
            config: AgentRuntimeConfig {
                model_id: model_id.into(),
                ..Default::default()
            },
            kernel_handle,
            persona_manager: None,
            tool_retriever: None,
        }
    }

    /// Attach a PersonaManager for persona system prompt injection.
    pub fn with_persona_manager(mut self, pm: Arc<PersonaManager>) -> Self {
        self.persona_manager = Some(pm);
        self
    }

    /// Set the runtime config (overrides defaults).
    pub fn with_config(mut self, config: AgentRuntimeConfig) -> Self {
        self.config = config;
        self
    }

    /// Attach a ToolRetriever for semantic capability discovery.
    pub fn with_tool_retriever(
        mut self,
        retriever: Arc<crate::tools::retrieval::ToolRetriever>,
    ) -> Self {
        self.tool_retriever = Some(retriever);
        self
    }

    /// Execute a Seed by running the tool-calling loop to completion.
    ///
    /// 1. Resolves CSpace from persona/role/hint
    /// 2. Registers tools via CSpace
    /// 3. Recalls memories if available
    /// 4. Runs the agent loop
    pub async fn execute(&self, agent_id: AgentId, seed: &Seed) -> Result<ExecutionResult> {
        let prompt = build_user_prompt(seed);

        // Get active persona system prompt.
        let persona_prompt = self
            .persona_manager
            .as_ref()
            .map(|pm| pm.active_system_prompt())
            .filter(|s| !s.trim().is_empty());

        // Determine persona role for CSpace resolution.
        let persona_role = self
            .persona_manager
            .as_ref()
            .and_then(|pm| pm.get_active_persona().map(|p| p.role.clone()));

        // Resolve CSpace from persona role, seed hint, or default.
        let cspace = resolve_cspace(
            seed.cspace_hint.as_deref(),
            persona_role.as_deref(),
            Some("worker"),
            agent_id,
        );

        // Build system prompt (without SKILL.md injection — capabilities are
        // surfaced through the CSpace tool set + semantic retrieval instead).
        let mut system_prompt = build_system_prompt(seed, persona_prompt.as_deref(), None, None);

        // Semantic capability retrieval: find tools relevant to this seed's goal.
        let capabilities_xml = if let Some(ref retriever) = self.tool_retriever {
            match retriever.embedder().embed(&seed.goal).await {
                Ok(query_vec) => {
                    let results = retriever.retrieve(&query_vec, 8);
                    if results.is_empty() {
                        None
                    } else {
                        let xml = crate::tools::retrieval::format_capability_index(&results);
                        tracing::info!(count = results.len(), "Retrieved relevant capabilities");
                        Some(xml)
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to embed seed goal for retrieval");
                    None
                }
            }
        } else {
            None
        };

        // Build kernel manifest from CSpace active domains.
        let kernel_manifest = {
            let domains = cspace.active_domains();
            if domains.is_empty() {
                None
            } else {
                Some(crate::tools::retrieval::build_kernel_manifest(&domains))
            }
        };

        // Rebuild system prompt with capabilities and manifest if available.
        if capabilities_xml.is_some() || kernel_manifest.is_some() {
            system_prompt = build_system_prompt(
                seed,
                persona_prompt.as_deref(),
                capabilities_xml.as_deref(),
                kernel_manifest.as_deref(),
            );
        }

        // Blend relevant memories into system prompt.
        let memory_manager = self.kernel_handle.agents.memory_manager();
        match memory_manager.recall(&seed.goal).await {
            Ok(memories) if !memories.is_empty() => {
                tracing::info!(count = memories.len(), "Recalled memories for seed");
                system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);
            }
            Ok(_) => tracing::debug!("No memories recalled"),
            Err(e) => tracing::warn!(error = %e, "Failed to recall memories"),
        }

        // Clone everything to move into spawn_blocking.
        let config = self.config.clone();
        let provider = Arc::clone(&self.provider);
        let seed_id = seed.id;
        let kernel_handle = Arc::clone(&self.kernel_handle);

        let ctx = AgentLoopContext {
            provider,
            config,
            system_prompt,
            prompt,
            seed_id,
            agent_id,
            kernel_handle,
            cspace,
            persona_prompt,
        };

        let (final_content, steps_completed, success) =
            tokio::task::spawn_blocking(move || run_agent_loop(ctx)).await??;

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
/// Run the agent loop inside a blocking thread.
///
/// `AgentLoop::run()` still captures non-`Send` state internally
/// (`FinalizedToolCallEntry::Future` lacks `+ Send`). Until oxi fixes this,
/// we use `spawn_blocking` + `Handle::block_on`.
fn run_agent_loop(ctx: AgentLoopContext) -> Result<(String, usize, bool)> {
    let AgentLoopContext {
        provider,
        config,
        system_prompt,
        prompt,
        seed_id,
        agent_id,
        kernel_handle,
        cspace,
        persona_prompt: _,
    } = ctx;

    // Extract workspace before using config
    let workspace = if !config.project_paths.is_empty() {
        config.project_paths[0].clone()
    } else if let Some(ref ws) = config.workspace_dir {
        ws.clone()
    } else {
        std::env::temp_dir()
            .join("oxios-agent-workspace")
            .join(agent_id.to_string())
    };

    // Ensure workspace exists.
    let _ = std::fs::create_dir_all(&workspace);

    tracing::debug!(workspace = %workspace.display(), "Agent workspace scoped");

    // ── Register tools based on CSpace ──
    let registry = ToolRegistry::new();
    let search_cache = Arc::new(SearchCache::new());
    register_tools_from_cspace(&registry, &kernel_handle, &cspace, search_cache, agent_id);

    tracing::info!(
        seed_id = %seed_id,
        capabilities = cspace.len(),
        "Tools registered from CSpace"
    );

    // ── Program tools: registered individually from ProgramManager ──
    let pm = kernel_handle.extensions.program_manager();

    // Create a separate ExecTool for program routing (needed by ProgramTool).
    let exec_for_programs: Option<std::sync::Arc<crate::tools::ExecTool>> = if cspace.can(
        &crate::capability::ResourceRef::Exec {
            mode: "shell".into(),
        },
        crate::capability::Rights::EXECUTE,
    ) {
        Some(std::sync::Arc::new(crate::tools::ExecTool::from_kernel(
            &kernel_handle,
        )))
    } else {
        None
    };

    {
        let rt = tokio::runtime::Handle::current();
        let programs: Vec<_> = rt.block_on(async { pm.list_enabled().await });

        // MCP bridge tools from program configs
        let mut mcp_server_names: Vec<String> = Vec::new();
        for program in &programs {
            for server_config in &program.meta.mcp_servers {
                if server_config.enabled {
                    mcp_server_names.push(server_config.name.clone());
                }
            }
        }

        if !mcp_server_names.is_empty() {
            let bridge = kernel_handle.mcp.bridge();
            if let Err(e) = rt.block_on(bridge.initialize_all()) {
                tracing::warn!(error = %e, "MCP bridge init failed — skipping MCP tools");
            } else {
                let _ = rt.block_on(bridge.list_tools());
                for server_name in &mcp_server_names {
                    if let Some(tool_defs) = rt.block_on(bridge.cached_tools(server_name)) {
                        for tool_def in tool_defs {
                            let wrapper = crate::tools::McpToolWrapper::new(
                                bridge.clone(),
                                server_name,
                                &tool_def.name,
                                tool_def.description.clone(),
                                serde_json::json!({"type": "object", "properties": {}}),
                            );
                            registry.register(wrapper);
                        }
                    }
                }
            }
        }

        // Program-defined tools
        for program in &programs {
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
                    "Skipping program: required tools not found"
                );
                continue;
            }

            for tool_def in &program.meta.tools {
                if !tool_def.command.is_empty() {
                    if let Some(ref exec) = exec_for_programs {
                        let tool = crate::tools::ProgramTool::from_definition(
                            &program.meta.name,
                            tool_def,
                            &program.meta.host_requirements,
                            exec.clone(),
                        );
                        registry.register(tool);
                    }
                }
            }
        }
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
        api_key: None,
        workspace_dir: config.project_paths.first().cloned(), // Use first project path as workspace
    };

    let state = SharedState::new();
    let agent_loop = AgentLoop::new(provider, loop_config, tools, state);

    // Shared mutable state for the event callback.
    let exec_state = Arc::new(Mutex::new(ExecuteState::default()));
    let exec_state_clone = Arc::clone(&exec_state);
    let memory_for_callback: Arc<MemoryManager> = (*kernel_handle.agents.memory_manager()).clone();
    let session_id_for_callback = seed_id.to_string();

    // Run the agent loop inside a blocking thread.
    // AgentLoop::run() captures !Send state internally.
    let rt = tokio::runtime::Handle::current();
    let rt_for_callback = rt.clone();
    rt.block_on(async {
        let result = agent_loop
            .run(prompt, move |event| {
                let mut s = exec_state_clone.lock();
                match event {
                    AgentEvent::ToolExecutionEnd {
                        is_error: false, ..
                    } => {
                        s.steps_completed += 1;
                    }
                    AgentEvent::AgentEnd {
                        messages,
                        stop_reason,
                        ..
                    } => {
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
                        let mm = &memory_for_callback;
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
            "AgentLoop completed"
        );
        Ok((s.final_content.clone(), s.steps_completed, s.success))
    })
}

/// Build a system prompt from the Seed's goal, constraints, persona,
/// and optionally a capability index and kernel manifest.
///
/// Note: SKILL.md content is no longer injected here. Capabilities are
/// surfaced through the CSpace tool set + semantic retrieval instead.
fn build_system_prompt(
    seed: &Seed,
    persona_prompt: Option<&str>,
    capabilities_xml: Option<&str>,
    kernel_manifest: Option<&str>,
) -> String {
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

    // Inject persona system prompt
    if let Some(pp) = persona_prompt {
        prompt.push_str("\n## Persona\n");
        prompt.push_str(pp);
        prompt.push('\n');
    }

    // Inject semantic capability index (from ToolRetriever)
    if let Some(xml) = capabilities_xml {
        prompt.push_str("\n## Available Capabilities\n");
        prompt.push_str("The following capabilities are relevant to your goal. ");
        prompt.push_str("Use the `read` tool to load SKILL.md for any program.\n\n");
        prompt.push_str(xml);
        prompt.push('\n');
    }

    // Inject kernel manifest (from CSpace)
    if let Some(manifest) = kernel_manifest {
        prompt.push('\n');
        prompt.push_str(manifest);
        prompt.push('\n');
    }

    // Execution environment guidance
    prompt.push_str(
        "\n## Execution Environment\n\
         Use `exec` for all command execution (git, gh, osascript, etc.).\n",
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
    use oxios_ouroboros::Entity;
    use serde_json::Value;

    /// A test tool that does nothing — used to populate the registry.
    struct DummyTool {
        name: String,
    }

    #[async_trait]
    impl AgentTool for DummyTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn label(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "Test tool"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }

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
        registry.register(DummyTool {
            name: "read".into(),
        });
        registry.register(DummyTool {
            name: "exec".into(),
        });

        // Simulate a program that requires "read" and "exec".
        let required_tools = vec!["read".to_string(), "exec".to_string()];

        // Validation: all required tools must exist in the registry.
        let missing: Vec<&str> = required_tools
            .iter()
            .filter(|name| registry.get(name).is_none())
            .map(|s| s.as_str())
            .collect();

        assert!(
            missing.is_empty(),
            "Expected no missing tools, got: {:?}",
            missing
        );
    }

    /// Test that requires_tools validation fails when a tool is missing.
    #[test]
    fn test_requires_tools_validation_fails() {
        let registry = ToolRegistry::new();

        // Only register "read", not "exec" or "nonexistent".
        registry.register(DummyTool {
            name: "read".into(),
        });

        // Simulate a program that requires tools that don't exist.
        let required_tools = vec![
            "read".to_string(),        // exists
            "exec".to_string(),        // missing
            "nonexistent".to_string(), // missing
        ];

        // Validation: find missing tools.
        let missing: Vec<&str> = required_tools
            .iter()
            .filter(|name| registry.get(name).is_none())
            .map(|s| s.as_str())
            .collect();

        assert_eq!(missing, vec!["exec", "nonexistent"]);
    }

    #[test]
    fn test_build_system_prompt_includes_goal() {
        let seed = Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Build a web server".into(),
            constraints: vec!["Must use Rust".into()],
            acceptance_criteria: vec!["Server responds to requests".into()],
            ontology: vec![Entity {
                name: "HttpServer".into(),
                entity_type: "struct".into(),
                description: "The main server struct".into(),
            }],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
        };

        let prompt = build_system_prompt(&seed, None, None, None);

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
    fn test_build_system_prompt_empty() {
        let seed = Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Test goal".into(),
            constraints: vec![],
            acceptance_criteria: vec![],
            ontology: vec![],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
        };

        let prompt = build_system_prompt(&seed, None, None, None);

        // Verify goal is present
        assert!(prompt.contains("Test goal"));
    }
}
