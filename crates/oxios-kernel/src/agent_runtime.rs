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
use std::collections::HashMap;
use std::sync::Arc;
// RFC-014 Phase D: `ToolRegistry::register_arc` is used in the AgentBuilder
// path to attach CSpace tools after `builder.build()` returns.

use crate::access_manager::{AccessGate, AgentContext, TracingAuditSink, TrailAuditSink};
use crate::capability::resolve::resolve_cspace;
use crate::engine::OxiosEngine;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType};
use crate::persona::PersonaManager;
use crate::tools::registration::register_tools_from_cspace_gated;

use crate::KernelHandle;
use crate::event_bus::KernelEvent;
use crate::session_context::SessionContext;
use crate::types::AgentId;
use oxios_ouroboros::{Directive, Entity, ExecEnv, ExecutionResult, Seed};

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
    /// Ordered tool_call_ids matching trajectory_steps indices.
    /// Pushed in ToolExecutionStart, same order as trajectory_steps.
    tool_call_ids: Vec<String>,
    /// Per-step tool args (JSON string) captured from ToolExecutionStart.
    tool_args_map: std::collections::HashMap<String, String>,
    /// Per-step error flag from ToolExecutionEnd.
    tool_error_map: std::collections::HashMap<String, bool>,
    /// Per-step start timestamp (UTC) from ToolExecutionStart.
    tool_timestamps: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>,
    /// Cumulative input tokens from AgentEvent::Usage.
    total_input_tokens: u64,
    /// Cumulative output tokens from AgentEvent::Usage.
    total_output_tokens: u64,
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
    /// Autonomous persistence hook (RFC-016).
    persistence_hook: Option<Arc<crate::persistence_hook::PersistenceHook>>,
    /// Per-session assistant message index counter (RFC-016).
    session_msg_counter: Arc<Mutex<HashMap<String, usize>>>,
}

