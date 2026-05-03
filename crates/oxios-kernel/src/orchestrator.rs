//! Orchestrator: coordinates the full Ouroboros lifecycle for user messages.
//!
//! The orchestrator is the "brain" that runs the Ouroboros protocol
//! end-to-end. Given a user message:
//! 1. Conduct the interview (ask clarifying questions if needed)
//! 2. Generate a seed when ambiguity is low enough
//! 3. Fork and execute an agent via the supervisor
//! 4. Evaluate the result
//! 5. Evolve and re-execute if evaluation fails
//!
//! The orchestrator does NOT know about channels or HTTP — it only
//! coordinates Ouroboros + Supervisor + EventBus + StateStore + Scheduler + AccessManager.

use std::sync::Arc;

use anyhow::{Context, Result};
use oxios_ouroboros::{
    EvaluationResult, InterviewResult, OuroborosProtocol, Phase, Seed,
};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access_manager::AccessManager;
use crate::a2a::{A2AProtocol, AgentCard, TaskPriority};
use crate::event_bus::{EventBus, KernelEvent};
use crate::persona_manager::PersonaManager;
use crate::scheduler::{AgentScheduler, Priority, ScheduledTask};
use crate::state_store::StateStore;
use crate::supervisor::Supervisor;
use crate::types::AgentId;

/// Maximum number of Ouroboros loops before giving up.
const MAX_EVOLUTION_ITERATIONS: usize = 3;

/// The orchestrator coordinates the full Ouroboros lifecycle.
pub struct Orchestrator {
    ouroboros: Arc<dyn OuroborosProtocol>,
    supervisor: Arc<dyn Supervisor>,
    event_bus: EventBus,
    state_store: Arc<StateStore>,
    scheduler: Arc<AgentScheduler>,
    access_manager: Arc<Mutex<AccessManager>>,
    /// Active interview sessions, keyed by session ID.
    sessions: RwLock<std::collections::HashMap<String, InterviewSession>>,
    /// Persona manager for voice/personality customization.
    persona_manager: Arc<PersonaManager>,
    /// A2A protocol for inter-agent communication and delegation.
    a2a_protocol: Arc<A2AProtocol>,
}

impl Orchestrator {
    /// Creates a new orchestrator.
    pub fn new(
        ouroboros: Arc<dyn OuroborosProtocol>,
        supervisor: Arc<dyn Supervisor>,
        event_bus: EventBus,
        state_store: Arc<StateStore>,
        scheduler: Arc<AgentScheduler>,
        access_manager: Arc<Mutex<AccessManager>>,
        persona_manager: Arc<PersonaManager>,
        a2a_protocol: Arc<A2AProtocol>,
    ) -> Self {
        Self {
            ouroboros,
            supervisor,
            event_bus,
            state_store,
            scheduler,
            access_manager,
            sessions: RwLock::new(std::collections::HashMap::new()),
            persona_manager,
            a2a_protocol,
        }
    }

    /// Handle a user message through the full Ouroboros loop.
    ///
    /// Returns an `OrchestrationResult` with the response and metadata.
    ///
    /// If the interview phase needs clarification (ambiguity > 0.2),
    /// the result will contain the questions and the phase will be
    /// `Phase::Interview`. The caller should send these questions to
    /// the user and include the `session_id` in follow-up messages.
    pub async fn handle_message(
        &self,
        user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
    ) -> Result<OrchestrationResult> {
        let session_id = session_id
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        tracing::info!(session_id = %session_id, user_id = %user_id, content_len = user_message.len(), "Orchestrator handling message");

        // Phase 1: Interview
        self.publish_phase_started(&session_id, Phase::Interview).await;

        // Get or create the interview session.
        let needs_interview = {
            let sessions = self.sessions.read();
            !sessions.contains_key(&session_id)
        };

        // Conduct the interview.
        let interview = if needs_interview {
            self.ouroboros.interview(user_message).await?
        } else {
            // This is a follow-up message in an existing interview.
            // Record the user's answer in the session and extract the Q&A context.
            let qa_context = {
                let mut sessions = self.sessions.write();
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.interview.add_exchange("", user_message);
                }
                // Extract Q&A context while holding the write lock, then drop.
                let sessions = self.sessions.read();
                let session = sessions.get(&session_id).expect("session exists");
                session
                    .interview
                    .questions
                    .iter()
                    .zip(session.interview.answers.iter())
                    .map(|(q, a)| format!("Q: {}\nA: {}", q, a))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };

            // Run another interview pass with accumulated context.
            self.ouroboros.interview(&qa_context).await?
        };

