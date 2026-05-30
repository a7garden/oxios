//! Orchestrator: coordinates the Ouroboros lifecycle for user messages.
//!
//! The orchestrator is the "brain" that runs the Ouroboros protocol.
//! Given a user message:
//! 1. Conduct the interview (ask clarifying questions if needed)
//! 2. Generate a seed (via LLM for complex tasks, or ad-hoc for simple tasks)
//! 3. Execute the agent via the supervisor
//! 4. Return the result to the user
//!
//! The orchestrator does NOT know about channels or HTTP — it only
//! coordinates Ouroboros + Supervisor + EventBus + StateStore + Scheduler + AccessManager.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono;
use oxios_ouroboros::{EvaluationResult, ExecutionResult, InterviewResult, OuroborosProtocol, Phase, Seed};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_lifecycle::AgentLifecycleManager;
use crate::event_bus::{EventBus, KernelEvent};
use crate::git_layer::GitLayer;
use crate::metrics::get_metrics;
use crate::scheduler::Priority;
use crate::project::{ProjectId, ProjectManager};
use crate::space::ConversationBuffer;
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
    /// Project manager for context partitioning.
    project_manager: RwLock<Option<Arc<ProjectManager>>>,
    /// Conversation buffer for topic shift detection.
    conversation_buffer: RwLock<ConversationBuffer>,
    /// Orchestrator configuration (Ouroboros protocol settings).
    delegation_config: DelegationConfig,
    /// A2A circuit breaker for delegation reliability.
    a2a_breaker: Arc<crate::a2a_circuit_breaker::A2ACircuitBreaker>,
    /// Evolution loop settings.
    evolution_config: EvolutionConfig,
}

/// Configuration for A2A delegation retries.
#[derive(Debug, Clone)]
struct DelegationConfig {
    /// Maximum retry attempts for A2A delegation.
    max_retries: u32,
    /// Base delay for exponential backoff (milliseconds).
    base_delay_ms: u64,
    /// Maximum delay cap for exponential backoff (milliseconds).
    max_delay_ms: u64,
    /// Timeout per delegation attempt (milliseconds).
    #[allow(dead_code)]
    timeout_ms: u64,
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            timeout_ms: 5000,
        }
    }
}

impl DelegationConfig {
    /// Calculate exponential backoff delay.
    fn backoff_delay(&self, attempt: u32) -> u64 {
        let delay = self.base_delay_ms * 2_u64.saturating_pow(attempt.min(10));
        delay.min(self.max_delay_ms)
    }
}

/// Evolution loop settings extracted from OrchestratorConfig.
#[derive(Debug, Clone)]
struct EvolutionConfig {
    /// Maximum evolution iterations (0 = evaluate only).
    max_iterations: u32,
    /// Minimum score to pass evaluation.
    score_threshold: f64,
    /// Enable evaluation result caching.
    #[allow(dead_code)]
    eval_cache_enabled: bool,
}

impl From<crate::config::OrchestratorConfig> for EvolutionConfig {
    fn from(c: crate::config::OrchestratorConfig) -> Self {
        Self {
            max_iterations: c.max_evolution_iterations,
            score_threshold: c.min_evaluation_score,
            eval_cache_enabled: c.eval_cache_enabled,
        }
    }
}

impl Orchestrator {
    /// Creates a new orchestrator.
    pub fn new(
        ouroboros: Arc<dyn OuroborosProtocol>,
        event_bus: EventBus,
        state_store: Arc<StateStore>,
        lifecycle: AgentLifecycleManager,
    ) -> Self {
        Self::with_config(
            ouroboros,
            event_bus,
            state_store,
            lifecycle,
            crate::config::OrchestratorConfig::default(),
        )
    }

