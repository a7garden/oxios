//! Agent runtime: wraps oxi-sdk's Agent for Seed execution.
//!
//! The AgentRuntime uses `OxiosEngine.oxi().agent()` (AgentBuilder pattern)
//! to construct agents with full middleware, observability, and security
//! integration from oxi-sdk 0.24.0.
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
//! ## Routing integration (RFC-011)
//!
//! Model usage events (`AgentEvent::Usage`) are recorded to the shared
//! `RoutingStats` so the Web dashboard can display per-model call counts
//! and estimated costs.

use anyhow::Result;
use oxi_sdk::observability::AuditTrail;
use oxi_sdk::{
    Agent, AgentConfig, AgentEvent, CompactionEvent, CompactionStrategy, ProviderResolver,
};
use oxi_sdk::{SearchCache, ToolExecutionMode, ToolRegistry};
use parking_lot::Mutex;
use std::sync::Arc;
// RFC-014 Phase D: `ToolRegistry::register_arc` is used in the AgentBuilder
// path to attach CSpace tools after `builder.build()` returns.

use crate::access_manager::{AccessGate, AgentContext, TracingAuditSink, TrailAuditSink};
use crate::capability::resolve::resolve_cspace;
use crate::engine::OxiosEngine;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType};
use crate::persona::PersonaManager;
use crate::tools::registration::register_tools_from_cspace_gated;

use crate::event_bus::KernelEvent;
use crate::session_context::SessionContext;
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
    /// Provider-level RPM for rate-limited provider pool. 0 = no pooling.
    /// When set, uses `OxiosEngine::pooled_provider()` instead of `create_provider()`.
    pub provider_rpm: u32,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            max_iterations: 8,
            tool_execution: ToolExecutionMode::Parallel,
            auto_retry_enabled: true,
            project_paths: Vec::new(),
            workspace_dir: None,
            api_key: None,
            provider_options: None,
            rate_limit_per_minute: 0,
            token_budget: 0,
            audit_tool_calls: false,
            provider_rpm: 0,
        }
    }
}

/// Mutable state shared between the event callback and the main execute flow.
#[derive(Default)]
struct ExecuteState {
    final_content: String,
    steps_completed: usize,
    success: bool,
    /// Collected trajectory steps for SONA learning (RFC-020 Phase 2).
    /// Ordered by insertion — parallel tools get their final position
    /// resolved when they complete, preserving approximate execution order.
    trajectory_steps: Vec<oxios_memory::memory::sona::TrajectoryStep>,
    /// Map of tool_call_id → (start instant, index into trajectory_steps).
    /// Used to correlate ToolExecutionEnd with the correct step when
    /// parallel tool calls complete out of order.
    pending_tools: std::collections::HashMap<String, (std::time::Instant, usize)>,
}

/// Runtime that wraps an oxi-sdk `Agent` for executing Seeds.
///
/// Each call to [`AgentRuntime::execute`] creates a fresh `Agent`,
/// builds a ToolRegistry based on the agent's CSpace, and runs it to completion.
///
/// All OS-level access goes through `KernelHandle` — the single syscall table
/// for agent control. Provider/model resolution goes through `EngineHandle`,
/// which returns the latest `OxiosEngine` (hot-swapped on config change).
pub struct AgentRuntime {
    engine_handle: Arc<crate::engine::EngineHandle>,
    config: AgentRuntimeConfig,
    /// Single path to all kernel services.
    kernel_handle: Arc<KernelHandle>,
    /// Persona manager for system prompt injection.
    persona_manager: Option<Arc<PersonaManager>>,
    /// Semantic tool retriever for capability discovery.
    tool_retriever: Option<Arc<crate::tools::retrieval::ToolRetriever>>,
    /// Shared routing stats (shared with EngineApi).
    routing_stats: Option<Arc<crate::kernel_handle::RoutingStats>>,
}

