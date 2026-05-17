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
use chrono;
use oxios_ouroboros::{EvaluationResult, InterviewResult, OuroborosProtocol, Phase, Seed};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_lifecycle::AgentLifecycleManager;
use crate::event_bus::{EventBus, KernelEvent};
use crate::git_layer::GitLayer;
use crate::metrics::get_metrics;
use crate::scheduler::Priority;
use crate::space::{ConversationBuffer, SpaceId, SpaceManager};
use crate::state_store::StateStore;
use crate::types::AgentId;

/// Role of an agent within a group.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum AgentRole {
    /// Executes a specific subtask.
    #[default]
    Worker,
    /// Coordinates subtasks, synthesizes results.
    Manager,
}

/// A subtask within a multi-agent plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    /// Unique subtask ID.
    pub id: Uuid,
    /// Human-readable description.
    pub description: String,
    /// Capability required (e.g., "code-review", "testing").
    pub required_capability: Option<String>,
    /// Result of the subtask (filled after execution).
    pub result: Option<String>,
    /// Whether this subtask succeeded.
    pub success: bool,
    /// Role of the agent assigned to this subtask.
    #[serde(default)]
    pub role: AgentRole,
}

impl SubTask {
    /// Create a new subtask with the given description.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
            required_capability: None,
            result: None,
            success: false,
            role: AgentRole::default(),
        }
    }

    /// Set the required capability for this subtask.
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.required_capability = Some(cap.into());
        self
    }
}

/// Maximum number of Ouroboros loops before giving up.
const MAX_EVOLUTION_ITERATIONS: usize = 3;

/// The orchestrator coordinates the full Ouroboros lifecycle.
pub struct Orchestrator {
    ouroboros: Arc<dyn OuroborosProtocol>,
    event_bus: EventBus,
    state_store: Arc<StateStore>,
    /// Git version control layer for auto-commits.
    git_layer: Option<Arc<GitLayer>>,
    /// Active interview sessions, keyed by session ID.
    sessions: RwLock<std::collections::HashMap<String, InterviewSession>>,
    /// Agent lifecycle manager (fork, register, run, cleanup).
    lifecycle: AgentLifecycleManager,
    /// A2A protocol for inter-agent task delegation.
    a2a: Option<Arc<crate::a2a::A2AProtocol>>,
    /// Space manager for context partitioning.
    space_manager: RwLock<Option<Arc<SpaceManager>>>,
    /// Conversation buffer for topic shift detection.
    conversation_buffer: RwLock<ConversationBuffer>,
}

impl Orchestrator {
    /// Creates a new orchestrator.
    pub fn new(
        ouroboros: Arc<dyn OuroborosProtocol>,
        event_bus: EventBus,
        state_store: Arc<StateStore>,
        lifecycle: AgentLifecycleManager,
    ) -> Self {
        Self {
            ouroboros,
            event_bus,
            state_store,
            git_layer: None,
            sessions: RwLock::new(std::collections::HashMap::new()),
            lifecycle,
            a2a: None,
            space_manager: RwLock::new(None),
            conversation_buffer: RwLock::new(ConversationBuffer::default()),
        }
    }

    /// Set the SpaceManager for context partitioning.
    pub fn set_space_manager(&self, manager: Arc<SpaceManager>) {
        *self.space_manager.write() = Some(manager);
    }

    /// Get the current Space ID, if SpaceManager is set.
    pub fn current_space_id(&self) -> Option<SpaceId> {
        self.space_manager
            .read()
            .as_ref()
            .map(|m| m.current_space_id())
    }

    /// Get the current Space name tag for response decoration.
    pub fn current_space_tag(&self) -> String {
        self.space_manager
            .read()
            .as_ref()
            .and_then(|m| {
                m.current_space().map(|s| {
                    if s.is_default() {
                        String::new()
                    } else {
                        format!("[{} {}]", s.emoji(), s.name)
                    }
                })
            })
            .unwrap_or_default()
    }

    /// Set the A2A protocol for inter-agent task delegation.
    pub fn set_a2a(&mut self, a2a: Arc<crate::a2a::A2AProtocol>) {
        self.a2a = Some(a2a);
    }