impl AgentRuntime {
    /// Creates a new agent runtime with engine handle and kernel access.
    ///
    /// The active model is resolved live from `engine_handle` on each
    /// `execute()` (reads the post-hot-swap default) — there is no frozen
    /// model id at construction. Tool access goes through `kernel_handle`.
    pub fn new(
        engine_handle: Arc<crate::engine::EngineHandle>,
        kernel_handle: Arc<KernelHandle>,
        routing_stats: Option<Arc<crate::kernel_handle::RoutingStats>>,
    ) -> Self {
        Self {
            engine_handle,
            config: AgentRuntimeConfig::default(),
            kernel_handle,
            persona_manager: None,
            tool_retriever: None,
            routing_stats,
            persistence_hook: None,
            session_msg_counter: Arc::new(Mutex::new(HashMap::new())),
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

    /// Attach a PersistenceHook for autonomous persistence (RFC-016).
    pub fn with_persistence_hook(
        mut self,
        hook: Arc<crate::persistence_hook::PersistenceHook>,
    ) -> Self {
        self.persistence_hook = Some(hook);
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
        self.execute_inner(
            agent_id,
            &seed.goal,
            &seed.original_request,
            &seed.constraints,
            &seed.acceptance_criteria,
            &seed.ontology,
            seed.cspace_hint.as_deref(),
            &seed.mount_paths,
            seed.workspace_context.as_deref(),
            session_ctx,
            session_id,
            Some(seed),
        )
        .await
    }

    /// Execute a Directive with its ExecEnv (RFC-027 unified intent handling).
    ///
    /// Maps Directive/ExecEnv fields to the agent's runtime inputs and runs
    /// the same tool-calling loop as [`execute`](Self::execute). The
    /// persistence hook (RFC-016) is currently skipped on this path because
    /// it still expects a `&Seed`; Phase 6 will update it to accept a
    /// `&Directive`.
    pub async fn execute_directive(
        &self,
        agent_id: AgentId,
        directive: &Directive,
        env: &ExecEnv,
        session_ctx: &mut SessionContext,
    ) -> Result<ExecutionResult> {
        // Directive has no stable per-execution ID yet (Phase 6). Derive a
        // session_id from the agent_id so chat transparency events still
        // correlate.
        let session_id: Option<String> = Some(agent_id.to_string());
        self.execute_directive_with_session(agent_id, directive, env, session_ctx, session_id)
            .await
    }

    /// Like [`execute_directive`](Self::execute_directive) but with an
    /// explicit session_id for RFC-015 chat transparency event publishing.
    pub async fn execute_directive_with_session(
        &self,
        agent_id: AgentId,
        directive: &Directive,
        env: &ExecEnv,
        session_ctx: &mut SessionContext,
        session_id: Option<String>,
    ) -> Result<ExecutionResult> {
        let ontology: &[Entity] = &[];
        self.execute_inner(
            agent_id,
            &directive.goal,
            &directive.original_request,
            &directive.constraints,
            &directive.acceptance_criteria,
            ontology,
            env.cspace_hint.as_deref(),
            &env.mount_paths,
            env.workspace_context.as_deref(),
            session_ctx,
            session_id,
            None,
        )
        .await
    }

    /// Shared execution body for Seed and Directive paths.
    ///
    /// Performs the full agent-runtime pipeline: prompt assembly, capability
    /// retrieval, memory + knowledge recall, CSpace tool registration,
    /// model resolution, agent run, post-execution summary, and (Seed path
    /// only) the autonomous persistence hook. Directive callers pass
    /// `persistence_seed = None` to skip persistence until Phase 6.
    #[allow(clippy::too_many_arguments)]
    async fn execute_inner(
        &self,
        agent_id: AgentId,
        goal: &str,
        original_request: &str,
        constraints: &[String],
        acceptance_criteria: &[String],
        ontology: &[Entity],
        cspace_hint: Option<&str>,
        mount_paths: &[std::path::PathBuf],
        workspace_context: Option<&str>,
        session_ctx: &mut SessionContext,
        session_id: Option<String>,
        persistence_seed: Option<&Seed>,
    ) -> Result<ExecutionResult> {
        let prompt = build_user_prompt_inner(goal, acceptance_criteria);

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

        // Resolve CSpace from persona role, hint, or default.
        let cspace = resolve_cspace(
            cspace_hint,
            persona_role.as_deref(),
            Some("worker"),
            agent_id,
        );

        // Build system prompt (without SKILL.md injection — capabilities are
        // surfaced through the CSpace tool set + semantic retrieval instead).
        let mut system_prompt = build_system_prompt_inner(
            goal,
            original_request,
            constraints,
            acceptance_criteria,
            ontology,
            workspace_context,
            persona_prompt.as_deref(),
            None,
            None,
        );

        // Semantic capability retrieval: find tools relevant to this task's goal.
        let capabilities_xml = if let Some(ref retriever) = self.tool_retriever {
            match retriever.embedder().embed(goal).await {
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
                    tracing::warn!(error = %e, "Failed to embed goal for retrieval");
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
            system_prompt = build_system_prompt_inner(
                goal,
                original_request,
                constraints,
                acceptance_criteria,
                ontology,
                workspace_context,
                persona_prompt.as_deref(),
                capabilities_xml.as_deref(),
                kernel_manifest.as_deref(),
            );
        }

        // Blend relevant memories into system prompt.
        let memory_manager = self.kernel_handle.agents.memory_manager();
        match memory_manager
            .recall_with_proactive(goal, &mut session_ctx.recall_timing)
            .await
        {
            Ok(memories) if !memories.is_empty() => {
                tracing::info!(count = memories.len(), "Recalled memories for task");
                system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);
            }
            Ok(_) => tracing::debug!("No memories recalled"),
            Err(e) => tracing::warn!(error = %e, "Failed to recall memories"),
        }

        // Inject learned strategy from SONA (RFC-020 Phase 2).
        if let Some(sona) = memory_manager.sona_engine() {
            match sona.adapt(goal).await {
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
            .recall_for_context(goal, 5)
            .await
        {
            Ok(ctx) if !ctx.notes.is_empty() => {
                tracing::info!(
                    notes = ctx.notes.len(),
                    memories = ctx.memories.len(),
                    "Recalled knowledge context for task"
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

        // Resolve the LIVE default model (post-hot-swap). This is the single
        // source of truth — the same engine default the OuroborosEngine reads
        // via the ModelResolver port. Validates fail-fast: a bad model ID set
        // via the Web UI is rejected here at execute entry, before any tool work.
        let engine = self.engine_handle.get();
        let model_id = engine.default_model_id().to_string();
        engine.resolve_model(&model_id)?;
        // Synthetic per-execution ID for tracing. Seed path uses seed.id;
        // Directive path mints a fresh UUID since Directive doesn't carry one.
        let exec_id = persistence_seed
            .map(|s| s.id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        // Build the agent. Refresh config.model_id to the live value so every
        // downstream consumer (AgentConfig, legacy provider path, usage callback)
        // uses the same model as the interview/seed phases — no frozen boot
        // string that silently diverges from what interview used.
        let mut config = self.config.clone();
        config.model_id = model_id;
        let kernel_handle = Arc::clone(&self.kernel_handle);

        // Extract audit trail from kernel for TrailAuditSink wiring.
        let audit_trail: Option<Arc<AuditTrail>> =
            Some(Arc::clone(&self.kernel_handle.security.audit_trail));

        let (
            mut final_content,
            steps_completed,
            success,
            trajectory_steps,
            agent,
            tool_call_ids,
            tool_args_map,
            tool_error_map,
            tool_timestamps,
            total_input_tokens,
            total_output_tokens,
        ) = {
            run_agent(
                &config,
                &engine,
                kernel_handle,
                system_prompt,
                prompt,
                exec_id,
                goal.to_string(),
                agent_id,
                cspace,
                audit_trail,
                self.routing_stats.clone(),
                session_id.clone(),
                mount_paths,
            )
            .await?
        };

        // ── Post-execution: safety net for empty final content ──
        //
        // oxi 0.32.0 removed max_iterations — the loop now exits naturally
        // when the LLM produces a text-only response (pi-agent behavior).
        // This block is kept as a safety net in case the LLM returns empty
        // text despite a natural exit (rare, but possible).
        if final_content.is_empty() && !trajectory_steps.is_empty() {
            let tool_summary: Vec<String> = trajectory_steps
                .iter()
                .enumerate()
                .map(|(i, step)| {
                    let truncated = if step.output.len() > 800 {
                        // Char-boundary safe truncation: roll back to the
                        // nearest UTF-8 boundary so multibyte sequences
                        // (Korean, CJK, emoji) don't panic on byte slicing.
                        let mut end = 800;
                        while end > 0 && !step.output.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}...", &step.output[..end])
                    } else {
                        step.output.clone()
                    };
                    format!("{}. [{}] {}", i + 1, step.input, truncated)
                })
                .collect();
            let summary_prompt = format!(
                "도구 실행 결과:\n\n{}\n\n\
                 위 결과를 바탕으로 사용자의 요청에 대해 자연스럽게 한국어로 답변해주세요. \
                 도구의 원시 출력을 그대로 복사하지 말고, 의미 있는 내용만 정리해서 전달하세요.",
                tool_summary.join("\n")
            );
            match agent.run(summary_prompt).await {
                Ok((response, _events)) => {
                    if !response.content.is_empty() {
                        tracing::info!(exec_id = %exec_id, "Post-execution summary generated");
                        final_content = response.content;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Post-execution summary failed");
                }
            }
        }

        // Map trajectory steps to tool call records for the execution result.
        // tool_call_ids[i] corresponds to trajectory_steps[i].
        let tool_calls: Vec<oxios_ouroboros::ToolCallRecord> = trajectory_steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let tc_id = tool_call_ids.get(i).cloned().unwrap_or_default();
                let args_str = tool_call_ids
                    .get(i)
                    .and_then(|id| tool_args_map.get(id))
                    .cloned()
                    .unwrap_or_default();
                let is_error = tool_call_ids
                    .get(i)
                    .and_then(|id| tool_error_map.get(id))
                    .copied()
                    .unwrap_or(false);
                let timestamp = tool_call_ids
                    .get(i)
                    .and_then(|id| tool_timestamps.get(id))
                    .copied();
                let input_str = truncate_json_str(&args_str, 500);
                oxios_ouroboros::ToolCallRecord {
                    tool: step.input.clone(),
                    input: input_str,
                    output: step.output.clone(),
                    duration_ms: step.duration_ms,
                    is_error,
                    tool_call_id: tc_id,
                    timestamp,
                }
            })
            .collect();

        tracing::info!(
            exec_id = %exec_id,
            steps = steps_completed,
            success,
            tool_calls = tool_calls.len(),
            "AgentRuntime finished"
        );

        let result = ExecutionResult {
            output: final_content.clone(),
            steps_completed,
            success,
            tool_calls,
            tokens_input: total_input_tokens,
            tokens_output: total_output_tokens,
            model_id: self.engine_handle.get().default_model_id().to_string(),
        };

        // RFC-016: Autonomous persistence hook.
        // Runs after successful execution, fire-and-forget.
        // Only available on the Seed path today (persistence_seed is Some);
        // the Directive path will gain its own hook adapter in Phase 6.
        if let Some(seed) = persistence_seed
            && success && let Some(hook) = &self.persistence_hook
        {
            let already_saved_knowledge = trajectory_steps
                .iter()
                .any(|s| s.input == "knowledge" && s.output.contains("written successfully"));
            let hook = hook.clone();
            let seed_clone = seed.clone();
            let traj_clone = trajectory_steps.clone();
            let output_clone = final_content.clone();
            let sid = session_id.clone();
            // Compute the assistant message index for this execution.
            // Increment per-session counter, then use the pre-increment value.
            let msg_index = {
                let mut counter = self.session_msg_counter.lock();
                let idx = counter.entry(sid.clone().unwrap_or_default()).or_insert(0);
                let current = *idx;
                *idx += 1;
                current
            };
            tokio::spawn(async move {
                match hook
                    .evaluate(
                        &seed_clone,
                        &traj_clone,
                        &output_clone,
                        already_saved_knowledge,
                    )
                    .await
                {
                    Ok(plan) => {
                        if !plan.memory.is_empty() || !plan.knowledge.is_empty() {
                            tracing::info!(
                                memory = plan.memory.len(),
                                knowledge = plan.knowledge.len(),
                                message_index = msg_index,
                                "PersistenceHook executing plan"
                            );
                            let session_id = sid.unwrap_or_default();
                            hook.execute_plan(plan, &session_id, msg_index).await;
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "PersistenceHook evaluate failed"),
                }
            });
        }

        Ok(result)
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
    mount_paths: &[std::path::PathBuf],
) -> Result<(
    String,
    usize,
    bool,
    Vec<oxios_memory::memory::sona::TrajectoryStep>,
    Arc<Agent>,
    Vec<String>,
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, bool>,
    std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>,
    u64,
    u64,
)> {
    // Extract workspace.
    // RFC-025: prefer the primary Mount's first path, then fall back to the
    // legacy config.project_paths, then workspace_dir, then temp.
    let workspace = if !mount_paths.is_empty() {
        mount_paths[0].clone()
    } else if !config.project_paths.is_empty() {
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

    // Ensure all paths the agent might access are in allowed_paths.
    //
    // AgentLifecycleManager::ensure_permissions() adds kernel.workspace (~/.oxios/workspace),
    // but the agent operates in different directories depending on context:
    //
    //   1. Process CWD — oxi-sdk 0.35+ bakes `workspace_dir` into file tools
    //      via `with_cwd`, so ReadTool/LsTool resolve relatives against the
    //      workspace, NOT the process CWD. However, oxios's own CSpace tools
    //      (kernel-bridge tools wrapped in GatedTool) and bash/exec
    //      subprocesses may still resolve against the process CWD. We grant
    //      it as a safety net so those tools aren't denied by GatedTool.
    //   2. The designated workspace — computed from mount_paths / workspace_dir / temp.
    //   3. Kernel workspace — state store path for seeds, sessions, etc.
    //   4. /tmp -- general temp file access.
    //
    // All four must be in allowed_paths before GatedTool wraps any tool.
    {
        use crate::access_manager::{Role, Subject};
        let agent_name = format!("agent-{agent_id}");
        let mut am = kernel_handle.exec.access_manager().lock();
        let perms = am.get_or_create_permissions(&agent_name);

        // 1. CWD -- critical: oxi-sdk resolves relative paths here
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_pattern = format!("{}/**", cwd.to_string_lossy().trim_end_matches('/'));
            if !perms.allowed_paths.iter().any(|p| p == &cwd_pattern) {
                perms.allow_path(&cwd_pattern);
                tracing::debug!(
                    agent = %agent_name,
                    path = %cwd_pattern,
                    "Added CWD to agent allowed paths"
                );
            }
        }

        // 2. Designated workspace
        let ws_pattern = format!("{}/**", workspace.to_string_lossy().trim_end_matches('/'));
        if !perms.allowed_paths.iter().any(|p| p == &ws_pattern) {
            perms.allow_path(&ws_pattern);
        }

        // 2b. RFC-025: every bound Mount grants path access.
        //     This fixes the latent gap where only project_paths[0] was
        //     accessible — now all Mount paths (multi-path work) are allowed.
        //     Parent patterns already covering a path are skipped.
        for mount_path in mount_paths {
            let pattern = format!("{}/**", mount_path.to_string_lossy().trim_end_matches('/'));
            if !perms.allowed_paths.iter().any(|p| p == &pattern) {
                perms.allow_path(&pattern);
                tracing::debug!(
                    agent = %agent_name,
                    path = %pattern,
                    "Added Mount path to agent allowed paths (RFC-025)"
                );
            }
        }

        // 3. Kernel workspace (state store path)
        let kernel_ws = kernel_handle
            .state
            .workspace_path()
            .to_string_lossy()
            .to_string();
        let kernel_ws_pattern = format!("{}/**", kernel_ws.trim_end_matches('/'));
        if kernel_ws_pattern != ws_pattern
            && !perms.allowed_paths.iter().any(|p| p == &kernel_ws_pattern)
        {
            perms.allow_path(&kernel_ws_pattern);
        }

        // 4. /tmp -- for general temp file access
        if !perms.allowed_paths.iter().any(|p| p == "/tmp/**") {
            perms.allow_path("/tmp/**");
        }

        // Ensure RBAC Superuser role so AccessGate Layer 1 passes.
        let rbac_subject = Subject::Agent(agent_id);
        am.rbac_manager_mut()
            .assign_role(rbac_subject, Role::Superuser);
    }

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
        timeout_seconds: 300,
        temperature: Some(0.7),
        max_tokens: Some(8192),
        compaction_strategy: CompactionStrategy::Threshold(0.8),
        compaction_instruction: None,
        context_window: 128_000,
        api_key: config.api_key.clone(),
        workspace_dir: Some(workspace.clone()),
        output_mode: None,
        provider_options: config.provider_options.clone(),
        // oxi-sdk 0.37.0+: ownership identity for oxi's built-in ownership-gated
        // tools (e.g. the `issue` tool's flock). `None` preserves the pre-0.37.1
        // behavior (ToolContext.session_id == None). Oxios runs its own tool
        // set, so no ownership identity is needed here; set `Some(...)` only if
        // oxios agents start using oxi ownership-gated tools.
        session_id: None,
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
                    args,
                    context,
                    ..
                } => {
                    // Record start time and push a placeholder step.
                    let idx = s.trajectory_steps.len();
                    s.pending_tools
                        .insert(tool_call_id.clone(), (std::time::Instant::now(), idx));
                    s.tool_args_map.insert(
                        tool_call_id.clone(),
                        serde_json::to_string(&args).unwrap_or_default(),
                    );
                    s.tool_timestamps
                        .insert(tool_call_id.clone(), chrono::Utc::now());
                    s.tool_call_ids.push(tool_call_id.clone());
                    s.trajectory_steps
                        .push(oxios_memory::memory::sona::TrajectoryStep {
                            input: tool_name.clone(),
                            output: String::new(),
                            duration_ms: 0,
                            confidence: 0.0,
                        });
                    // RFC-015: broadcast tool start so Web UI can show progress.
                    if let Some(ref sid) = transparency_session {
                        let context_json = context
                            .as_ref()
                            .map(serde_json::to_value)
                            .transpose()
                            .unwrap_or(None);
                        let _ =
                            kernel_handle_for_cb
                                .infra
                                .publish(KernelEvent::ToolExecutionStarted {
                                    session_id: sid.clone(),
                                    tool_name: tool_name.clone(),
                                    tool_call_id: tool_call_id.clone(),
                                    tool_args: args.clone(),
                                    context: context_json,
                                });
                    }
                }
                AgentEvent::ToolExecutionUpdate {
                    tool_call_id,
                    tool_name,
                    partial_result,
                    tab_id,
                    context,
                } => {
                    // RFC-015: forward real-time progress to the event bus
                    // so the Web UI can show a spinner and progress text
                    // while the tool is still executing. Best-effort —
                    // publish failures (e.g. lagged subscribers) are ignored.
                    //
                    // `tab_id` and `context` come from oxi-agent 0.29+
                    // (ToolCallContext: PageVisit, WebSearch, etc.).
                    // Older agent versions won't send these — they default
                    // to None and the UI gracefully ignores them.
                    if let Some(ref sid) = transparency_session {
                        let context_json = context
                            .as_ref()
                            .map(serde_json::to_value)
                            .transpose()
                            .unwrap_or(None);
                        let _ = kernel_handle_for_cb.infra.publish(
                            KernelEvent::ToolExecutionProgress {
                                session_id: sid.clone(),
                                tool_call_id: tool_call_id.clone(),
                                tool_name: tool_name.clone(),
                                progress: partial_result,
                                tab_id,
                                context: context_json,
                            },
                        );
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
                    s.tool_error_map.insert(tool_call_id.clone(), is_error);
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
                    // oxi 0.32.0: loop exits naturally when LLM produces text-only
                    // response (StopReason::Stop). Error/Aborted = failure.
                    // ToolUse should not occur at AgentEnd in 0.32.0 (the loop
                    // continues until text-only), but treat it as non-failure
                    // since tool calls were executed successfully.
                    s.success = matches!(stop_reason.as_deref(), Some("Stop") | Some("ToolUse"));
                }
                AgentEvent::Error { message, .. } => {
                    s.final_content = message.clone();
                    s.success = false;
                }
                AgentEvent::Usage {
                    input_tokens,
                    output_tokens,
                } => {
                    // Accumulate totals for ExecutionResult.
                    s.total_input_tokens += input_tokens as u64;
                    s.total_output_tokens += output_tokens as u64;

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
            s.tool_call_ids.clone(),
            s.tool_args_map.clone(),
            s.tool_error_map.clone(),
            s.tool_timestamps.clone(),
            s.total_input_tokens,
            s.total_output_tokens,
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
    if !s.trajectory_steps.is_empty()
        && let Some(sona) = kernel_handle.agents.memory_manager().sona_engine()
    {
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

    Ok((
        s.final_content.clone(),
        s.steps_completed,
        s.success,
        s.trajectory_steps.clone(),
        agent,
        s.tool_call_ids.clone(),
        s.tool_args_map.clone(),
        s.tool_error_map.clone(),
        s.tool_timestamps.clone(),
        s.total_input_tokens,
        s.total_output_tokens,
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
        let take = max_len.saturating_sub(3);
        let truncated: String = if take == 0 {
            first_line.chars().take(max_len).collect()
        } else {
            first_line.chars().take(take).collect()
        };
        format!("{truncated}...")
    }
}
fn truncate_json_str(json_str: &str, max_len: usize) -> String {
    if json_str.len() <= max_len {
        return json_str.to_string();
    }
    // Saturating sub avoids underflow panic when max_len < 3; if there
    // isn't room for an ellipsis, return as many chars as fit.
    let take = max_len.saturating_sub(3);
    if take == 0 {
        return json_str.chars().take(max_len).collect();
    }
    let truncated: String = json_str.chars().take(take).collect();
    format!("{truncated}...")
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
#[allow(dead_code)]
fn build_system_prompt(
    seed: &Seed,
    persona_prompt: Option<&str>,
    capabilities_xml: Option<&str>,
    kernel_manifest: Option<&str>,
    workspace_context: Option<&str>,
) -> String {
    build_system_prompt_inner(
        &seed.goal,
        &seed.original_request,
        &seed.constraints,
        &seed.acceptance_criteria,
        &seed.ontology,
        workspace_context,
        persona_prompt,
        capabilities_xml,
        kernel_manifest,
    )
}

/// Build a system prompt from a Directive and ExecEnv (RFC-027).
///
/// Maps [`Directive`] fields (`goal`, `original_request`, `constraints`,
/// `acceptance_criteria`) and [`ExecEnv`] fields (`workspace_context`) into
#[allow(dead_code)]
fn build_directive_system_prompt(
    directive: &Directive,
    env: &ExecEnv,
    persona_prompt: Option<&str>,
    capabilities_xml: Option<&str>,
    kernel_manifest: Option<&str>,
) -> String {
    let ontology: &[Entity] = &[];
    build_system_prompt_inner(
        &directive.goal,
        &directive.original_request,
        &directive.constraints,
        &directive.acceptance_criteria,
        ontology,
        env.workspace_context.as_deref(),
        persona_prompt,
        capabilities_xml,
        kernel_manifest,
    )
}

/// Shared system-prompt builder for Seed and Directive paths.
///
/// Composes the static agent prelude, goal/constraints/criteria sections,
/// optional workspace context and ontology, persona, capability index, and
/// kernel manifest into a single prompt string. The ontology section is
/// Seed-only; Directive callers pass an empty slice.
#[allow(clippy::too_many_arguments)]
fn build_system_prompt_inner(
    goal: &str,
    original_request: &str,
    constraints: &[String],
    acceptance_criteria: &[String],
    ontology: &[Entity],
    workspace_context: Option<&str>,
    persona_prompt: Option<&str>,
    capabilities_xml: Option<&str>,
    kernel_manifest: Option<&str>,
) -> String {
    let mut prompt = String::from(
        "You are an autonomous agent in the Oxios operating system.\n\
         You execute Seeds — immutable specifications with goals, constraints, and\n\
         acceptance criteria.\n\n\
         ## Available Tools\n\
         You have the following tools:\n\
         - **File tools**: read, write, edit files; grep, find, ls for searching\n\
         - **Web tools**: web_search for searching the web, get_search_results for retrieving cached results\n\
         - **Exec**: run shell commands\n\
         - **Memory tools**: memory_read, memory_write, memory_search — agent's internal recall\n\
         - **Knowledge**: knowledge — personal markdown vault for documents and notes\n\
         - **Kernel tools**: agent, project, persona, cron, security, budget, resource\n\n\
         **Important**: When the task involves fetching information from the internet,\n\
         websites, or online services, use `web_search` first — do NOT search local files.\n\
         When the task asks to \"get\", \"fetch\", \"find online\", or \"look up\" something\n\
         from the web, use `web_search`.\n",
    );
    prompt.push_str(&format!("\n## Goal\n{}\n", goal));

    // Preserve user's original wording so the agent sees exact language,
    // filenames, and nuances that may have been abstracted in the goal.
    if !original_request.is_empty() && original_request != goal {
        prompt.push_str(&format!(
            "\n## User's Original Request\n{}\n",
            original_request
        ));
    }

    if !constraints.is_empty() {
        prompt.push_str("\n## Constraints\n");
        for (i, c) in constraints.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, c));
        }
    }

    if !acceptance_criteria.is_empty() {
        prompt.push_str("\n## Acceptance Criteria\n");
        for (i, c) in acceptance_criteria.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, c));
        }
    }

    // ── Workspace Context (RFC-025) ──
    // Inject active Mounts + project instructions AFTER the goal/constraints
    // and BEFORE the persona, so the agent sees its workspace before it acts.
    if let Some(ctx) = workspace_context.filter(|s| !s.trim().is_empty()) {
        prompt.push_str("\n## Workspace Context\n");
        prompt.push_str(ctx);
        prompt.push('\n');
    }

    if !ontology.is_empty() {
        prompt.push_str("\n## Domain Entities\n");
        for e in ontology {
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
#[allow(dead_code)]
fn build_user_prompt(seed: &Seed) -> String {
    build_user_prompt_inner(&seed.goal, &seed.acceptance_criteria)
}
#[allow(dead_code)]
fn build_directive_user_prompt(directive: &Directive) -> String {
    build_user_prompt_inner(&directive.goal, &directive.acceptance_criteria)
}

/// Shared user-prompt builder for Seed and Directive paths.
fn build_user_prompt_inner(goal: &str, acceptance_criteria: &[String]) -> String {
    format!(
        "Execute the following goal:\n\n{}\n\nAcceptance criteria:\n{}",
        goal,
        acceptance_criteria
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
            .field("model_id", &self.engine_handle.get().default_model_id())
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
            project_id: None,
            workspace_context: None,
            mount_paths: Vec::new(),
        };

        let prompt = build_system_prompt(&seed, None, None, None, None);

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
            project_id: None,
            workspace_context: None,
            mount_paths: Vec::new(),
        };

        let prompt = build_system_prompt(&seed, None, None, None, None);

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