impl AgentRuntime {
    /// Creates a new agent runtime with engine handle and kernel access.
    ///
    /// Provider/model resolution goes through `engine_handle` (hot-swapped on config change).
    /// Tool access goes through `kernel_handle`.
    pub fn new(
        engine_handle: Arc<crate::engine::EngineHandle>,
        model_id: impl Into<String>,
        kernel_handle: Arc<KernelHandle>,
        routing_stats: Option<Arc<crate::kernel_handle::RoutingStats>>,
    ) -> Self {
        Self {
            engine_handle,
            config: AgentRuntimeConfig {
                model_id: model_id.into(),
                ..Default::default()
            },
            kernel_handle,
            persona_manager: None,
            tool_retriever: None,
            routing_stats,
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
    pub async fn execute(
        &self,
        agent_id: AgentId,
        seed: &Seed,
        session_ctx: &mut SessionContext,
    ) -> Result<ExecutionResult> {
        // RFC-015: session_id is derived from seed.id for chat transparency
        // event publishing. Most callers run one Seed per session turn, so
        // seed.id is a usable session identifier.
        let session_id: Option<String> = Some(seed.id.to_string());
        self.execute_with_session(agent_id, seed, session_ctx, session_id)
            .await
    }

    /// Like [`execute`](Self::execute) but with an explicit session_id for
    /// RFC-015 chat transparency event publishing.
    pub async fn execute_with_session(
        &self,
        agent_id: AgentId,
        seed: &Seed,
        session_ctx: &mut SessionContext,
        session_id: Option<String>,
    ) -> Result<ExecutionResult> {
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
        match memory_manager
            .recall_with_proactive(&seed.goal, &mut session_ctx.recall_timing)
            .await
        {
            Ok(memories) if !memories.is_empty() => {
                tracing::info!(count = memories.len(), "Recalled memories for seed");
                system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);
            }
            Ok(_) => tracing::debug!("No memories recalled"),
            Err(e) => tracing::warn!(error = %e, "Failed to recall memories"),
        }

        // Inject learned strategy from SONA (RFC-020 Phase 2).
        if let Some(sona) = memory_manager.sona_engine() {
            match sona.adapt(&seed.goal).await {
                Ok(Some(pattern)) if pattern.confidence > 0.5 => {
                    tracing::info!(
                        domain = %pattern.domain,
                        confidence = pattern.confidence,
                        "SONA learned pattern injected"
                    );
                    system_prompt.push_str(&format!(
                        "\n\n## Learned Strategy (confidence: {:.0}%)\n{}\n",
                        pattern.confidence * 100.0,
                        pattern.strategy,
                    ));
                }
                Ok(_) => tracing::debug!("No high-confidence SONA pattern found"),
                Err(e) => tracing::debug!(error = %e, "SONA adapt failed (non-fatal)"),
            }
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
        // Get the latest engine — may have been hot-swapped via Web UI config change.
        let engine = self.engine_handle.get();
        let _model = engine.resolve_model(&self.config.model_id)?;
        let seed_id = seed.id;

        // Build the agent.
        let config = self.config.clone();
        let kernel_handle = Arc::clone(&self.kernel_handle);

        // Extract audit trail from kernel for TrailAuditSink wiring.
        let audit_trail: Option<Arc<AuditTrail>> =
            Some(Arc::clone(&self.kernel_handle.security.audit_trail));

        let (final_content, steps_completed, success, trajectory_steps, _agent) = {
            run_agent(
                &config,
                &engine,
                kernel_handle,
                system_prompt,
                prompt,
                seed_id,
                seed.goal.clone(),
                agent_id,
                cspace,
                audit_trail,
                self.routing_stats.clone(),
                session_id.clone(),
            )
            .await?
        };

        // Map trajectory steps to tool call records for the execution result.
        let tool_calls: Vec<oxios_ouroboros::ToolCallRecord> = trajectory_steps
            .iter()
            .map(|s| oxios_ouroboros::ToolCallRecord {
                tool: s.input.clone(),
                input: String::new(), // Input is summarized in the trajectory step's input field
                output: s.output.clone(),
                duration_ms: s.duration_ms,
            })
            .collect();

        tracing::info!(
            seed_id = %seed_id,
            steps = steps_completed,
            success,
            tool_calls = tool_calls.len(),
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
            tool_calls,
        })
    }
}

/// Create and run an oxi-sdk `Agent` with CSpace-based tool registration.
///
/// Uses `engine.oxi().agent()` (AgentBuilder) for full middleware,
/// observability, and security integration from oxi-sdk 0.23.0.
#[allow(clippy::too_many_arguments)]
async fn run_agent(
    config: &AgentRuntimeConfig,
    engine: &OxiosEngine,
    kernel_handle: Arc<KernelHandle>,
    system_prompt: String,
    prompt: String,
    seed_id: uuid::Uuid,
    seed_goal: String,
    agent_id: AgentId,
    cspace: crate::capability::CSpace,
    audit_trail: Option<Arc<AuditTrail>>,
    routing_stats: Option<Arc<crate::kernel_handle::RoutingStats>>,
    session_id: Option<String>,
) -> Result<(
    String,
    usize,
    bool,
    Vec<oxios_memory::memory::sona::TrajectoryStep>,
    Arc<Agent>,
)> {
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
    let _trace_guard = crate::observability::tracer().start(
        format!("seed-{}", &seed_id.to_string()[..8]).as_str(),
        oxi_sdk::SpanKind::Agent,
    );

    // ── Register tools based on CSpace (with access gate) ──
    let registry = ToolRegistry::new();
    let search_cache = Arc::new(SearchCache::new());

    // Build agent context for security
    let agent_context = AgentContext {
        agent_id,
        agent_name: format!("agent-{agent_id}"),
        cspace: Arc::new(cspace.clone()),
    };

    // Build audit sink: TrailAuditSink (Merkle chain + JSONL) when audit_trail
    // is available, otherwise fall back to TracingAuditSink.
    let audit_sink: Arc<dyn crate::access_manager::AuditSink> = if let Some(trail) = audit_trail {
        let audit_path = kernel_handle
            .state
            .workspace_path()
            .join("audit")
            .join("access.jsonl");
        Arc::new(TrailAuditSink::new(trail, audit_path))
    } else {
        Arc::new(TracingAuditSink)
    };

    // Build access gate from kernel's security infrastructure
    let access_gate = Arc::new(AccessGate::new(
        kernel_handle.exec.access_manager().clone(),
        Arc::new(kernel_handle.exec.config_snapshot()),
        audit_sink,
    ));

    register_tools_from_cspace_gated(
        &registry,
        &kernel_handle,
        &cspace,
        search_cache,
        agent_id,
        access_gate,
        agent_context,
    );

    tracing::info!(
        seed_id = %seed_id,
        capabilities = cspace.len(),
        "Tools registered from CSpace"
    );

    // ── Build AgentConfig ──
    //
    // RFC-014 Phase D: `system_prompt` is also passed to the new
    // `AgentBuilder::system_prompt()` (which overrides the value embedded
    // in `AgentConfig` at build time). We clone here so the builder path
    // can consume the value while the legacy `Agent::new_with_resolver`
    // path still sees it in the config.
    let agent_config = AgentConfig {
        name: format!("agent-{agent_id}"),
        description: None,
        model_id: config.model_id.clone(),
        system_prompt: Some(system_prompt.clone()),
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

    // ── Build Agent (RFC-014 Phase D) ──
    //
    // Two paths:
    //   1. `provider_rpm == 0` (common): use oxi-sdk 0.26.2's new
    //      `AgentBuilder` API. The builder unifies model resolution, provider
    //      creation, and (optionally) middleware wiring. Engine-level
    //      `authorizer` / `tracer` / `cost_tracker` are propagated through
    //      the new builder methods.
    //   2. `provider_rpm > 0` (rare): keep the legacy
    //      `Agent::new_with_resolver` + `set_hooks` path because the
    //      AgentBuilder does not expose a way to inject a pre-built
    //      `ProviderPool` for rate-limited access. This is a deliberate
    //      scope-limit per RFC-014/phase-d-agentbuilder.md §2 "Provider
    //      선택 로직은 보존".
    let agent = if config.provider_rpm > 0 {
        // ── Legacy path: rate-limited provider pool ──
        let resolver: Arc<dyn ProviderResolver> = Arc::new(engine.oxi().clone());
        let provider_name = engine.resolve_model(&config.model_id)?.provider;
        let provider = engine.pooled_provider(&provider_name, config.provider_rpm)?;

        // Build middleware pipeline.
        let mut pipeline = oxi_sdk::MiddlewarePipeline::new();
        if config.rate_limit_per_minute > 0 {
            pipeline = pipeline.push(oxi_sdk::middleware::builtins::RateLimitMiddleware::new(
                config.rate_limit_per_minute,
            ));
        }
        if config.token_budget > 0 {
            pipeline = pipeline.push(oxi_sdk::middleware::builtins::TokenBudgetMiddleware::new(
                config.token_budget,
            ));
        }
        if config.audit_tool_calls {
            pipeline = pipeline.push(oxi_sdk::middleware::builtins::LoggingMiddleware::new(
                tracing::Level::INFO,
            ));
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

        agent
    } else {
        // ── New path: AgentBuilder (RFC-014 Phase D) ──
        let mut builder = engine
            .oxi()
            .agent(agent_config)
            .workspace(&workspace)
            .system_prompt(system_prompt);

        // CSpace-based tool registration is oxios-specific and is preserved.
        //
        // The builder's `.tool()` method takes `impl AgentTool + 'static`
        // (a concrete value), but oxios' CSpace tools are `Arc<dyn AgentTool>`.
        // The SDK does not expose a way to inject a pre-built `ToolRegistry`
        // into the builder, so we register them on the agent's tool registry
        // after `build()` returns. This keeps CSpace semantics intact.
        //
        // We capture the tool names now and apply them once the agent exists.
        let cspace_tool_arcs: Vec<Arc<dyn oxi_sdk::AgentTool>> = registry
            .names()
            .into_iter()
            .filter_map(|name| registry.get(&name))
            .collect();

        // Engine-level observability/security → AgentBuilder (new API).
        if let Some(auth) = engine.authorizer() {
            builder = builder.authorizer(auth.clone());
        }
        if let Some(tracer) = engine.tracer() {
            builder = builder.tracer(tracer.clone());
        }
        if let Some(ct) = engine.cost_tracker() {
            builder = builder.cost_tracker(ct.clone());
        }

        // Middleware: AgentBuilder convenience helpers replace the manual
        // `MiddlewarePipeline` + `build_hooks()` + `set_hooks()` triple.
        if config.rate_limit_per_minute > 0 {
            builder = builder.with_rate_limit(config.rate_limit_per_minute);
        }
        if config.token_budget > 0 {
            builder = builder.with_token_budget(config.token_budget);
        }
        if config.audit_tool_calls {
            builder = builder.with_logging();
        }

        let built = builder.build()?;
        let agent = Arc::new(built);

        // Attach CSpace tools to the agent's tool registry.
        // `Agent::tools()` returns the same `Arc<ToolRegistry>` that
        // `AgentBuilder` populated, so `register_arc` is the canonical
        // extension point for `Arc<dyn AgentTool>` values.
        let agent_tools = agent.tools();
        for tool in cspace_tool_arcs {
            agent_tools.register_arc(tool);
        }

        agent
    };

    // Shared mutable state for the event callback.
    let exec_state = Arc::new(Mutex::new(ExecuteState::default()));
    let exec_state_cb = Arc::clone(&exec_state);
    let memory_for_callback: Arc<MemoryManager> = (*kernel_handle.agents.memory_manager()).clone();
    let session_id_for_callback = seed_id.to_string();
    let model_id_for_callback = config.model_id.clone();
    let agent_id_for_callback = agent_id.to_string();
    let routing_stats_for_cb = routing_stats.clone();
    // RFC-015: real-time event publishing for chat transparency.
    // Falls back to None when the caller did not opt in.
    let transparency_session: Option<String> = session_id.clone();
    let kernel_handle_for_cb: Arc<KernelHandle> = Arc::clone(&kernel_handle);

    // Run the agent with streaming events.
    let result = agent
        .run_streaming(prompt, move |event| {
            let mut s = exec_state_cb.lock();
            match event {
                AgentEvent::ToolExecutionStart {
                    tool_name,
                    tool_call_id,
                    ..
                } => {
                    // Record start time and push a placeholder step.
                    let idx = s.trajectory_steps.len();
                    s.pending_tools
                        .insert(tool_call_id.clone(), (std::time::Instant::now(), idx));
                    s.trajectory_steps
                        .push(oxios_memory::memory::sona::TrajectoryStep {
                            input: tool_name.clone(),
                            output: String::new(),
                            duration_ms: 0,
                            confidence: 0.0,
                        });
                    // RFC-015: broadcast tool start so Web UI can show progress.
                    if let Some(ref sid) = transparency_session {
                        let _ =
                            kernel_handle_for_cb
                                .infra
                                .publish(KernelEvent::ToolExecutionStarted {
                                    session_id: sid.clone(),
                                    tool_name: tool_name.clone(),
                                    tool_call_id: tool_call_id.clone(),
                                    tool_args: serde_json::Value::Null,
                                });
                    }
                }
                AgentEvent::ToolExecutionEnd {
                    tool_name,
                    tool_call_id,
                    is_error,
                    result,
                    ..
                } => {
                    if !is_error {
                        s.steps_completed += 1;
                    }
                    // Look up the exact step by tool_call_id.
                    let mut duration_ms: u64 = 0;
                    let mut summary = String::new();
                    if let Some((start, idx)) = s.pending_tools.remove(tool_call_id.as_str()) {
                        duration_ms = start.elapsed().as_millis() as u64;
                        if let Some(step) = s.trajectory_steps.get_mut(idx) {
                            summary = summarize_tool_result(&result.content, 200);
                            step.output = summary.clone();
                            step.duration_ms = duration_ms;
                            step.confidence = if is_error { 0.3 } else { 0.8 };
                        }
                    }
                    // RFC-015: broadcast tool completion.
                    if let Some(ref sid) = transparency_session {
                        let _ = kernel_handle_for_cb.infra.publish(
                            KernelEvent::ToolExecutionFinished {
                                session_id: sid.clone(),
                                tool_call_id: tool_call_id.clone(),
                                tool_name: tool_name.clone(),
                                duration_ms,
                                is_error,
                                output_summary: summary,
                            },
                        );
                    }
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
                    // Record token usage to cost tracker (existing).
                    let agent_label = format!("agent-{agent_id_for_callback}");
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

                    // Record to routing stats (RFC-011).
                    if let Some(stats) = &routing_stats_for_cb {
                        let cost = crate::kernel_handle::engine_api::estimate_cost(
                            &model_id_for_callback,
                            input_tokens as u64,
                            output_tokens as u64,
                        );
                        stats.record_model_usage(&model_id_for_callback, cost);
                    }
                    // RFC-015: publish cumulative token usage.
                    if let Some(ref sid) = transparency_session {
                        let _ = kernel_handle_for_cb
                            .infra
                            .publish(KernelEvent::TokenUsageUpdate {
                                session_id: sid.clone(),
                                input_tokens: input_tokens as u64,
                                output_tokens: output_tokens as u64,
                            });
                    }
                }
                AgentEvent::Compaction {
                    event: CompactionEvent::Completed { result, .. },
                } => {
                    handle_compaction(
                        result.summary.clone(),
                        session_id_for_callback.clone(),
                        memory_for_callback.clone(),
                    );
                    // RFC-015: compaction is a form of reasoning — expose it.
                    if let Some(ref sid) = transparency_session {
                        let _ =
                            kernel_handle_for_cb
                                .infra
                                .publish(KernelEvent::ReasoningFragment {
                                    session_id: sid.clone(),
                                    content: result.summary.clone(),
                                    source: "compaction".to_string(),
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
        crate::metrics::get_metrics()
            .llm_circuit_breaker_state
            .set(1.0);
    } else {
        circuit.record_success();
        crate::metrics::get_metrics()
            .llm_circuit_breaker_state
            .set(0.0);
    }

    if let Err(e) = result {
        tracing::error!(seed_id = %seed_id, error = %e, "Agent failed");
        let s = exec_state.lock();
        return Ok((
            format!("Agent failed: {e}"),
            s.steps_completed,
            false,
            s.trajectory_steps.clone(),
            agent,
        ));
    }

    let s = exec_state.lock();
    tracing::info!(
        seed_id = %seed_id,
        steps = s.steps_completed,
        success = s.success,
        "Agent completed"
    );

    // Record trajectory to SONA learning engine (RFC-020 Phase 2).
    // Fire-and-forget: don't block the result on learning.
    if !s.trajectory_steps.is_empty() {
        if let Some(sona) = kernel_handle.agents.memory_manager().sona_engine() {
            let steps = s.trajectory_steps.clone();
            let success = s.success;
            let sona = Arc::clone(sona);
            let domain = infer_domain(&seed_goal);
            tokio::spawn(async move {
                let verdict = if success {
                    oxios_memory::memory::sona::Verdict::Success
                } else {
                    oxios_memory::memory::sona::Verdict::Failure
                };
                let trajectory = oxios_memory::memory::sona::Trajectory::new(steps, verdict, &domain);
                if let Err(e) = sona.record(trajectory).await {
                    tracing::debug!(error = %e, "SONA trajectory recording failed (non-fatal)");
                }
            });
        }
    }

    Ok((
        s.final_content.clone(),
        s.steps_completed,
        s.success,
        s.trajectory_steps.clone(),
        agent,
    ))
}

/// Summarize a tool result string to fit within `max_len` characters.
///
/// Uses char-aware truncation to avoid panicking on multi-byte UTF-8
/// (e.g., Korean, CJK, emoji).
fn summarize_tool_result(result: &str, max_len: usize) -> String {
    let trimmed = result.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }
    // Take the first line or truncate.
    let first_line = trimmed.lines().next().unwrap_or("");
    if first_line.chars().count() <= max_len {
        first_line.to_string()
    } else {
        let truncated: String = first_line.chars().take(max_len - 3).collect();
        format!("{truncated}...")
    }
}

/// Infer a domain category from a seed goal for SONA trajectory grouping.
///
/// Extracts the core verb + object from the goal to create a meaningful
/// domain label. Falls back to "general" for unrecognizable patterns.
fn infer_domain(goal: &str) -> String {
    let lower = goal.to_lowercase();
    let keywords: Vec<&str> = lower.split_whitespace().take(8).collect();

    // Check for known domain indicators.
    if keywords.iter().any(|k| {
        [
            "test",
            "tests",
            "spec",
            "testing",
            "assert",
            "unit test",
            "integration",
        ]
        .contains(k)
    }) {
        return "testing".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["deploy", "release", "publish", "ship"].contains(k))
    {
        return "deployment".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["fix", "bug", "patch", "repair", "debug"].contains(k))
    {
        return "bugfix".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["refactor", "restructure", "reorganize", "rewrite"].contains(k))
    {
        return "refactoring".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["doc", "document", "readme", "guide", "explain"].contains(k))
    {
        return "documentation".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["build", "create", "implement", "add", "make", "new"].contains(k))
    {
        return "development".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["analyze", "review", "audit", "inspect", "check"].contains(k))
    {
        return "analysis".to_string();
    }
    if keywords
        .iter()
        .any(|k| ["config", "setup", "install", "configure", "init"].contains(k))
    {
        return "configuration".to_string();
    }

    // Fallback: first 2 meaningful words
    let meaningful: Vec<&str> = lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .take(2)
        .collect();
    if meaningful.len() >= 2 {
        meaningful.join("_")
    } else {
        "general".to_string()
    }
}

/// Handle compaction completion by storing the summary as a Warm memory.
///
/// Extracts the compaction summary from the event and spawns a background
/// task to persist it via MemoryManager. This replaces the inline 30-line
/// block that was previously in the event callback.
fn handle_compaction(summary: String, session_id: String, memory_manager: Arc<MemoryManager>) {
    let entry = MemoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        memory_type: MemoryType::Conversation,
        tier: crate::memory::MemoryTier::Warm,
        content: summary,
        content_hash: 0,
        source: "compaction".to_string(),
        session_id: Some(session_id),
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
    tokio::spawn(async move {
        if let Err(e) = memory_manager.remember(entry).await {
            tracing::warn!(error = %e, "Failed to save compaction summary");
        }
    });
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

    #[test]
    fn test_infer_domain_testing() {
        assert_eq!(infer_domain("run all unit tests for the kernel"), "testing");
    }

    #[test]
    fn test_infer_domain_deployment() {
        assert_eq!(
            infer_domain("deploy the web service to production"),
            "deployment"
        );
    }

    #[test]
    fn test_infer_domain_bugfix() {
        assert_eq!(infer_domain("fix the null pointer error in main"), "bugfix");
    }

    #[test]
    fn test_infer_domain_development() {
        assert_eq!(
            infer_domain("create a new REST API endpoint"),
            "development"
        );
    }

    #[test]
    fn test_infer_domain_analysis() {
        assert_eq!(
            infer_domain("review the code for security issues"),
            "analysis"
        );
    }

    #[test]
    fn test_infer_domain_fallback() {
        let domain = infer_domain("optimize performance metrics");
        // Should fall back to first 2 meaningful words
        assert!(!domain.is_empty());
    }
}
