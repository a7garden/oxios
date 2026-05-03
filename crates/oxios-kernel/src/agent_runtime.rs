//! Agent runtime: wraps oxi-agent's tool-calling loop for use by the kernel.
//!
//! The AgentRuntime creates an oxi-agent session, configures it with built-in
//! tools (read, write, edit, bash, grep, find, ls), and executes a Seed's
//! goal through the LLM tool-calling loop.
//!
//! Note: oxi-agent's Agent is not `Send` (it holds `parking_lot` guards),
//! so we execute it via `tokio::task::spawn_blocking` to stay on one thread.

use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use oxi_agent::{
    Agent, AgentConfig, AgentEvent,
    ReadTool, WriteTool, EditTool, BashTool, GrepTool, FindTool, LsTool,
};
use oxi_ai::Provider;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

use oxios_ouroboros::{ExecutionResult, Seed};

/// A dummy provider that returns an error on any LLM call.
///
/// This is used for testing and development when real LLM access is not available.
/// The AgentRuntime is intended for the execution loop; the Ouroboros engine
/// handles spec generation via its own LLM calls.
#[derive(Clone, Default)]
pub struct DummyProvider;

#[async_trait]
impl Provider for DummyProvider {
    async fn stream(
        &self,
        _model: &oxi_ai::Model,
        _context: &oxi_ai::Context,
        _options: Option<oxi_ai::StreamOptions>,
    ) -> Result<Pin<Box<dyn Stream<Item = oxi_ai::ProviderEvent> + Send>>, oxi_ai::ProviderError> {
        Err(oxi_ai::ProviderError::MissingApiKey)
    }

    fn name(&self) -> &str {
        "dummy"
    }
}

/// Runtime that wraps an oxi-agent session for executing Seeds.
pub struct AgentRuntime {
    #[allow(dead_code)]
    provider: Arc<dyn Provider>,
    model_id: String,
}

impl AgentRuntime {
    /// Creates a new agent runtime with the given LLM provider and model.
    pub fn new(provider: Arc<dyn Provider>, model_id: impl Into<String>) -> Self {
        Self {
            provider,
            model_id: model_id.into(),
        }
    }

    /// Execute a Seed by running the tool-calling loop to completion.
    ///
    /// Creates a fresh `oxi_agent::Agent`, registers built-in tools,
    /// sets the system prompt from the Seed's goal and constraints,
    /// and runs the agent loop.
    pub async fn execute(&self, seed: &Seed) -> Result<ExecutionResult> {
        let system_prompt = build_system_prompt(seed);
        let prompt = build_user_prompt(seed);
        let model_id = self.model_id.clone();

        // Run the agent in a blocking task since Agent is !Send.
        // spawn_blocking keeps execution on a single thread, avoiding
        // the need to move Agent guards across thread boundaries.
        let result = tokio::task::spawn_blocking(move || {
            let config = AgentConfig::new(&model_id)
                .with_name("oxios-worker")
                .with_system_prompt(&system_prompt)
                .with_max_iterations(20)
                .with_timeout(300);

            let agent = Agent::new(Arc::new(DummyProvider), config);
            agent.add_tool(ReadTool::new());
            agent.add_tool(WriteTool::new());
            agent.add_tool(EditTool::new());
            agent.add_tool(BashTool::new());
            agent.add_tool(GrepTool::new());
            agent.add_tool(FindTool::new());
            agent.add_tool(LsTool::new());

            // Channel to collect events from the agent.
            let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);

            // Run the agent and collect events concurrently.
            // We use a simple polling loop instead of select! since
            // the agent future is not Send.
            let agent_result = agent.run_with_channel(prompt, tx);

            // Drain events while the agent runs.
            let mut final_content = String::new();
            let mut steps_completed = 0usize;
            let mut success = false;

            // Poll events in a tight loop.
            while let Some(event) = rx.blocking_recv() {
                match event {
                    AgentEvent::ToolComplete { .. } => {
                        steps_completed += 1;
                    }
                    AgentEvent::Complete { content, stop_reason } => {
                        final_content = content.clone();
                        success = stop_reason == "Stop";
                        break;
                    }
                    AgentEvent::Error { message } => {
                        final_content = message.clone();
                        success = false;
                        break;
                    }
                    _ => {}
                }
            }

            // Await the agent result and drop any error.
            drop(agent_result);

            (final_content, steps_completed, success)
        })
        .await?;

        let (final_content, steps_completed, success) = result;

        tracing::info!(
            seed_id = %seed.id,
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
            .field("model_id", &self.model_id)
            .finish()
    }
}
