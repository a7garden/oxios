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
    AgentEvent, AgentLoop, AgentLoopConfig, GrepTool, LsTool, ReadTool, SharedState, ToolExecutionMode, ToolRegistry, WriteTool, EditTool, FindTool,
};
use oxi_ai::{CompactionStrategy, Provider};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::tools::{ContainerExecTool, HostExecTool};
use oxios_ouroboros::{ExecutionResult, Seed};

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
/// runs it to completion, and returns the result.
pub struct AgentRuntime {
    provider: Arc<dyn Provider>,
    config: AgentRuntimeConfig,
}

impl AgentRuntime {
    /// Creates a new agent runtime with the given LLM provider and default config.
    pub fn new(provider: Arc<dyn Provider>, model_id: impl Into<String>) -> Self {
        Self {
            provider,
            config: AgentRuntimeConfig {
                model_id: model_id.into(),
                ..Default::default()
            },
        }
    }

    /// Creates a new agent runtime with the given LLM provider and full config.
    #[allow(dead_code)]
    pub fn with_config(provider: Arc<dyn Provider>, config: AgentRuntimeConfig) -> Self {
        Self { provider, config }
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
        let system_prompt = build_system_prompt(seed);
        let prompt = build_user_prompt(seed);

        // Clone config and provider to move into spawn_blocking.
        let config = self.config.clone();
        let provider = Arc::clone(&self.provider);
        let seed_id = seed.id;

        let (final_content, steps_completed, success) =
            tokio::task::spawn_blocking(move || {
                run_agent_loop(provider, config, system_prompt, prompt, seed_id)
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
fn run_agent_loop(
    provider: Arc<dyn Provider>,
    config: AgentRuntimeConfig,
    system_prompt: String,
    prompt: String,
    seed_id: uuid::Uuid,
) -> Result<(String, usize, bool)> {
    // Build tool registry with Oxios tool composition.
    // Tier 1: oxi native tools (file operations) — no BashTool
    // Tier 2: Oxios execution tools (container_exec, host_exec)
    // Note: Tier 3 (Program tools) will be added in Phase 2
    let registry = ToolRegistry::new();
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());
    registry.register(ContainerExecTool::new(None));
    // TODO: Phase 2 — add HostExecTool and ProgramTool when wiring is complete
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

    // Run the async AgentLoop inside the blocking thread.
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let result = agent_loop
            .run(prompt, move |event| {
                let mut s = exec_state_clone.lock();
                match event {
                    AgentEvent::ToolExecutionEnd { is_error: false, .. } => {
                        s.steps_completed += 1;
                    }
                    AgentEvent::AgentEnd { messages, stop_reason, .. } => {
                        if let Some(msg) = messages.last() {
                            if let oxi_ai::Message::Assistant(a) = msg {
                                s.final_content = a.text_content();
                            }
                        }
                        s.success = stop_reason.as_deref() == Some("Stop");
                    }
                    AgentEvent::Error { message, .. } => {
                        s.final_content = message.clone();
                        s.success = false;
                    }
                    _ => {}
                }
            })
            .await;

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

/// Build a system prompt from the Seed's goal and constraints.
fn build_system_prompt(seed: &Seed) -> String {
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
