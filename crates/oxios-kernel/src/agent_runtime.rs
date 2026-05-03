//! Agent runtime: wraps oxi-agent's tool-calling loop for use by the kernel.
//!
//! The AgentRuntime creates an oxi-agent session, configures it with built-in
//! tools (read, write, edit, bash, grep, find, ls), and executes a Seed's
//! goal through the LLM tool-calling loop.

use anyhow::Result;
use oxi_agent::{
    Agent, AgentConfig, AgentEvent,
    ReadTool, WriteTool, EditTool, BashTool, GrepTool, FindTool, LsTool,
};
use oxi_ai::Provider;
use std::sync::Arc;
use tokio::sync::mpsc;

use oxios_ouroboros::{ExecutionResult, Seed};

/// Runtime that wraps an oxi-agent session for executing Seeds.
pub struct AgentRuntime {
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

        let config = AgentConfig::new(&self.model_id)
            .with_name("oxios-worker")
            .with_system_prompt(&system_prompt)
            .with_max_iterations(20)
            .with_timeout(300);

        let agent = Agent::new(Arc::clone(&self.provider), config);
        agent.add_tool(ReadTool::new());
        agent.add_tool(WriteTool::new());
        agent.add_tool(EditTool::new());
        agent.add_tool(BashTool::new());
        agent.add_tool(GrepTool::new());
        agent.add_tool(FindTool::new());
        agent.add_tool(LsTool::new());

        tracing::info!(seed_id = %seed.id, goal = %seed.goal, "AgentRuntime executing seed");

        // Run the agent with an event channel to collect step counts.
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);

        // Run the agent in a separate task. The Agent itself is not Send
        // (due to parking_lot RwLock guards), so we use a scoped approach:
        // run_with_channel is called in a local async block and events are
        // drained from the channel concurrently.
        let provider = Arc::clone(&self.provider);
        let model_id = self.model_id.clone();
        let seed_id = seed.id;

        let result = {
            // We need to run the agent directly since it's not Send.
            // Use a select-like pattern: run the agent and drain events concurrently.
            let agent = Agent::new(provider, AgentConfig::new(&model_id)
                .with_name("oxios-worker")
                .with_system_prompt(&system_prompt)
                .with_max_iterations(20)
                .with_timeout(300));
            agent.add_tool(ReadTool::new());
            agent.add_tool(WriteTool::new());
            agent.add_tool(EditTool::new());
            agent.add_tool(BashTool::new());
            agent.add_tool(GrepTool::new());
            agent.add_tool(FindTool::new());
            agent.add_tool(LsTool::new());

            // Run the agent and collect events in parallel.
            // Since Agent is !Send, we run it on the current task.
            tokio::select! {
                result = agent.run_with_channel(prompt, tx) => {
                    // Drain remaining events.
                    drop(result);
                    let mut steps_completed = 0usize;
                    let mut final_content = String::new();
                    let mut success = false;

                    while let Ok(event) = rx.try_recv() {
                        match &event {
                            AgentEvent::ToolComplete { .. } => {
                                steps_completed += 1;
                            }
                            AgentEvent::Complete { content, stop_reason } => {
                                final_content = content.clone();
                                success = stop_reason == "Stop";
                            }
                            AgentEvent::Error { message } => {
                                final_content = message.clone();
                                success = false;
                            }
                            _ => {}
                        }
                    }

                    (final_content, steps_completed, success)
                }
            }
        };

        let (final_content, steps_completed, success) = result;

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