        // If ambiguity is too high, return questions for the user to answer.
        if !interview.ready_for_seed {
            // Store the interview so follow-up messages continue it.
            {
                let mut sessions = self.sessions.write();
                sessions.insert(
                    session_id.clone(),
                    InterviewSession {
                        id: session_id.clone(),
                        interview: interview.clone(),
                        phase: Phase::Interview,
                        seed_id: None,
                        agent_id: None,
                    },
                );
            }

            let questions = interview
                .questions
                .iter()
                .filter(|q| !q.is_empty())
                .cloned()
                .collect::<Vec<_>>();

            tracing::info!(
                session_id = %session_id,
                ambiguity = interview.ambiguity.ambiguity(),
                questions = questions.len(),
                "Interview needs clarification"
            );

            self.publish_phase_completed(&session_id, Phase::Interview, "needs clarification").await;

            return Ok(OrchestrationResult {
                session_id: Some(session_id),
                response: format_questions(&questions),
                seed_id: None,
                agent_id: None,
                phase_reached: Phase::Interview,
                evaluation_passed: false,
                output: None,
            });
        }

        // Interview complete and ready. Proceed to seed generation.
        self.publish_phase_completed(&session_id, Phase::Interview, "ready for seed").await;
        self.publish_phase_started(&session_id, Phase::Seed).await;

        // Phase 2: Generate seed
        let seed = self.ouroboros.generate_seed(&interview).await?;

        // Save seed to state store.
        self.save_seed(&seed).await?;

        // Publish seed created event.
        self.event_bus
            .publish(KernelEvent::SeedCreated { seed_id: seed.id })?;

        self.publish_phase_completed(&session_id, Phase::Seed, "generated").await;
        self.publish_phase_started(&session_id, Phase::Execute).await;

        // Phase 3: Fork and execute agent
        let agent_id = self.supervisor.fork(&seed).await?;
        tracing::info!(session_id = %session_id, agent_id = %agent_id, seed_id = %seed.id, "Agent forked");

        // Register agent in A2A registry for inter-agent delegation.
        let agent_name = format!("agent-{}", agent_id);
        let mut card = AgentCard::new(
            agent_id,
            &agent_name,
            &format!("Agent executing seed: {}", seed.goal),
        )
        .with_capability("execute-seed")
        .with_status(crate::types::AgentStatus::Starting);

        // Infer capabilities from seed goal and constraints.
        let goal_lower = seed.goal.to_lowercase();
        if goal_lower.contains("review") || goal_lower.contains("code") {
            card = card.with_capability("code-review");
        }
        if goal_lower.contains("test") {
            card = card.with_capability("testing");
        }
        if goal_lower.contains("refactor") || goal_lower.contains("improve") {
            card = card.with_capability("refactoring");
        }
        if goal_lower.contains("write") || goal_lower.contains("create") || goal_lower.contains("implement") {
            card = card.with_capability("code-generation");
        }
        if goal_lower.contains("debug") || goal_lower.contains("fix") {
            card = card.with_capability("debugging");
        }