    /// Creates a new orchestrator with custom config.
    pub fn with_config(
        ouroboros: Arc<dyn OuroborosProtocol>,
        event_bus: EventBus,
        state_store: Arc<StateStore>,
        lifecycle: AgentLifecycleManager,
        config: crate::config::OrchestratorConfig,
    ) -> Self {
        let evolution_config = EvolutionConfig::from(config);
        Self {
            ouroboros,
            event_bus,
            state_store,
            git_layer: None,
            sessions: RwLock::new(std::collections::HashMap::new()),
            lifecycle,
            a2a: None,
            project_manager: RwLock::new(None),
            conversation_buffer: RwLock::new(ConversationBuffer::default()),
            delegation_config: DelegationConfig::default(),
            a2a_breaker: Arc::new(crate::a2a_circuit_breaker::A2ACircuitBreaker::new(5, 30)),
            evolution_config,
        }
    }

    /// Set the ProjectManager for context partitioning.
    pub fn set_project_manager(&self, manager: Arc<ProjectManager>) {
        *self.project_manager.write() = Some(manager);
    }

    /// Get a reference to the ProjectManager, if set.
    pub fn project_manager(&self) -> Option<Arc<ProjectManager>> {
        self.project_manager.read().as_ref().cloned()
    }

    /// Detect a project from a message, returning tag string.
    pub fn detect_project_tag(&self, message: &str) -> Option<String> {
        self.project_manager
            .read()
            .as_ref()
            .and_then(|pm| {
                let projects = pm.list_projects();
                let result = crate::project::detect_project(message, &projects);
                match result {
                    crate::project::DetectionResult::Found(id) => {
                        pm.get_project(id).map(|p| p.tag())
                    }
                    crate::project::DetectionResult::NoMatch { .. } => None,
                }
            })
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
    pub async fn handle_message(
        &self,
        user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
        project_ids: Option<&str>,
    ) -> Result<OrchestrationResult> {
        tracing::info!(name = "orchestrator.handle_message", session_id = %session_id.unwrap_or("new"), "starting");
        get_metrics().messages.inc();
        let orch_start = std::time::Instant::now();

        let session_id = session_id
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        tracing::info!(session_id = %session_id, user_id = %user_id, content_len = user_message.len(), "Orchestrator handling message");

        // ── Project Detection ──
        // Parse project IDs from caller ("uuid1,uuid2,...") or auto-detect.
        let primary_project_id: Option<Uuid> = if let Some(ids_str) = project_ids {
            // Explicit project IDs from caller
            ids_str.split(',').next().and_then(|s| Uuid::parse_str(s.trim()).ok())
        } else {
            // Auto-detect from message
            self.detect_project_tag(user_message).and_then(|tag| {
                // Extract UUID from project manager
                self.project_manager().and_then(|pm| {
                    let projects = pm.list_projects();
                    let result = crate::project::detect_project(user_message, &projects);
                    match result {
                        crate::project::DetectionResult::Found(id) => Some(id),
                        crate::project::DetectionResult::NoMatch { .. } => None,
                    }
                })
            })
        };

        // Resolve project tag for display
        let project_tag = primary_project_id
            .and_then(|id| {
                self.project_manager()
                    .and_then(|pm| pm.get_project(id).map(|p| p.tag()))
            })
            .unwrap_or_default();

        // Touch the project to record activity
        if let Some(pid) = primary_project_id {
            if let Some(pm) = self.project_manager() {
                pm.touch(pid);
            }
        }

        let conversation_turns = {
            let buffer = self.conversation_buffer.read();
            buffer.turns().iter().cloned().collect::<Vec<_>>()
        };

        // Record user message in conversation buffer
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_user(user_message);
        }

        // Phase 1: Interview
        self.publish_phase_started(&session_id, Phase::Interview)
            .await;

        // Get or create the interview session (pre-fetch to avoid lock across await).
        let needs_interview;
        let existing_history: Option<Vec<_>>;
        {
            let sessions = self.sessions.read();
            needs_interview = !sessions.contains_key(&session_id);
            existing_history = if !needs_interview {
                sessions
                    .get(&session_id)
                    .map(|s| s.interview.conversation_history.clone())
            } else {
                None
            };
            // Lock dropped here before any .await
        }

        // Conduct the interview.
        let interview = {
            tracing::info!(phase = "interview", "Starting interview phase");
            if needs_interview {
                self.ouroboros.interview(user_message).await?
            } else {
                // This is a follow-up message in an existing interview.
                // Build multi-turn context from conversation history.
                let multi_turn_context = {
                    let mut context_parts = Vec::new();
                    if let Some(ref history) = existing_history {
                        for exchange in history {
                            context_parts.push(format!(
                                "User: {}\nAgent: {}",
                                exchange.user, exchange.agent
                            ));
                        }
                    }
                    context_parts.push(format!("User: {}", user_message));
                    context_parts.join("\n\n")
                };

                // Record user's answer in session for future turns (brief write lock)
                {
                    let mut sessions = self.sessions.write();
                    if let Some(s) = sessions.get_mut(&session_id) {
                        let last_q = s.interview.questions.last().cloned().unwrap_or_default();
                        s.interview.add_exchange(&last_q, user_message);
                    }
                }

                // Run another interview pass with full conversation history.
                self.ouroboros.interview(&multi_turn_context).await?
            }
        };

        // If this is a non-task message (greeting, small talk), return the chat response directly.
        if !interview.is_task {
            tracing::info!(session_id = %session_id, "Chat response (non-task)");

            let response_text = if interview.chat_response.is_empty() {
                "Hello! How can I help you today?".to_string()
            } else {
                interview.chat_response.clone()
            };

            // Record agent response in conversation buffer
            {
                let mut buffer = self.conversation_buffer.write();
                buffer.push_agent(&response_text, &Uuid::nil());
            }

            // Record exchange in conversation history for multi-turn
            // and store session so multi-turn works on follow-up messages
            {
                let mut sessions = self.sessions.write();
                if let Some(session) = sessions.get_mut(&session_id) {
                    tracing::debug!(session_id = %session_id, history_len = session.interview.conversation_history.len(), "Adding to existing session history");
                    session
                        .interview
                        .add_to_history(user_message, &response_text);
                } else {
                    // First non-task message — create a minimal session for history
                    let mut interview = InterviewResult::new();
                    interview.is_task = false;
                    interview.chat_response = response_text.clone();
                    interview.add_to_history(user_message, &response_text);
                    sessions.insert(
                        session_id.clone(),
                        InterviewSession {
                            id: session_id.clone(),
                            interview,
                            phase: Phase::Interview,
                            seed_id: None,
                            agent_id: None,
                        },
                    );
                }
            }

            self.publish_phase_completed(&session_id, Phase::Interview, "chat")
                .await;

            return Ok(OrchestrationResult {
                session_id: Some(session_id.clone()),
                primary_project_id,
                project_tag: Some(project_tag.clone()),
                response: response_text,
                seed_id: None,
                agent_id: None,
                phase_reached: Phase::Interview,
                evaluation_passed: false,
                output: None,
            });
        }

        // If ambiguity is too high, return questions for the user to answer.
        if !interview.ready_for_seed {
            // Record this exchange in conversation history and store the interview.
            {
                let mut sessions = self.sessions.write();
                let session =
                    sessions
                        .entry(session_id.clone())
                        .or_insert_with(|| InterviewSession {
                            id: session_id.clone(),
                            interview: interview.clone(),
                            phase: Phase::Interview,
                            seed_id: None,
                            agent_id: None,
                        });
                // The session already has user's answer recorded via add_exchange above.
                // Record the questions as the agent's response in history.
                let questions_text = interview.questions.join("\n");
                let last_answer = session.interview.answers.last().cloned();
                if let Some(ref ans) = last_answer {
                    if !ans.is_empty() {
                        session.interview.add_to_history(ans, &questions_text);
                    }
                }
            } // Lock dropped before .await

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
                primary_project_id,
                project_tag: Some(project_tag.clone()),
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
            buffer.push_agent("[interview: ready]", &Uuid::nil());
        }

        // Interview complete and ready.
        self.publish_phase_completed(&session_id, Phase::Interview, "ready")
            .await;
        self.publish_phase_started(&session_id, Phase::Seed).await;

        // ── Complexity-based routing ──
        //
        // "simple" + low ambiguity → create a lightweight Seed from the user
        // message directly (no LLM call) and skip formal evaluation.
        // "complex" (or ambiguous simple) → generate a full Seed via LLM.
        let is_simple = interview.complexity == "simple" && interview.ambiguity.ambiguity() <= 0.3;

        let seed = if is_simple {
            tracing::info!(
                phase = "seed",
                method = "from_message",
                "Simple task — ad-hoc seed"
            );
            Seed::from_message(&interview.original_message)
        } else {
            tracing::info!(
                phase = "seed",
                method = "llm",
                "Complex task — LLM-generated seed"
            );
            self.ouroboros.generate_seed(&interview).await?
        };

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
                    primary_project_id,
                    project_tag: Some(project_tag.clone()),
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
            buffer.push_agent("[multi-agent: complete]", &Uuid::nil());
        }

