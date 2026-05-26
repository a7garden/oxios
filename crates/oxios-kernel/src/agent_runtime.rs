//! Agent runtime: wraps oxi-sdk's Agent for Seed execution.
//!
//! The AgentRuntime uses `OxiosEngine.oxi().agent()` (AgentBuilder pattern)
//! to construct agents with full middleware, observability, and security
//! integration from oxi-sdk 0.23.0.
//!
//! # Architecture
//!
//! All tool access goes through `KernelHandle` — the single syscall-table-like
//! path for agent OS control. The runtime:
//!
//! 1. Resolves the agent's CSpace from persona/role/hint
//! 2. Registers tools via `register_tools_from_cspace()`
//! 3. Optionally queries `ToolRetriever` for semantic capability hints
//! 4. Builds an `Agent` via `AgentBuilder` with middleware pipeline
//! 5. Runs via `Agent::run_streaming()` for real-time event processing
//!
//! # oxi-sdk 0.23.0 Integration
//!
//! Uses `AgentBuilder` for agent construction with:
//! - `.with_rate_limit()` — tool call rate limiting
//! - `.with_token_budget()` — per-execution token caps
//! - `.tracer()` / `.cost_tracker()` — observability hooks
//! - `.middleware()` — custom middleware chain

use anyhow::Result;
use oxi_sdk::{Agent, AgentConfig, AgentEvent, CompactionEvent, CompactionStrategy, ProviderResolver};
use oxi_sdk::{SearchCache, ToolExecutionMode, ToolRegistry};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::capability::resolve::resolve_cspace;
use crate::engine::OxiosEngine;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType};
use crate::persona_manager::PersonaManager;
use crate::tools::registration::register_tools_from_cspace;
use crate::types::AgentId;
use crate::KernelHandle;
use oxios_ouroboros::{ExecutionResult, Seed};

/// Global LLM circuit breaker instance — delegates to oxi-sdk's ProviderCircuitBreaker.
static LLM_CIRCUIT_BREAKER: std::sync::OnceLock<oxi_sdk::ProviderCircuitBreaker> =
    std::sync::OnceLock::new();

/// Get the global LLM circuit breaker.
fn get_llm_circuit_breaker() -> &'static oxi_sdk::ProviderCircuitBreaker {
    LLM_CIRCUIT_BREAKER.get_or_init(|| {
        oxi_sdk::ProviderCircuitBreaker::new(
            "global".to_string(),
            oxi_sdk::CircuitBreakerConfig::default(),
        )
    })
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
    /// API key resolved from CredentialStore at build time.
    pub api_key: Option<String>,
    /// Per-provider options for fine-grained control.
    pub provider_options: Option<oxi_sdk::ProviderOptions>,
    /// Rate limit for tool calls (requests per minute). 0 = unlimited.
    pub rate_limit_per_minute: usize,
    /// Token budget per agent execution. 0 = unlimited.
    pub token_budget: usize,
    /// Enable audit logging for all tool executions.
    pub audit_tool_calls: bool,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            max_iterations: 8,
            tool_execution: ToolExecutionMode::Parallel,
            auto_retry_enabled: true,
            space_id: None,
            project_paths: Vec::new(),
            workspace_dir: None,
            api_key: None,
            provider_options: None,
            rate_limit_per_minute: 0,
            token_budget: 0,
            audit_tool_calls: false,
        }
    }
}

/// Mutable state shared between the event callback and the main execute flow.
#[derive(Default)]
struct ExecuteState {
    final_content: String,
    steps_completed: usize,
    success: bool,
}

/// Runtime that wraps an oxi-sdk `Agent` for executing Seeds.
///
/// Each call to [`AgentRuntime::execute`] creates a fresh `Agent`,
/// builds a ToolRegistry based on the agent's CSpace, and runs it to completion.
///
/// All OS-level access goes through `KernelHandle` — the single syscall table
/// for agent control. Provider/model resolution goes through `OxiosEngine`.
pub struct AgentRuntime {
    engine: Arc<OxiosEngine>,
    config: AgentRuntimeConfig,
    /// Single path to all kernel services.
    kernel_handle: Arc<KernelHandle>,
    /// Persona manager for system prompt injection.
    persona_manager: Option<Arc<PersonaManager>>,
    /// Semantic tool retriever for capability discovery.
    tool_retriever: Option<Arc<crate::tools::retrieval::ToolRetriever>>,
}