        if let Err(e) = self.a2a_protocol.registry().register_agent(card).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to register agent card in A2A registry");
        }

        // Check access for agent tools before running.
        let agent_name = format!("agent-{}", agent_id);
        {
            let mut access = self.access_manager.lock();
            // Log the access check for the agent's primary tools.
            for tool in ["bash", "read", "write", "edit", "grep", "find"] {
                access.can_use_tool(&agent_name, tool);
            }
            // Ensure default permissions exist for the agent.
            if access.get_permissions(&agent_name).is_none() {
                tracing::warn!(agent_id = %agent_id, "Agent has no permissions defined, using default");
                access.get_or_create_permissions(&agent_name);
            }
        }

        // Submit the execution task to the scheduler and dispatch atomically.
        let task = ScheduledTask::for_agent(
            agent_id,
            format!("Execute seed '{}'", seed.goal),
            Priority::Normal,
        );
        let task_id = self.scheduler.submit(task)?;
        // Immediately start the task so no other caller can grab it.
        self.scheduler.start_task(task_id)?;

        let exec_result = self.supervisor.run_with_seed(agent_id, &seed).await?;

        // Unregister agent from A2A registry on completion.
        if let Err(e) = self.a2a_protocol.registry().unregister_agent(agent_id).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to unregister agent card from A2A registry");
        }

        if exec_result.success {
            let _ = self.scheduler.complete_task(task_id);
        } else {
            let _ = self.scheduler.fail_task(task_id, &exec_result.output);
        }

        // Periodically reap zombie tasks.
        self.reap_zombies().await;

        // Phase 4: Evaluate
        self.publish_phase_completed(&session_id, Phase::Execute, "completed").await;
        self.publish_phase_started(&session_id, Phase::Evaluate).await;

        let evaluation = self.ouroboros.evaluate(&seed, &exec_result).await?;

        self.publish_phase_completed(
            &session_id,
            Phase::Evaluate,
            &format!("score={:.2}", evaluation.score),
        ).await;

        self.event_bus.publish(KernelEvent::EvaluationComplete {
            seed_id: seed.id,
            passed: evaluation.all_passed(),
        })?;


        // Save evaluation to state store for lineage tracking.
        self.save_evaluation(&seed, &evaluation).await?;


        // Phase 5: Evolve if needed
        let mut current_seed = Some(seed);
        let mut current_evaluation = evaluation;
        let mut iterations = 0;

        while !current_evaluation.all_passed() && current_evaluation.score < 0.8 && iterations < MAX_EVOLUTION_ITERATIONS {
            iterations += 1;
            self.publish_phase_started(&session_id, Phase::Evolve).await;

            if let Some(evolved) = self.ouroboros.evolve(
                current_seed.as_ref().expect("seed exists"),
                &current_evaluation,
            ).await? {
                current_seed = Some(evolved.clone());

                // Save evolved seed.
                self.save_seed(&evolved).await?;
                self.event_bus.publish(KernelEvent::SeedCreated { seed_id: evolved.id })?;

                self.publish_phase_completed(&session_id, Phase::Evolve, "evolved").await;
                self.publish_phase_started(&session_id, Phase::Execute).await;

                // Re-fork and execute with the evolved seed.
                let new_agent_id = self.supervisor.fork(&evolved).await?;
                let agent_name = format!("agent-{}", new_agent_id);
                {
                    let mut access = self.access_manager.lock();
                    for tool in ["bash", "read", "write", "edit", "grep", "find"] {
                        access.can_use_tool(&agent_name, tool);
                    }
                    if access.get_permissions(&agent_name).is_none() {
                        access.get_or_create_permissions(&agent_name);
                    }
                }

                // Register evolved agent in A2A registry.
                let mut card = AgentCard::new(
                    new_agent_id,
                    &agent_name,
                    &format!("Evolved agent executing seed: {}", evolved.goal),
                )
                .with_capability("execute-seed")
                .with_capability("evolved")
                .with_status(crate::types::AgentStatus::Starting);

                if let Err(e) = self.a2a_protocol.registry().register_agent(card).await {
                    tracing::warn!(agent_id = %new_agent_id, error = %e, "Failed to register evolved agent card");
                }

                let task = ScheduledTask::for_agent(
                    new_agent_id,
                    format!("Execute evolved seed '{}'", evolved.goal),
                    Priority::High,
                );
                let task_id = self.scheduler.submit(task)?;
                self.scheduler.start_task(task_id)?;

                let new_exec = self.supervisor.run_with_seed(new_agent_id, &evolved).await?;

                // Unregister evolved agent on completion.
                if let Err(e) = self.a2a_protocol.registry().unregister_agent(new_agent_id).await {
                    tracing::warn!(agent_id = %new_agent_id, error = %e, "Failed to unregister evolved agent card");
                }

                if new_exec.success {
                    let _ = self.scheduler.complete_task(task_id);
                } else {
                    let _ = self.scheduler.fail_task(task_id, &new_exec.output);
                }

                self.publish_phase_completed(&session_id, Phase::Execute, "completed").await;
                self.publish_phase_started(&session_id, Phase::Evaluate).await;

                let new_eval = self.ouroboros.evaluate(&evolved, &new_exec).await?;
                current_evaluation = new_eval;

                self.publish_phase_completed(
                    &session_id,
                    Phase::Evaluate,
                    &format!("score={:.2}", current_evaluation.score),
                ).await;
                // Save evolved seed evaluation for lineage tracking.
                self.save_evaluation(&evolved, &current_evaluation).await?;
            } else {
                // No evolution possible.
                self.publish_phase_completed(&session_id, Phase::Evolve, "no evolution").await;
                break;
            }
        }

        // Clean up the session.
        {
            let mut sessions = self.sessions.write();
            sessions.remove(&session_id);
        }

        let final_seed = current_seed.expect("at least one seed exists");
        let passed = current_evaluation.all_passed();

        tracing::info!(
            session_id = %session_id,
            iterations,
            score = current_evaluation.score,
            passed,
            "Orchestration complete"
        );

        Ok(OrchestrationResult {
            session_id: Some(session_id),
            response: format_result(&final_seed, &current_evaluation),
            seed_id: Some(final_seed.id),
            agent_id: None,
            phase_reached: Phase::Evaluate,
            evaluation_passed: passed,
            output: Some(current_evaluation.notes.join("; ")),
        })
    }

    /// Save a seed to the state store.
    async fn save_seed(&self, seed: &Seed) -> Result<()> {
        let key = seed.id.to_string();

        self.state_store
            .save_json("seeds", &key, seed)
            .await
            .context("failed to save seed to state store")?;

        Ok(())
    }


    /// Save an evaluation result to the state store.
    async fn save_evaluation(&self, seed: &Seed, evaluation: &EvaluationResult) -> Result<()> {
        let key = format!("{}-eval", seed.id);

        self.state_store
            .save_json("evals", &key, evaluation)
            .await
            .context("failed to save evaluation to state store")?;

        Ok(())
    }

    /// Publish a PhaseStarted event.
    async fn publish_phase_started(&self, session_id: &str, phase: Phase) {
        let _ = self.event_bus.publish(KernelEvent::PhaseStarted {
            session_id: session_id.to_owned(),
            phase,
        });
    }

    /// Publish a PhaseCompleted event.
    async fn publish_phase_completed(&self, session_id: &str, phase: Phase, result: &str) {
        let _ = self.event_bus.publish(KernelEvent::PhaseCompleted {
            session_id: session_id.to_owned(),
            phase,
            result_summary: result.to_owned(),
        });
    }

    /// Reaps zombie tasks and logs the reaped tasks to access audit log.
    async fn reap_zombies(&self) {
        let reaped_ids = self.scheduler.reap_zombies();

        if !reaped_ids.is_empty() {
            tracing::warn!(count = reaped_ids.len(), "Zombie tasks reaped");
            let mut access = self.access_manager.lock();
            for task_id in &reaped_ids {
                access.log_access(
                    "scheduler",
                    "zombie_reap",
                    &task_id.to_string(),
                    true,
                    Some(format!("zombie task {} reaped", task_id)),
                );
            }
        }
    }
}