        // Execute agent via lifecycle manager.
        tracing::info!(phase = "execute", "Starting execution phase");
        let exec_result = self
            .lifecycle
            .spawn_and_run(&seed, Priority::Normal)
            .await?;

        // Periodically reap zombie tasks.
        self.lifecycle.reap_zombies();

        self.publish_phase_completed(&session_id, Phase::Execute, "completed")
            .await;

        // ── Evaluate + Evolve ──
        //
        // Three paths:
        // 1. output_schema → structured validation (no evolution)
        // 2. acceptance_criteria present → full evaluate + optional evolve loop
        // 3. neither → simple boolean pass/fail
        let (final_result, final_seed, passed, phase_reached) =
            if let Some(ref schema) = seed.output_schema {
                // Structured output validation — no evolution.
                let passed = match oxi_sdk::StructuredOutput::extract(
                    &exec_result.output,
                    &oxi_sdk::OutputMode::ValidatedJson {
                        schema: schema.clone(),
                    },
                ) {
                    Ok(_) => {
                        tracing::info!(session_id = %session_id, "Structured output validation passed");
                        true
                    }
                    Err(e) => {
                        tracing::warn!(session_id = %session_id, error = %e, "Structured output validation failed");
                        false
                    }
                };
                (exec_result, seed.clone(), passed, Phase::Execute)
            } else if self.should_evaluate(&seed) {
                // Full Ouroboros evaluate + optional evolve loop.
                self.publish_phase_started(&session_id, Phase::Evaluate).await;

                let (result, eval, evolved_seed) = self
                    .run_evolution_loop(&session_id, &seed, exec_result)
                    .await?;

                let passed = eval.all_passed() && eval.score >= self.evolution_config.score_threshold;

                self.publish_phase_completed(
                    &session_id,
                    Phase::Evaluate,
                    &format!("score={:.2}", eval.score),
                )
                .await;

                let reached = if evolved_seed.generation > 0 {
                    Phase::Evolve
                } else {
                    Phase::Evaluate
                };

                (result, evolved_seed, passed, reached)
            } else {
                // Simple task: boolean pass/fail, no LLM evaluation.
                let passed = exec_result.success;
                (exec_result, seed.clone(), passed, Phase::Execute)
            };