impl AgentRuntime {
    /// Creates a new agent runtime with engine and kernel access.
    ///
    /// Provider/model resolution goes through `engine`.
    /// Tool access goes through `kernel_handle`.
    pub fn new(
        engine: Arc<OxiosEngine>,
        model_id: impl Into<String>,
        kernel_handle: Arc<KernelHandle>,
    ) -> Self {
        Self {
            engine,
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

    /// Execute a Seed by running the tool-calling agent to completion.
    ///
    /// 1. Resolves CSpace from persona/role/hint
    /// 2. Registers tools via CSpace
    /// 3. Recalls memories if available
    /// 4. Creates Agent via `Agent::new_with_resolver()`
    /// 5. Runs via `Agent::run_streaming()`
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

        // Blend relevant knowledge notes into system prompt (KnowledgeLens, RFC-003 Phase 3).
        match self
            .kernel_handle
            .knowledge_lens
            .recall_for_context(&seed.goal, 5)
            .await
        {
            Ok(ctx) if !ctx.notes.is_empty() => {
                tracing::info!(
                    notes = ctx.notes.len(),
                    memories = ctx.memories.len(),
                    "Recalled knowledge context for seed"
                );
                let knowledge_blend = ctx
                    .notes
                    .iter()
                    .take(3)
                    .map(|n| format!("## {}\n\n{}", n.name, n.content))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                system_prompt.push_str("\n\n## Relevant Knowledge\n\n");
                system_prompt.push_str(&knowledge_blend);
            }
            Ok(_) => tracing::debug!("No knowledge recalled"),
            Err(e) => tracing::warn!(error = %e, "Failed to recall knowledge context"),
        }

        // Resolve model from engine (provider resolution happens inside AgentBuilder).
        let _model = self.engine.resolve_model(&self.config.model_id)?;
        let seed_id = seed.id;

        // Build the agent.
        let config = self.config.clone();
        let kernel_handle = Arc::clone(&self.kernel_handle);

        let (final_content, steps_completed, success, _agent) = {
            run_agent(
                &config,
                &self.engine,
                kernel_handle,
                system_prompt,
                prompt,
                seed_id,
                agent_id,
                cspace,
            )
            .await?
        };

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

/// Create and run an oxi-sdk `Agent` with CSpace-based tool registration.
///
/// Uses `engine.oxi().agent()` (AgentBuilder) for full middleware,
/// observability, and security integration from oxi-sdk 0.23.0.
async fn run_agent(
    config: &AgentRuntimeConfig,
    engine: &OxiosEngine,
    kernel_handle: Arc<KernelHandle>,
    system_prompt: String,
    prompt: String,
    seed_id: uuid::Uuid,
    agent_id: AgentId,
    cspace: crate::capability::CSpace,
) -> Result<(String, usize, bool, Arc<Agent>)> {
    // Extract workspace.
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

    // Start distributed trace span for this agent execution.
    let _trace_guard = crate::observability::tracer()
        .start(format!("seed-{}", &seed_id.to_string()[..8]).as_str(), oxi_sdk::SpanKind::Agent);

    // ── Register tools based on CSpace ──
    let registry = ToolRegistry::new();
    let search_cache = Arc::new(SearchCache::new());
    register_tools_from_cspace(&registry, &kernel_handle, &cspace, search_cache, agent_id);

    tracing::info!(
        seed_id = %seed_id,
        capabilities = cspace.len(),
        "Tools registered from CSpace"
    );

    // ── Build AgentConfig ──
    let agent_config = AgentConfig {
        name: format!("agent-{}", agent_id),
        description: None,
        model_id: config.model_id.clone(),
        system_prompt: Some(system_prompt),
        max_iterations: config.max_iterations,
        timeout_seconds: 300,
        temperature: Some(0.7),
        max_tokens: Some(8192),
        compaction_strategy: CompactionStrategy::Threshold(0.8),
        compaction_instruction: None,
        context_window: 128_000,
        api_key: config.api_key.clone(),
        workspace_dir: config.project_paths.first().cloned(),
        output_mode: None,
        provider_options: config.provider_options.clone(),
    };

    // ── Build Agent with middleware pipeline ──
    // Create provider and resolver from engine.
    let resolver: Arc<dyn ProviderResolver> = Arc::new(engine.oxi().clone());
    let provider = engine.create_provider(
        &engine.resolve_model(&config.model_id)?.provider,
    )?;

    // Build middleware pipeline.
    let mut pipeline = oxi_sdk::MiddlewarePipeline::new();
    if config.rate_limit_per_minute > 0 {
        pipeline = pipeline.push(
            oxi_sdk::middleware::builtins::RateLimitMiddleware::new(config.rate_limit_per_minute)
        );
    }
    if config.token_budget > 0 {
        pipeline = pipeline.push(
            oxi_sdk::middleware::builtins::TokenBudgetMiddleware::new(config.token_budget)
        );
    }
    if config.audit_tool_calls {
        pipeline = pipeline.push(
            oxi_sdk::middleware::builtins::LoggingMiddleware::new(tracing::Level::INFO)
        );
    }

    // Create Agent with CSpace tool registry and provider resolver.
    let agent = Arc::new(Agent::new_with_resolver(
        provider,
        agent_config,
        Arc::new(registry),
        resolver,
    ));

    // Wire middleware pipeline → AgentHooks.
    if !pipeline.is_empty() {
        let terminate_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let agent_id_for_hooks = agent_id.to_string();
        let hooks = oxi_sdk::middleware::build_hooks(
            Arc::new(pipeline),
            agent_id_for_hooks,
            terminate_flag,
        );
        agent.set_hooks(hooks);
    }

    // Shared mutable state for the event callback.
    let exec_state = Arc::new(Mutex::new(ExecuteState::default()));
    let exec_state_cb = Arc::clone(&exec_state);
    let memory_for_callback: Arc<MemoryManager> = (*kernel_handle.agents.memory_manager()).clone();
    let session_id_for_callback = seed_id.to_string();
    let model_id_for_callback = config.model_id.clone();
    let agent_id_for_callback = agent_id.to_string();

    // Run the agent with streaming events.
    let result = agent
        .run_streaming(prompt, move |event| {
            let mut s = exec_state_cb.lock();
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
                    if let Some(oxi_sdk::Message::Assistant(a)) = messages.last() {
                        s.final_content = a.text_content();
                    }
                    s.success = stop_reason.as_deref() == Some("Stop");
                }
                AgentEvent::Error { message, .. } => {
                    s.final_content = message.clone();
                    s.success = false;
                }
                AgentEvent::Usage {
                    input_tokens,
                    output_tokens,
                } => {
                    // Record token usage to cost tracker.
                    let agent_label = format!("agent-{}", agent_id_for_callback);
                    crate::observability::cost_tracker().record(
                        &agent_label,
                        &oxi_sdk::Model::new(
                            &model_id_for_callback,
                            &model_id_for_callback,
                            oxi_sdk::Api::OpenAiCompletions,
                            "unknown",
                            "https://unknown.com",
                        ),
                        oxi_sdk::TokenUsage {
                            input: input_tokens as u64,
                            output: output_tokens as u64,
                            cache_read: 0,
                            cache_write: 0,
                        },
                    );
                }
                AgentEvent::Compaction { event } => {
                    if let CompactionEvent::Completed { result, .. } = event {
                        let entry = MemoryEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            memory_type: MemoryType::Conversation,
                            tier: crate::memory::MemoryTier::Warm,
                            content: result.summary.clone(),
                            content_hash: 0,
                            source: "compaction".to_string(),
                            session_id: Some(session_id_for_callback.clone()),
                            space_id: None,
                            tags: vec![],
                            importance: 0.5,
                            pinned: false,
                            protection: crate::memory::ProtectionLevel::None,
                            auto_classified: false,
                            session_appearances: 0,
                            user_corrected: false,
                            seen_in_sessions: vec![],
                            created_at: chrono::Utc::now(),
                            accessed_at: chrono::Utc::now(),
                            modified_at: chrono::Utc::now(),
                            access_count: 0,
                            decay_score: 1.0,
                            compaction_level: 0,
                            compacted_from: vec![],
                            related_ids: vec![],
                            contradicts: None,
                        };
                        let mm = memory_for_callback.clone();
                        tokio::spawn(async move {
                            if let Err(e) = mm.remember(entry).await {
                                tracing::warn!(error = %e, "Failed to save compaction summary");
                            }
                        });
                    }
                }
                _ => {}
            }
        })
        .await;