/// Active session state for multi-turn interviews.
#[derive(Debug, Clone)]
#[allow(unused)]
struct InterviewSession {
    id: String,
    interview: InterviewResult,
    phase: Phase,
    seed_id: Option<Uuid>,
    agent_id: Option<AgentId>,
}

/// Result of a full orchestration cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationResult {
    /// Session ID for multi-turn interviews. Pass this on follow-up messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The response to send back to the user.
    pub response: String,
    /// The seed that was created (if seed phase was reached).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_id: Option<Uuid>,
    /// The agent that executed (if execute phase was reached).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    /// The furthest phase reached.
    pub phase_reached: Phase,
    /// Whether evaluation passed (false if evaluation was skipped or failed).
    pub evaluation_passed: bool,
    /// Output or notes from evaluation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Format clarifying questions for display.
fn format_questions(questions: &[String]) -> String {
    if questions.is_empty() {
        "I need a bit more clarification before I can proceed.".to_string()
    } else {
        format!(
            "I'd like to understand your request better. Could you help clarify:\n\n{}",
            questions
                .iter()
                .enumerate()
                .map(|(i, q)| format!("{}. {}", i + 1, q))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Format the final result for display.
fn format_result(seed: &Seed, evaluation: &EvaluationResult) -> String {
    let passed = evaluation.all_passed();

    let mut lines = Vec::new();
    lines.push(format!(
        "Task '{}' completed.",
        seed.goal
    ));

    if passed {
        lines.push("✅ Evaluation passed.".to_string());
    } else {
        lines.push(format!(
            "⚠️ Evaluation score: {:.0}%",
            evaluation.score * 100.0
        ));
    }

    if !evaluation.notes.is_empty() {
        lines.push("\nNotes:".to_string());
        for note in &evaluation.notes {
            lines.push(format!("- {}", note));
        }
    }

    lines.join("\n")
}