        // Clean up the session.
        {
            let mut sessions = self.sessions.write();
            sessions.remove(&session_id);
        }

        tracing::info!(
            session_id = %session_id,
            passed,
            phase = %phase_reached,
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
            buffer.push_agent(&final_seed.goal, &Uuid::nil());
        }

        Ok(OrchestrationResult {
            session_id: Some(session_id),
            primary_project_id,
            project_tag: Some(project_tag.clone()),
            response: format_execution_result(&final_seed, &final_result),
            seed_id: Some(final_seed.id),
            agent_id: None,
            phase_reached,
            evaluation_passed: passed,
            output: Some(final_result.output.clone()),
        })
    }

    /// Check whether a seed should go through full evaluate + evolve.
    ///
    /// Only seeds with acceptance criteria and no output_schema qualify.
    /// Simple tasks (from_message, no criteria) get boolean pass/fail.
    fn should_evaluate(&self, seed: &Seed) -> bool {
        !seed.acceptance_criteria.is_empty() && seed.output_schema.is_none()
    }

    /// Execute a seed via the lifecycle manager.
    async fn execute_seed(&self, seed: &Seed) -> Result<ExecutionResult> {
        self.lifecycle.spawn_and_run(seed, Priority::Normal).await
    }

    /// Evaluate → (optional) Evolve → re-execute loop.
    ///
    /// Tracks the best result seen across iterations. If evolution
    /// degrades the score, returns the previous best.
    async fn run_evolution_loop(
        &self,
        _session_id: &str,
        seed: &Seed,
        initial_result: ExecutionResult,
    ) -> Result<(ExecutionResult, EvaluationResult, Seed)> {
        let max_iterations = self.evolution_config.max_iterations;
        let threshold = self.evolution_config.score_threshold;

        let mut current_seed = seed.clone();
        let mut current_result = initial_result;

        // Best-result tracking.
        let mut best_result = current_result.clone();
        let mut best_seed = current_seed.clone();
        let mut best_eval: Option<EvaluationResult> = None;

        for iteration in 0..=max_iterations {
            // Evaluate
            let evaluation = self
                .ouroboros
                .evaluate(&current_seed, &current_result)
                .await?;

            tracing::info!(
                iteration,
                seed_id = %current_seed.id,
                score = evaluation.score,
                passed = evaluation.all_passed(),
                "Evaluation complete"
            );

            let _ = self.event_bus.publish(KernelEvent::EvaluationComplete {
                seed_id: current_seed.id,
                passed: evaluation.all_passed(),
            });

            // Update best if this iteration improved.
            if best_eval
                .as_ref()
                .is_none_or(|b| evaluation.score >= b.score)
            {
                best_result = current_result.clone();
                best_seed = current_seed.clone();
                best_eval = Some(evaluation.clone());
            }

            // Passed or exhausted iterations.
            if evaluation.score >= threshold || iteration == max_iterations {
                if iteration == max_iterations && max_iterations > 0 {
                    let _ = self.event_bus.publish(KernelEvent::EvolutionMaxReached {
                        seed_id: current_seed.id,
                        final_score: evaluation.score,
                        iterations: iteration,
                    });
                }
                return Ok((best_result, best_eval.ok_or_else(|| anyhow::anyhow!("Evolve loop exited with threshold met but no evaluation was produced"))?, best_seed));
            }

            // max_iterations == 0 → evaluate only, no evolution.
            if max_iterations == 0 {
                return Ok((best_result, best_eval.ok_or_else(|| anyhow::anyhow!("No iterations configured and no evaluation was produced"))?, best_seed));
            }

            // Evolve: produce an improved seed.
            let evolved = self
                .ouroboros
                .evolve(&current_seed, &evaluation)
                .await?;
            match evolved {
                Some(new_seed) => {
                    tracing::info!(
                        old_seed_id = %current_seed.id,
                        new_seed_id = %new_seed.id,
                        iteration,
                        "Seed evolved, re-executing"
                    );

                    let _ = self.event_bus.publish(KernelEvent::EvolutionStarted {
                        seed_id: current_seed.id,
                        new_seed_id: new_seed.id,
                        iteration,
                    });

                    // Save the evolved seed.
                    self.save_seed(&new_seed).await?;

                    current_seed = new_seed;
                    current_result = self.execute_seed(&current_seed).await?;
                }
                None => {
                    tracing::info!(
                        seed_id = %current_seed.id,
                        "Evolve returned None, stopping loop"
                    );
                    return Ok((best_result, best_eval.ok_or_else(|| anyhow::anyhow!("Evolve returned no seed and no evaluation was produced"))?, best_seed));
                }
            }
        }

        // Unreachable: every branch above returns.
        unreachable!()
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
    /// A2A protocol with circuit breaker and retry support.
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
            // Check circuit breaker
            if !self.a2a_breaker.is_allowed() {
                tracing::warn!(
                    state = ?self.a2a_breaker.state(),
                    "A2A circuit breaker open, using lifecycle fallback"
                );
                return self.delegate_via_lifecycle(subtasks, parent_seed).await;
            }

            // Delegate with retry
            return self.delegate_with_retry(subtasks, parent_seed, a2a).await;
        }

        // Fallback: direct lifecycle execution (no A2A).
        self.delegate_via_lifecycle(subtasks, parent_seed).await
    }

    /// Delegate subtasks via A2A with circuit breaker and retry support.
    async fn delegate_with_retry(
        &self,
        subtasks: Vec<SubTask>,
        parent_seed: &Seed,
        a2a: &Arc<crate::a2a::A2AProtocol>,
    ) -> Result<Vec<SubTask>> {
        let mut attempt = 0;
        let max_retries = self.delegation_config.max_retries;

        loop {
            match self
                .delegate_via_a2a(subtasks.clone(), parent_seed, a2a)
                .await
            {
                Ok(results) => {
                    self.a2a_breaker.record_success();
                    return Ok(results);
                }
                Err(e) => {
                    self.a2a_breaker.record_failure();
                    attempt += 1;

                    if attempt >= max_retries {
                        tracing::error!(
                            attempts = attempt,
                            error = %e,
                            "A2A delegation exhausted after {} attempts, using lifecycle fallback",
                            attempt
                        );
                        return self.delegate_via_lifecycle(subtasks, parent_seed).await;
                    }

                    // Exponential backoff
                    let delay = self.delegation_config.backoff_delay(attempt);
                    tracing::warn!(
                        attempt,
                        delay_ms = delay,
                        error = %e,
                        "A2A delegation failed, retrying with backoff"
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }

    /// Execute a single subtask directly via lifecycle manager.
    async fn execute_single_subtask(
        &self,
        subtasks: Vec<SubTask>,
        parent_seed: &Seed,
    ) -> Result<Vec<SubTask>> {
        let mut task = subtasks
            .into_iter()
            .next()
            .expect("execute_single_subtask is only called when subtasks is non-empty");
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
            original_request: parent_seed.original_request.clone(),
            output_schema: None,
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
    pub primary_project_id: Option<Uuid>,
    /// Space decoration tag for the response (e.g. "[🔧 oxios]").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_tag: Option<String>,
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
/// Format execution result for display to the user.
fn format_execution_result(seed: &Seed, exec: &ExecutionResult) -> String {
    let mut lines = Vec::new();

    if exec.success {
        lines.push(format!("✅ '{}'", seed.goal));
    } else {
        lines.push(format!(
            "⚠️ '{}'을(를) 시도했지만 완전히 성공하지 못했습니다.",
            seed.goal
        ));
    }

    // Show a truncated preview of the output if present.
    if !exec.output.is_empty() {
        let preview = if exec.output.len() > 500 {
            format!("{}...", &exec.output[..500])
        } else {
            exec.output.clone()
        };
        lines.push(String::new());
        lines.push(preview);
    }

    lines.join("\n")
}

/// Check if a seed should be split into subtasks.
///
/// Simple heuristic: if the seed has 3 or more acceptance criteria,
/// it likely contains distinct concerns that can be parallelized.
fn should_split_seed(seed: &Seed) -> bool {
    // Only split for genuinely complex tasks with many criteria.
    // Simple tasks (even with 3-4 criteria) are better handled by a single agent
    // to preserve context coherence.
    seed.acceptance_criteria.len() >= 5
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
        original_request: parent_seed.original_request.clone(),
            output_schema: None,
    };
    match lifecycle.spawn_and_run(&child_seed, Priority::Normal).await {
        Ok(result) => (result.output, result.success),
        Err(e) => (format!("Failed: {e}"), false),
    }
}