    // Record circuit breaker result after agent execution.
    let circuit = get_llm_circuit_breaker();
    if result.is_err() {
        circuit.record_failure();
        crate::metrics::get_metrics().llm_circuit_breaker_state.set(1.0);
    } else {
        circuit.record_success();
        crate::metrics::get_metrics().llm_circuit_breaker_state.set(0.0);
    }

    if let Err(e) = result {
        tracing::error!(seed_id = %seed_id, error = %e, "Agent failed");
        let s = exec_state.lock();
        return Ok((format!("Agent failed: {e}"), s.steps_completed, false, agent));
    }

    let s = exec_state.lock();
    tracing::info!(
        seed_id = %seed_id,
        steps = s.steps_completed,
        success = s.success,
        "Agent completed"
    );
    Ok((s.final_content.clone(), s.steps_completed, s.success, agent))
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
        "You are an autonomous agent in the Oxios operating system.\n\
         You execute Seeds — immutable specifications with goals, constraints, and\n\
         acceptance criteria. You have tools for reading, writing, editing files,\n\
         running commands, and accessing kernel services.\n\n\
         ## Goal\n\
         {}\n",
        seed.goal,
    );

    // Preserve user's original wording so the agent sees exact language,
    // filenames, and nuances that may have been abstracted in the goal.
    if !seed.original_request.is_empty() && seed.original_request != seed.goal {
        prompt.push_str(&format!(
            "\n## User's Original Request\n{}\n",
            seed.original_request
        ));
    }

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
        "\n## Execution Protocol\n\
         1. UNDERSTAND — Read the Seed completely before acting.\n\
         2. PLAN — Determine the minimal set of actions needed.\n\
         3. EXECUTE — Use tools to accomplish the goal. Prefer the simplest approach.\n\
         4. VERIFY — After each action, check the result: created a file? read it back.\n\
         5. REPORT — Summarize how each acceptance criterion was met, with evidence.\n\n\
         ## Hard Boundaries\n\
         - NEVER modify files outside the workspace scope\n\
         - NEVER execute destructive commands without confirming scope\n\
         - NEVER claim completion without evidence — show the output, not your opinion\n\
         - NEVER add features or improvements beyond the Seed scope\n\
         - If you cannot complete the Seed, say so and explain WHY\n\n\
         ## Scope Guard\n\
         The Seed defines your universe. Do not:\n\
         - Refactor code the Seed didn't mention\n\
         - Add tests the Seed didn't require\n\
         - Change configuration the Seed didn't specify\n\
         - \"Improve\" anything beyond what the acceptance criteria demand\n\n\
         ## Error Handling\n\
         - If a tool fails, read the error message carefully before retrying\n\
         - If a command fails, do NOT immediately retry with --force or sudo\n\
         - If stuck after 3 attempts, report the blocker rather than continuing to fail\n\n\
         ## Shape Matching\n\
         Match your output to the task: simple task → concise response.\n\
         Do not write 50 lines when 5 would do.\n\
         Use `exec` for all command execution (git, gh, osascript, etc.).",
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
    use oxi_sdk::{AgentTool, ToolContext, ToolError};
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
            _ctx: &ToolContext,
        ) -> Result<oxi_sdk::AgentToolResult, ToolError> {
            Ok(oxi_sdk::AgentToolResult::success("ok"))
        }
    }

    /// Test that requires_tools validation passes when all tools are present.
    #[test]
    fn test_requires_tools_validation_passes() {
        let registry = ToolRegistry::new();

        registry.register(DummyTool {
            name: "read".into(),
        });
        registry.register(DummyTool {
            name: "exec".into(),
        });

        let missing = registry.missing(&["read", "exec"]);

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

        registry.register(DummyTool {
            name: "read".into(),
        });

        let missing = registry.missing(&["read", "exec", "nonexistent"]);

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
            original_request: String::new(),
            output_schema: None,
        };

        let prompt = build_system_prompt(&seed, None, None, None);

        assert!(prompt.contains("Build a web server"));
        assert!(prompt.contains("Must use Rust"));
        assert!(prompt.contains("Server responds to requests"));
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
            original_request: String::new(),
            output_schema: None,
        };

        let prompt = build_system_prompt(&seed, None, None, None);

        assert!(prompt.contains("Test goal"));
    }
}