    /// Set the GitLayer for auto-commits after state saves.
    pub fn set_git_layer(&mut self, git_layer: Arc<GitLayer>) {
        self.git_layer = Some(git_layer);
    }

    /// Commit a file to git if GitLayer is configured and enabled.
    fn git_commit(&self, rel_path: &str, message: &str) {
        if let Some(ref gl) = self.git_layer {
            if gl.is_enabled() {
                let _ = gl.commit_file(rel_path, message);
            }
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
    #[allow(clippy::await_holding_lock)]
    pub async fn handle_message(
        &self,
        user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
    ) -> Result<OrchestrationResult> {
        tracing::info!(name = "orchestrator.handle_message", session_id = %session_id.unwrap_or("new"), "starting");
        get_metrics().messages.inc();
        let orch_start = std::time::Instant::now();

        let session_id = session_id
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        tracing::info!(session_id = %session_id, user_id = %user_id, content_len = user_message.len(), "Orchestrator handling message");

        // ── Space Detection ──
        let space_tag = self.current_space_tag();
        let space_id = {
            let buffer = self.conversation_buffer.read();
            let space_manager = self.space_manager.read();
            if let Some(ref sm) = *space_manager {
                match sm.detect_or_create(user_message, &buffer).await {
                    Ok(id) => {
                        tracing::info!(space_id = %id, "Space detected/created for message");
                        id
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Space detection failed, using default");
                        sm.default_space_id()
                    }
                }
            } else {
                // No SpaceManager set — use zero ID
                uuid::Uuid::nil()
            }
        };

        // Record user message in conversation buffer
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_user(user_message);
        }

        // ── Chat Bypass ──
        // Simple conversational messages (greetings, small talk, questions
        // that don't need tool execution) should get a direct LLM response
        // instead of entering the full Ouroboros pipeline.
        let is_chat = Self::is_chat_message(user_message);

        if is_chat {
            tracing::info!(session_id = %session_id, "Chat bypass: responding directly");
            self.publish_phase_started(&session_id, Phase::Interview).await;

            // Use the ouroboros engine's LLM to generate a direct response.
            let response = self.lifecycle.chat_response(user_message).await;

            let response_text = match response {
                Ok(text) => text,
                Err(e) => {
                    tracing::warn!(error = %e, "Chat response failed, using fallback");
                    format!("죄송합니다, 응답을 생성하는 중 오류가 발생했습니다. 다시 시도해주세요.")
                }
            };

            // Record agent response in conversation buffer
            {
                let mut buffer = self.conversation_buffer.write();
                buffer.push_agent(&response_text, &space_id.to_string());
            }

            self.publish_phase_completed(&session_id, Phase::Interview, "chat response")
                .await;

            return Ok(OrchestrationResult {
                session_id: Some(session_id.clone()),
                space_id: Some(space_id),
                space_tag: Some(space_tag.clone()),
                response: response_text,
                seed_id: None,
                agent_id: None,
                phase_reached: Phase::Interview,
                evaluation_passed: false,
                output: None,
            });
        }

        // Phase 1: Interview
        self.publish_phase_started(&session_id, Phase::Interview)
            .await;

        // Get or create the interview session.
        let needs_interview = {
            let sessions = self.sessions.read();
            !sessions.contains_key(&session_id)
        };

        // Conduct the interview.
        let interview = {
            tracing::info!(phase = "interview", "Starting interview phase");
            if needs_interview {
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
            }
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

            self.publish_phase_completed(&session_id, Phase::Interview, "needs clarification")
                .await;

            return Ok(OrchestrationResult {
                session_id: Some(session_id.clone()),
                space_id: Some(space_id),
                space_tag: Some(space_tag.clone()),
                response: format_questions(&questions),
                seed_id: None,
                agent_id: None,
                phase_reached: Phase::Interview,
                evaluation_passed: false,
                output: None,
            });
        }

        // Record agent response in conversation buffer (for topic shift detection)
        // Note: interview phase returns questions, not a full agent response,
        // but we record it for completeness.
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_agent("[interview: questions]", &space_id);
        }

        // Interview complete and ready. Proceed to seed generation.
        self.publish_phase_completed(&session_id, Phase::Interview, "ready for seed")
            .await;
        self.publish_phase_started(&session_id, Phase::Seed).await;

        // Phase 2: Generate seed
        tracing::info!(phase = "seed", "Starting seed generation");
        let seed = self.ouroboros.generate_seed(&interview).await?;

        // Save seed to state store.
        self.save_seed(&seed).await?;

        // Publish seed created event.
        self.event_bus
            .publish(KernelEvent::SeedCreated { seed_id: seed.id })?;

        self.publish_phase_completed(&session_id, Phase::Seed, "generated")
            .await;
        self.publish_phase_started(&session_id, Phase::Execute)
            .await;

        // Check if the seed should be split into multi-agent execution.
        // When the seed has 3+ acceptance criteria, we treat each criterion
        // as a distinct subtask and delegate to separate agents.
        if should_split_seed(&seed) {
            let subtasks = split_into_subtasks(&seed);
            if subtasks.len() > 1 {
                tracing::info!(
                    phase = "delegate",
                    subtasks = subtasks.len(),
                    "Delegating to multi-agent"
                );
                let results = self.delegate_subtasks(subtasks, &seed).await?;

                // Combine successful results
                let combined: String = results
                    .iter()
                    .filter(|r| r.success)
                    .filter_map(|r| r.result.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n\n");

                let all_passed = results.iter().all(|r| r.success);

                // Clean up the session.
                {
                    let mut sessions = self.sessions.write();
                    sessions.remove(&session_id);
                }

                tracing::info!(
                    session_id = %session_id,
                    subtasks = results.len(),
                    passed = all_passed,
                    "Multi-agent orchestration complete"
                );

                return Ok(OrchestrationResult {
                    session_id: Some(session_id),
                    space_id: Some(space_id),
                    space_tag: Some(space_tag.clone()),
                    response: format_result_combined(&combined),
                    seed_id: Some(seed.id),
                    agent_id: None,
                    phase_reached: Phase::Execute,
                    evaluation_passed: all_passed,
                    output: Some(combined),
                });
            }
        }

        // Record agent response in conversation buffer (for multi-agent case)
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_agent("[multi-agent: complete]", &space_id);
        }

        // Phase 3: Fork and execute agent via lifecycle manager
        tracing::info!(phase = "execute", "Starting execution phase");
        let exec_result = self
            .lifecycle
            .spawn_and_run(&seed, Priority::Normal)
            .await?;

        // Periodically reap zombie tasks.
        self.lifecycle.reap_zombies();

        // Phase 4: Evaluate
        self.publish_phase_completed(&session_id, Phase::Execute, "completed")
            .await;
        self.publish_phase_started(&session_id, Phase::Evaluate)
            .await;

        tracing::info!(phase = "evaluate", "Starting evaluation phase");
        let evaluation = self.ouroboros.evaluate(&seed, &exec_result).await?;

        self.publish_phase_completed(
            &session_id,
            Phase::Evaluate,
            &format!("score={:.2}", evaluation.score),
        )
        .await;

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

        while !current_evaluation.all_passed()
            && current_evaluation.score < 0.8
            && iterations < MAX_EVOLUTION_ITERATIONS
        {
            iterations += 1;
            self.publish_phase_started(&session_id, Phase::Evolve).await;

            tracing::info!(
                phase = "evolve",
                iteration = iterations,
                "Starting evolve phase"
            );
            let evolve_result = self
                .ouroboros
                .evolve(
                    current_seed.as_ref().expect("seed exists"),
                    &current_evaluation,
                )
                .await?;

            if let Some(evolved) = evolve_result {
                current_seed = Some(evolved.clone());

                // Save evolved seed.
                self.save_seed(&evolved).await?;
                self.event_bus.publish(KernelEvent::SeedCreated {
                    seed_id: evolved.id,
                })?;

                self.publish_phase_completed(&session_id, Phase::Evolve, "evolved")
                    .await;
                self.publish_phase_started(&session_id, Phase::Execute)
                    .await;

                tracing::info!(
                    phase = "re-execute",
                    iteration = iterations,
                    "Re-executing with evolved seed"
                );
                let new_exec = self
                    .lifecycle
                    .spawn_and_run(&evolved, Priority::High)
                    .await?;

                self.publish_phase_completed(&session_id, Phase::Execute, "completed")
                    .await;
                self.publish_phase_started(&session_id, Phase::Evaluate)
                    .await;

                tracing::info!(
                    phase = "re-evaluate",
                    iteration = iterations,
                    "Re-evaluating evolved result"
                );
                let new_eval = self.ouroboros.evaluate(&evolved, &new_exec).await?;
                current_evaluation = new_eval;

                self.publish_phase_completed(
                    &session_id,
                    Phase::Evaluate,
                    &format!("score={:.2}", current_evaluation.score),
                )
                .await;
                // Save evolved seed evaluation for lineage tracking.
                self.save_evaluation(&evolved, &current_evaluation).await?;
            } else {
                // No evolution possible.
                self.publish_phase_completed(&session_id, Phase::Evolve, "no evolution")
                    .await;
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

        // Measure orchestration duration.
        let metrics = get_metrics();
        metrics
            .orch_duration
            .observe(orch_start.elapsed().as_secs_f64());
        if passed {
            metrics.agents_completed.inc();
        } else {
            metrics.agents_failed.inc();
        }

        // Record agent response in conversation buffer (for topic shift detection)
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_agent(&final_seed.goal, &space_id);
        }

        Ok(OrchestrationResult {
            session_id: Some(session_id),
            space_id: Some(space_id),
            space_tag: Some(space_tag.clone()),
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

        self.git_commit(&format!("seeds/{}.json", key), "ourobors: save seed");

        Ok(())
    }

    /// Save an evaluation result to the state store.
    async fn save_evaluation(&self, seed: &Seed, evaluation: &EvaluationResult) -> Result<()> {
        let key = format!("{}-eval", seed.id);

        self.state_store
            .save_json("evals", &key, evaluation)
            .await
            .context("failed to save evaluation to state store")?;

        self.git_commit(&format!("evals/{}.json", key), "ourobors: save eval");

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

    /// Execute multiple subtasks using separate agents in parallel.
    ///
    /// When A2A is available, the orchestrator delegates tasks through the
    /// A2A protocol — finding capable agents via the AgentCardRegistry.
    /// Otherwise, falls back to direct lifecycle execution.
    ///
    /// Results are collected as they complete using `JoinSet`.
    pub async fn delegate_subtasks(
        &self,
        subtasks: Vec<SubTask>,
        parent_seed: &Seed,
    ) -> Result<Vec<SubTask>> {
        // Single task — execute directly without group overhead.
        if subtasks.len() == 1 {
            return self.execute_single_subtask(subtasks, parent_seed).await;
        }

        // Try A2A-based delegation when the protocol is available.
        if let Some(ref a2a) = self.a2a {
            return self.delegate_via_a2a(subtasks, parent_seed, a2a).await;
        }

        // Fallback: direct lifecycle execution (no A2A).
        self.delegate_via_lifecycle(subtasks, parent_seed).await
    }

    /// Execute a single subtask directly via lifecycle manager.
    async fn execute_single_subtask(
        &self,
        subtasks: Vec<SubTask>,
        parent_seed: &Seed,
    ) -> Result<Vec<SubTask>> {
        let mut task = subtasks.into_iter().next().unwrap();
        let child_seed = Seed {
            id: Uuid::new_v4(),
            goal: task.description.clone(),
            constraints: parent_seed.constraints.clone(),
            acceptance_criteria: vec!["Task completes successfully".into()],
            ontology: parent_seed.ontology.clone(),
            created_at: chrono::Utc::now(),
            generation: parent_seed.generation + 1,
            parent_seed_id: Some(parent_seed.id),
            cspace_hint: None,
        };
        match self
            .lifecycle
            .spawn_and_run(&child_seed, Priority::Normal)
            .await
        {
            Ok(result) => {
                task.result = Some(result.output.clone());
            }
            Err(e) => {
                task.result = Some(format!("Failed: {e}"));
                task.success = false;
            }
        }
        Ok(vec![task])
    }

    /// Delegate subtasks via A2A protocol.
    ///
    /// Queries the AgentCardRegistry for agents matching each subtask's
    /// Execute subtasks via A2A dispatch handler.
    ///
    /// Queries the AgentCardRegistry for agents matching each subtask's
    /// required capability, then calls `execute_delegation` which runs
    /// the task through the registered handler (lifecycle).
    /// Falls back to direct lifecycle execution when no handler is registered.
    async fn delegate_via_a2a(
        &self,
        subtasks: Vec<SubTask>,
        parent_seed: &Seed,
        a2a: &Arc<crate::a2a::A2AProtocol>,
    ) -> Result<Vec<SubTask>> {
        use crate::a2a::TaskPriority;
        use tokio::task::JoinSet;

        tracing::info!(
            subtasks = subtasks.len(),
            "Delegating subtasks via A2A protocol"
        );

        let orchestrator_id: crate::types::AgentId = uuid::Uuid::nil();
        let subtask_count = subtasks.len();
        let mut join_set: JoinSet<(usize, SubTask)> = JoinSet::new();

        for (idx, subtask) in subtasks.into_iter().enumerate() {
            let capability = subtask.required_capability.clone();
            let description = subtask.description.clone();
            let subtask_id = subtask.id;
            let role = subtask.role.clone();
            let a2a = Arc::clone(a2a);
            let parent_seed = parent_seed.clone();
            let lifecycle = self.lifecycle.clone();

            join_set.spawn(async move {
                // Find agent with the required capability via A2A registry.
                let target: Option<crate::a2a::AgentCard> = if let Some(ref cap) = capability {
                    a2a.query_capabilities(cap).await.ok()
                        .and_then(|agents| agents.into_iter().next())
                } else {
                    None
                };

                let (output, success) = if let Some(ref target_card) = target {
                    let target_id = target_card.agent_id;
                    tracing::info!(
                        subtask_index = idx,
                        target = %target_card.name,
                        target_id = %target_id,
                        "A2A dispatching subtask"
                    );

                    let task = crate::a2a::TaskSpec::new(&description, serde_json::json!({
                        "parent_seed": parent_seed.id.to_string(),
                        "goal": description,
                    }))
                    .with_priority(TaskPriority::Normal);

                    // Enqueue audit trail (fire-and-forget into queue).
                    let _ = a2a.delegate_task(orchestrator_id, target_id, task.clone()).await;

                    // Execute through dispatch handler (blocking).
                    match a2a.execute_delegation(orchestrator_id, target_id, task).await {
                        Some(Ok(result)) => {
                            let out = result.get("output")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let ok = result.get("success")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            (out, ok)
                        }
                        Some(Err(e)) => {
                            tracing::warn!(subtask_index = idx, error = %e, "execute_delegation failed");
                            (format!("Failed: {e}"), false)
                        }
                        None => {
                            // No handler — fallback to lifecycle.
                            tracing::warn!(subtask_index = idx, "No dispatch handler, lifecycle fallback");
                            run_via_lifecycle(&lifecycle, &parent_seed, &description).await
                        }
                    }
                } else {
                    tracing::info!(subtask_index = idx, "No A2A agent found, lifecycle fallback");
                    run_via_lifecycle(&lifecycle, &parent_seed, &description).await
                };

                (idx, SubTask {
                    id: subtask_id,
                    description,
                    required_capability: capability,
                    result: Some(output),
                    success,
                    role,
                })
            });
        }

        let mut results: Vec<Option<SubTask>> = vec![None; subtask_count];
        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok((idx, subtask)) => {
                    results[idx] = Some(subtask);
                }
                Err(e) => {
                    tracing::error!(error = %e, "A2A task panicked");
                }
            }
        }

        let completed: Vec<SubTask> = results.into_iter().flatten().collect();
        tracing::info!(
            completed = completed.len(),
            succeeded = completed.iter().filter(|r| r.success).count(),
            "A2A delegation complete"
        );
        Ok(completed)
    }

    async fn delegate_via_lifecycle(
        &self,
        subtasks: Vec<SubTask>,
        parent_seed: &Seed,
    ) -> Result<Vec<SubTask>> {
        use crate::agent_group::OxiosAgentGroup;
        use tokio::task::JoinSet;

        let descriptions: Vec<String> = subtasks.iter().map(|st| st.description.clone()).collect();
        let group = OxiosAgentGroup::new(parent_seed, descriptions);
        let group_id = group.id;

        self.event_bus.publish(KernelEvent::AgentGroupCreated {
            group_id,
            agent_count: group.agents.len(),
        })?;

        tracing::info!(
            group_id = %group_id,
            agent_count = group.agents.len(),
            "Starting parallel multi-agent execution"
        );

        let mut join_set: JoinSet<(
            usize,
            crate::types::AgentId,
            Result<oxios_ouroboros::ExecutionResult>,
        )> = JoinSet::new();

        for (idx, agent_entry) in group.agents.iter().enumerate() {
            let child_seed = agent_entry.seed.clone();
            let agent_id = agent_entry.id;
            let lifecycle = self.lifecycle.clone();

            join_set.spawn(async move {
                let result = lifecycle.spawn_and_run(&child_seed, Priority::Normal).await;
                (idx, agent_id, result)
            });
        }

        let subtask_count = subtasks.len();
        let mut completed = vec![None; subtask_count];
        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok((idx, agent_id, Ok(exec_result))) => {
                    let _ = self
                        .event_bus
                        .publish(KernelEvent::AgentGroupMemberCompleted {
                            group_id,
                            agent_id,
                            success: exec_result.success,
                        });
                    completed[idx] = Some(SubTask {
                        id: subtasks[idx].id,
                        description: subtasks[idx].description.clone(),
                        required_capability: subtasks[idx].required_capability.clone(),
                        result: Some(exec_result.output.clone()),
                        success: exec_result.success,
                        role: subtasks[idx].role.clone(),
                    });
                }
                Ok((idx, agent_id, Err(e))) => {
                    tracing::warn!(subtask_index = idx, error = %e, "Subtask failed");
                    let _ = self
                        .event_bus
                        .publish(KernelEvent::AgentGroupMemberCompleted {
                            group_id,
                            agent_id,
                            success: false,
                        });
                    completed[idx] = Some(SubTask {
                        id: subtasks[idx].id,
                        description: subtasks[idx].description.clone(),
                        required_capability: subtasks[idx].required_capability.clone(),
                        result: Some(format!("Failed: {e}")),
                        success: false,
                        role: subtasks[idx].role.clone(),
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "JoinSet task panicked");
                }
            }
        }

        let completed: Vec<SubTask> = completed.into_iter().flatten().collect();
        let succeeded = completed.iter().filter(|r| r.success).count();
        let total = completed.len();

        tracing::info!(
            group_id = %group_id,
            succeeded,
            total,
            "Parallel multi-agent execution complete"
        );

        // Persist group state
        let _ = self
            .state_store
            .save_json("agent_groups", &group_id.to_string(), &group)
            .await;
        self.git_commit(
            &format!("agent_groups/{}.json", group_id),
            "orchestrator: save group",
        );

        Ok(completed)
    }

    /// Detect whether a message is conversational (no tool execution needed).
    ///
    /// Chat messages are greetings, small talk, questions about the system,
    /// gratitude, emoji, or generally short messages that don't describe
    /// a concrete task to execute.
    fn is_chat_message(msg: &str) -> bool {
        let trimmed = msg.trim();
        let lower = trimmed.to_lowercase();

        // Empty or very short
        if trimmed.len() <= 2 {
            return true;
        }

        // Pure emoji / symbols
        if trimmed.chars().all(|c| !c.is_alphanumeric()) {
            return true;
        }

        // Common greeting patterns (Korean + English)
        let chat_patterns = [
            "안녕", "하이", "헬로", "hello", "hi", "hey", "good morning",
            "good evening", "좋은 아침", "좋은 저녁", "반가",
            "고마워", "감사", "thanks", "thank you", "thx",
            "잘했어", "수고", "good job", "nice", "great",
            "네", "아니요", "yes", "no", "ok", "okay", "ㅇㅇ",
            "ㅎㅎ", "ㅋㅋ", "ㅠㅠ", "^^", "😀", "👍",
        ];
        for pat in &chat_patterns {
            if lower.contains(pat) {
                return true;
            }
        }

        // If the message is short (< 30 chars) and doesn't contain
        // action verbs, treat as chat
        if trimmed.len() < 30 {
            let action_verbs = [
                "만들", "생성", "실행", "수정", "삭", "작성", "읽", "찾", "검색",
                "create", "make", "write", "run", "execute", "build", "delete",
                "fix", "read", "find", "search", "list", "show", "get",
                "분석", "디버그", "리팩토링", "배포", "테스트",
                "deploy", "test", "debug", "refactor", "analyze",
                "파일", "코드", "스크립트",
                "file", "code", "script",
            ];
            let has_action = action_verbs.iter().any(|v| lower.contains(v));
            if !has_action {
                return true;
            }
        }

        false
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
    /// The Space ID that handled this message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<Uuid>,
    /// Space decoration tag for the response (e.g. "[🔧 oxios]").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_tag: Option<String>,
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
    lines.push(format!("Task '{}' completed.", seed.goal));

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

/// Check if a seed should be split into subtasks.
///
/// Simple heuristic: if the seed has 3 or more acceptance criteria,
/// it likely contains distinct concerns that can be parallelized.
fn should_split_seed(seed: &Seed) -> bool {
    seed.acceptance_criteria.len() >= 3
}

/// Split a seed into subtasks based on acceptance criteria.
///
/// Each acceptance criterion becomes a separate subtask with the
/// parent seed's goal as context. Infers required capability from
/// the goal text using the same heuristic as `build_agent_card`.
fn split_into_subtasks(seed: &Seed) -> Vec<SubTask> {
    seed.acceptance_criteria
        .iter()
        .map(|criterion| {
            let desc = format!("{}: {}", seed.goal, criterion);
            let desc_lower = desc.to_lowercase();

            // Infer capability from subtask description.
            let cap = if desc_lower.contains("review") || desc_lower.contains("code") {
                Some("code-review".to_string())
            } else if desc_lower.contains("test") {
                Some("testing".to_string())
            } else if desc_lower.contains("refactor") || desc_lower.contains("improve") {
                Some("refactoring".to_string())
            } else if desc_lower.contains("write")
                || desc_lower.contains("create")
                || desc_lower.contains("implement")
            {
                Some("code-generation".to_string())
            } else if desc_lower.contains("debug") || desc_lower.contains("fix") {
                Some("debugging".to_string())
            } else {
                None
            };

            let mut subtask = SubTask::new(desc);
            subtask.required_capability = cap;
            subtask
        })
        .collect()
}

/// Format combined results from multi-agent execution.
fn format_result_combined(combined: &str) -> String {
    if combined.is_empty() {
        "No subtasks completed successfully.".to_string()
    } else {
        format!("Multi-agent execution completed:\n\n{}", combined)
    }
}

/// Execute a subtask via lifecycle manager, returning (output, success).
async fn run_via_lifecycle(
    lifecycle: &crate::agent_lifecycle::AgentLifecycleManager,
    parent_seed: &Seed,
    description: &str,
) -> (String, bool) {
    let child_seed = Seed {
        id: Uuid::new_v4(),
        goal: description.to_string(),
        constraints: parent_seed.constraints.clone(),
        acceptance_criteria: vec!["Task completes successfully".into()],
        ontology: parent_seed.ontology.clone(),
        created_at: chrono::Utc::now(),
        generation: parent_seed.generation + 1,
        parent_seed_id: Some(parent_seed.id),
        cspace_hint: None,
    };
    match lifecycle.spawn_and_run(&child_seed, Priority::Normal).await {
        Ok(result) => (result.output, result.success),
        Err(e) => (format!("Failed: {e}"), false),
    }
}
