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
use oxios_ouroboros::{
    EvaluationResult, ExecutionResult, InterviewResult, OuroborosProtocol, Phase, Seed,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_lifecycle::AgentLifecycleManager;
use crate::event_bus::{EventBus, KernelEvent};
use crate::git_layer::GitLayer;
use crate::metrics::get_metrics;
use crate::mount::{MountId, MountManager};
use crate::project::{ConversationBuffer, ProjectManager};
use crate::scheduler::Priority;
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
    /// Mount manager for path-alias context (RFC-025).
    mount_manager: RwLock<Option<Arc<MountManager>>>,
    /// Conversation buffer for topic shift detection.
    conversation_buffer: RwLock<ConversationBuffer>,
    /// Orchestrator configuration (Ouroboros protocol settings).
    delegation_config: DelegationConfig,
    /// A2A circuit breaker for delegation reliability.
    a2a_breaker: Arc<crate::a2a::circuit_breaker::A2ACircuitBreaker>,
    /// Evolution loop settings.
    evolution_config: RwLock<EvolutionConfig>,
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
}

impl From<crate::config::OrchestratorConfig> for EvolutionConfig {
    fn from(c: crate::config::OrchestratorConfig) -> Self {
        Self {
            max_iterations: c.max_evolution_iterations,
            score_threshold: c.min_evaluation_score,
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
        let evolution_config = EvolutionConfig::from(config.clone());
        Self {
            ouroboros,
            event_bus,
            state_store,
            git_layer: None,
            sessions: RwLock::new(std::collections::HashMap::new()),
            lifecycle,
            a2a: None,
            project_manager: RwLock::new(None),
            mount_manager: RwLock::new(None),
            conversation_buffer: RwLock::new(ConversationBuffer::default()),
            delegation_config: DelegationConfig::default(),
            a2a_breaker: Arc::new(crate::a2a::circuit_breaker::A2ACircuitBreaker::new(5, 30)),
            evolution_config: RwLock::new(evolution_config),
        }
    }

    /// Set the ProjectManager for context partitioning.
    pub fn set_project_manager(&self, manager: Arc<ProjectManager>) {
        *self.project_manager.write() = Some(manager);
    }

    /// Set the MountManager for path-alias context (RFC-025).
    pub fn set_mount_manager(&self, manager: Arc<MountManager>) {
        *self.mount_manager.write() = Some(manager);
    }

    /// Get a reference to the MountManager, if set (RFC-025).
    pub fn mount_manager(&self) -> Option<Arc<MountManager>> {
        self.mount_manager.read().as_ref().cloned()
    }

    /// Get a reference to the ProjectManager, if set.
    pub fn project_manager(&self) -> Option<Arc<ProjectManager>> {
        self.project_manager.read().as_ref().cloned()
    }

    /// Detect a project from a message, returning tag string.
    pub fn detect_project_tag(&self, message: &str) -> Option<String> {
        self.project_manager.read().as_ref().and_then(|pm| {
            let projects = pm.list_projects();
            let result = crate::project::detect_project(message, &projects);
            match result {
                crate::project::DetectionResult::Found(id) => pm.get_project(id).map(|p| p.tag()),
                crate::project::DetectionResult::NoMatch { .. } => None,
            }
        })
    }

    /// Resolve the active Mounts for a message (RFC-025).
    ///
    /// Parses explicit `mount_ids` ("uuid1,uuid2,...", primary first); when
    /// none are given, auto-detects from the message. Returns:
    /// - the ordered list of active [`MountId`]s,
    /// - the rendered `## Workspace Context` body (without the header),
    /// - all resolved filesystem paths (primary first),
    /// - a display tag like `[🔧 oxios + oxi-sdk]`.
    ///
    /// Honors the sticky-primary model: when `mount_ids` is explicitly
    /// provided they are used as-is (detection is skipped). Detection only
    /// runs when `mount_ids` is `None`, seeding the primary slot — it never
    /// replaces an explicit primary, only appends a secondary.
    fn resolve_mount_workspace(
        &self,
        mount_ids: Option<&str>,
        project_ids: Option<&str>,
        user_message: &str,
    ) -> (
        Vec<MountId>,
        Option<String>,
        Vec<std::path::PathBuf>,
        String,
    ) {
        use crate::mount::Mount;

        let Some(mm) = self.mount_manager() else {
            return (Vec::new(), None, Vec::new(), String::new());
        };

        // Parse explicit mount_ids; otherwise auto-detect (seeds the primary slot).
        let mut ids: Vec<MountId> = if let Some(ids_str) = mount_ids {
            ids_str
                .split(',')
                .filter_map(|s| MountId::parse_str(s.trim()).ok())
                .collect()
        } else {
            match mm.detect(user_message) {
                crate::mount::DetectionResult::Found(id) => vec![id],
                crate::mount::DetectionResult::NoMatch { .. } => vec![],
            }
        };
        // De-duplicate while preserving order (handles non-consecutive dups).
        let mut seen = std::collections::HashSet::new();
        ids.retain(|id| seen.insert(*id));

        // ── Project-referenced Mount activation (RFC-025) ──
        // When a project_id is provided, auto-activate its referenced Mounts
        // BEFORE we derive mounts/tag/context/paths, so they are fully
        // visible in the system prompt and the badge — not just granted
        // path access. (Previously this ran after the prompt was built, so
        // project-referenced Mounts were invisible in the context body.)
        let project_for_instructions: Option<crate::project::Project> = if let Some(project_ids_str) =
            project_ids
            && let Some(first_id_str) = project_ids_str.split(',').next()
            && let Some(pm) = self.project_manager()
            && let Ok(pid) = Uuid::parse_str(first_id_str.trim())
        {
            let proj = pm.get_project(pid);
            if let Some(ref project) = proj {
                for mid in &project.mount_ids {
                    if !ids.contains(mid) {
                        ids.push(*mid);
                    }
                }
            }
            proj
        } else {
            None
        };

        if ids.is_empty() {
            return (Vec::new(), None, Vec::new(), String::new());
        }

        // Touch each active Mount (record activity) — now includes any
        // Project-referenced Mounts activated above.
        for id in &ids {
            mm.touch(*id);
        }

        let mounts: Vec<Mount> = mm.get_mounts_ordered(&ids);
        if mounts.is_empty() {
            return (Vec::new(), None, Vec::new(), String::new());
        }

        // Collect all paths (primary first, deduped) over the full Mount set.
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        for m in &mounts {
            for p in &m.paths {
                if !paths.contains(p) {
                    paths.push(p.clone());
                }
            }
        }

        // Legacy fallback (RFC-025 migration window): a Project created
        // before Mounts may carry explicit `paths` but no `mount_ids`. In
        // that case grant path access directly so pre-RFC-025 Projects still
        // resolve a CWD and populate `allowed_paths` (see agent_runtime.rs).
        if let Some(project) = &project_for_instructions
            && project.mount_ids.is_empty()
            && !project.paths.is_empty()
        {
            for p in &project.paths {
                if !paths.contains(p) {
                    paths.push(p.clone());
                }
            }
        }

        // Display tag.
        let tag = if mounts.len() == 1 {
            mounts[0].tag()
        } else {
            let names: Vec<&str> = mounts.iter().map(|m| m.name.as_str()).collect();
            format!("[🔧 {}]", names.join(" + "))
        };

        let mut context = build_workspace_context_body(&mounts).unwrap_or_default();

        // ── Project instructions (RFC-025) ──
        // Inject the project's instructions into the context body. The
        // "### Active Mounts" header above is only present when there are
        // actual mount entries in `context`; the Project Instructions section
        // stands on its own when only instructions exist.
        if let Some(project) = project_for_instructions {
            // Cap instructions to stay within the prompt budget (~500 tokens).
            let instructions = if project.instructions.len() > 2000 {
                let mut end = 2000;
                while end > 0 && !project.instructions.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &project.instructions[..end])
            } else {
                project.instructions.clone()
            };
            if !instructions.is_empty() {
                context.push_str(&format!(
                    "\n### Project Instructions: {}\n{}\n",
                    project.name, instructions
                ));
            }
        }

        // Enforce a hard prompt budget on the final context body (~1500 tokens).
        const MAX_CONTEXT_CHARS: usize = 6000;
        if context.len() > MAX_CONTEXT_CHARS {
            let mut end = MAX_CONTEXT_CHARS;
            while end > 0 && !context.is_char_boundary(end) {
                end -= 1;
            }
            context.truncate(end);
            context.push_str("\n...(context truncated)...\n");
        }

        let context_opt = if context.is_empty() {
            None
        } else {
            Some(context)
        };
        (ids, context_opt, paths, tag)
    }

    /// Set the A2A protocol for inter-agent task delegation.
    pub fn set_a2a(&mut self, a2a: Arc<crate::a2a::A2AProtocol>) {
        self.a2a = Some(a2a);
    }

    /// Set the GitLayer for auto-commits after state saves.
    pub fn set_git_layer(&mut self, git_layer: Arc<GitLayer>) {
        self.git_layer = Some(git_layer);
    }

    /// Hot-reload evolution config without restart.
    ///
    /// Takes effect on the next orchestration run.
    pub fn update_evolution_config(&self, config: crate::config::OrchestratorConfig) {
        *self.evolution_config.write() = EvolutionConfig::from(config);
        tracing::info!("Orchestrator evolution config hot-reloaded");
    }

    /// Restore sessions from persisted state.
    ///
    /// Loads sessions from the `StateStore` that have an `active_seed_id`
    /// (meaning they are mid-orchestration) and repopulates the in-memory
    /// interview session map so that follow-up messages can continue
    /// the conversation.
    pub async fn restore_sessions(&self) {
        let summaries = match self.state_store.list_sessions().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to list sessions for restore");
                return;
            }
        };

        let mut restored = 0usize;
        for summary in &summaries {
            // Only restore sessions that are mid-orchestration (have an active seed).
            let Some(ref seed_id_str) = summary.active_seed_id else {
                continue;
            };

            let session_id = crate::state_store::SessionId(summary.id.clone());
            let session = match self.state_store.load_session(&session_id).await {
                Ok(Some(s)) => s,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(
                        session_id = %summary.id,
                        error = %e,
                        "Failed to load session for restore"
                    );
                    continue;
                }
            };

            // Reconstruct an InterviewSession from the persisted data.
            // The interview result is rebuilt from conversation history so
            // that multi-turn context is available on follow-up messages.
            let mut interview = oxios_ouroboros::InterviewResult::new();
            interview.is_task = true; // Has active seed → was a task.
            interview.original_message = session
                .user_messages
                .last()
                .map(|m| m.content.clone())
                .unwrap_or_default();

            // Rebuild conversation history from user/agent exchanges.
            // Use an index loop (not zip) so a trailing user message
            // without a stored agent response (e.g. crash before flush)
            // is preserved with an empty agent turn instead of dropped.
            let history: Vec<oxios_ouroboros::interview::Exchange> = session
                .user_messages
                .iter()
                .enumerate()
                .map(|(i, user)| oxios_ouroboros::interview::Exchange {
                    user: user.content.clone(),
                    agent: session
                        .agent_responses
                        .get(i)
                        .map(|a| a.content.clone())
                        .unwrap_or_default(),
                })
                .collect();
            interview.conversation_history = history;

            let seed_id = seed_id_str.parse::<Uuid>().ok();

            let interview_session = InterviewSession {
                id: session.id.0.clone(),
                interview,
                phase: Phase::Execute,
                seed_id,
                agent_id: None,
            };

            {
                let mut sessions = self.sessions.write();
                sessions.insert(session.id.0.clone(), interview_session);
            }

            restored += 1;
        }

        if restored > 0 {
            tracing::info!(restored, total = summaries.len(), "Sessions restored");
        }
    }

    /// Commit a file to git if GitLayer is configured and enabled.
    fn git_commit(&self, rel_path: &str, message: &str) {
        if let Some(ref gl) = self.git_layer
            && gl.is_enabled()
        {
            let _ = gl.commit_file(rel_path, message);
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
        mount_ids: Option<&str>,
        request_id: &str,
    ) -> Result<OrchestrationResult> {
        tracing::info!(name = "orchestrator.handle_message", session_id = %session_id.unwrap_or("new"), request_id = %request_id, "starting");
        get_metrics().messages.inc();
        let orch_start = std::time::Instant::now();

        let session_id = session_id
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        tracing::info!(session_id = %session_id, user_id = %user_id, request_id = %request_id, content_len = user_message.len(), "Orchestrator handling message");

        // ── Project Detection ──
        // Parse project IDs from caller ("uuid1,uuid2,...") or auto-detect.
        let primary_project_id: Option<Uuid> = if let Some(ids_str) = project_ids {
            // Explicit project IDs from caller
            ids_str
                .split(',')
                .next()
                .and_then(|s| Uuid::parse_str(s.trim()).ok())
        } else {
            // Auto-detect from message
            self.detect_project_tag(user_message).and_then(|_tag| {
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
        if let Some(pid) = primary_project_id
            && let Some(pm) = self.project_manager()
        {
            pm.touch(pid);
        }

        // ── Mount workspace resolution (RFC-025) ──
        // Resolve active Mounts (explicit mount_ids or auto-detect), build the
        // `## Workspace Context` body, and collect all bound paths. These are
        // applied to the seed once it's created and returned to the caller so
        // the gateway/frontend can show a detection badge.
        let (active_mount_ids, workspace_context, mount_paths, mount_tag) =
            self.resolve_mount_workspace(mount_ids, project_ids, user_message);
        let mount_tag_opt = if mount_tag.is_empty() {
            None
        } else {
            Some(mount_tag.clone())
        };

        // RFC-025: suppress project_tag when mount_tag is present — the mount
        // badge is more specific (shows actual mount names) and avoids showing
        // two near-identical badges for the same context.
        let project_tag = if mount_tag_opt.is_some() {
            String::new()
        } else {
            project_tag
        };

        let _conversation_turns = {
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
                    context_parts.push(format!("User: {user_message}"));
                    context_parts.join("\n\n")
                };

                // Record all Q&A as a single exchange for multi-turn history.
                // The formatted `user_message` already contains Q&A context
                // (sent from the frontend as `text` field). Pair it with
                // the full question list as the agent side.
                {
                    let mut sessions = self.sessions.write();
                    if let Some(s) = sessions.get_mut(&session_id) {
                        let all_questions = s.interview.questions.join("\n");
                        s.interview.add_to_history(user_message, &all_questions);
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
                buffer.push_agent(&response_text, None);
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
                active_mount_ids: active_mount_ids.clone(),
                mount_tag: mount_tag_opt.clone(),
                response: response_text,
                seed_id: None,
                agent_id: None,
                phase_reached: Phase::Interview,
                evaluation_passed: None,
                output: None,
                tool_calls: vec![],
                interview_questions: None,
                interview_round: None,
                interview_ambiguity: None,
                mode: "ouroboros".to_string(),
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

                let questions_text = interview.questions.join("\n");

                // If this is the first round (no prior history), record the
                // original user message → agent questions as the first exchange.
                // Without this the multi-turn context loses the user's intent
                // and follow-up rounds can't understand the conversation.
                let is_first_round = session.interview.conversation_history.is_empty();
                if is_first_round {
                    let original = if interview.original_message.is_empty() {
                        user_message.to_string()
                    } else {
                        interview.original_message.clone()
                    };
                    session.interview.add_to_history(&original, &questions_text);
                } else {
                    // Follow-up round: record the user's answer + these questions.
                    let last_answer = session.interview.answers.last().cloned();
                    if let Some(ref ans) = last_answer
                        && !ans.is_empty()
                    {
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

            // Structured questions for the interactive Web UI come from
            // the same LLM call as `interview()` — the engine sanitizes
            // them and synthesizes a free_text fallback when the LLM
            // omitted the structured form. `None` means the frontend
            // falls back to plain markdown rendering of `questions`.
            let structured = interview.structured_questions.clone();

            // Round = completed user/agent exchange pairs in the interview
            // history, minimum 1. Previously this read `answers`, which is
            // never populated by `add_to_history` and left the round stuck
            // at 1 forever.
            let interview_round = {
                let sessions = self.sessions.read();
                sessions
                    .get(&session_id)
                    .map(|s| ((s.interview.conversation_history.len() / 2) as u32).max(1))
                    .unwrap_or(1)
            };

            return Ok(OrchestrationResult {
                session_id: Some(session_id.clone()),
                primary_project_id,
                project_tag: Some(project_tag.clone()),
                active_mount_ids: active_mount_ids.clone(),
                mount_tag: mount_tag_opt.clone(),
                response: format_questions(&questions),
                seed_id: None,
                agent_id: None,
                phase_reached: Phase::Interview,
                evaluation_passed: None,
                output: None,
                tool_calls: vec![],
                interview_questions: structured,
                interview_round: Some(interview_round),
                interview_ambiguity: Some(interview.ambiguity.ambiguity()),
                mode: "ouroboros".to_string(),
            });
        }

        // Record agent response in conversation buffer (for topic shift detection)
        // Note: interview phase returns questions, not a full agent response,
        // but we record it for completeness.
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_agent("[interview: ready]", None);
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

        let mut seed = if is_simple {
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
        seed.project_id = primary_project_id;
        seed.workspace_context = workspace_context.clone();
        seed.mount_paths = mount_paths.clone();

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

                // Record the same duration/success metrics the single-agent
                // path records, so multi-agent orchestration is observable.
                let metrics = get_metrics();
                metrics
                    .orch_duration
                    .observe(orch_start.elapsed().as_secs_f64());
                if all_passed {
                    metrics.agents_completed.inc();
                } else {
                    metrics.agents_failed.inc();
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
                    active_mount_ids: active_mount_ids.clone(),
                    mount_tag: mount_tag_opt.clone(),
                    response: format_result_combined(&combined),
                    seed_id: Some(seed.id),
                    agent_id: None,
                    phase_reached: Phase::Execute,
                    evaluation_passed: Some(all_passed),
                    output: Some(combined),
                    tool_calls: vec![],
                    interview_questions: None,
                    interview_round: None,
                    interview_ambiguity: None,
                    mode: "ouroboros".to_string(),
                });
            }
        }

        // Record agent response in conversation buffer (for multi-agent case)
        {
            let mut buffer = self.conversation_buffer.write();
            buffer.push_agent("[multi-agent: complete]", None);
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
        let (final_result, final_seed, passed, phase_reached) = if let Some(ref schema) =
            seed.output_schema
        {
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
            self.publish_phase_started(&session_id, Phase::Evaluate)
                .await;

            let (result, eval, evolved_seed) = self
                .run_evolution_loop(&session_id, &seed, exec_result)
                .await?;

            // Use a single read of the config so `passed` is consistent
            // with itself (the loop takes its own snapshot internally).
            let passed = {
                let cfg = self.evolution_config.read();
                eval.all_passed() && eval.score >= cfg.score_threshold
            };

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
            buffer.push_agent(&final_seed.goal, None);
        }

        Ok(OrchestrationResult {
            session_id: Some(session_id),
            primary_project_id,
            project_tag: Some(project_tag.clone()),
            active_mount_ids: active_mount_ids.clone(),
            mount_tag: mount_tag_opt.clone(),
            response: format_execution_result(&final_seed, &final_result),
            seed_id: Some(final_seed.id),
            agent_id: None,
            phase_reached,
            evaluation_passed: Some(passed),
            output: Some(final_result.output.clone()),
            tool_calls: final_result.tool_calls.clone(),
            interview_questions: None,
            interview_round: None,
            interview_ambiguity: None,
            mode: "ouroboros".to_string(),
        })
    }

    /// Check whether a seed should go through full evaluate + evolve.
    ///
    /// Only seeds with acceptance criteria and no output_schema qualify.
    /// Simple tasks (from_message, no criteria) get boolean pass/fail.
    fn should_evaluate(&self, seed: &Seed) -> bool {
        !seed.acceptance_criteria.is_empty() && seed.output_schema.is_none()
    }

    /// Default chat mode: execute via AgentRuntime directly.
    ///
    /// Skips interview/seed/evaluate/evolve. Returns fast responses.
    pub async fn chat(
        &self,
        _user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
        project_ids: Option<&str>,
        mount_ids: Option<&str>,
        request_id: &str,
    ) -> Result<OrchestrationResult> {
        tracing::info!(name = "orchestrator.chat", session_id = %session_id.unwrap_or("new"), request_id = %request_id, "starting");
        let metrics = get_metrics();
        metrics.messages.inc();
        let orch_start = std::time::Instant::now();

        let session_id = session_id
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Project detection (same as handle_message)
        let primary_project_id: Option<Uuid> = if let Some(ids_str) = project_ids {
            ids_str
                .split(',')
                .next()
                .and_then(|s| Uuid::parse_str(s.trim()).ok())
        } else {
            self.detect_project_tag(user_message).and_then(|_tag| {
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

        let project_tag = primary_project_id
            .and_then(|id| {
                self.project_manager()
                    .and_then(|pm| pm.get_project(id).map(|p| p.tag()))
            })
            .unwrap_or_default();

        // ── Mount workspace resolution (RFC-025) ──
        let (active_mount_ids, workspace_context, mount_paths, mount_tag) =
            self.resolve_mount_workspace(mount_ids, project_ids, user_message);
        let mount_tag_opt = if mount_tag.is_empty() {
            None
        } else {
            Some(mount_tag.clone())
        };

        // RFC-025: suppress project_tag when mount_tag is present.
        let project_tag = if mount_tag_opt.is_some() {
            String::new()
        } else {
            project_tag
        };

        // Lightweight seed — goal only, no constraints/criteria
        let mut seed = Seed::from_message(user_message);
        seed.project_id = primary_project_id;
        seed.workspace_context = workspace_context;
        seed.mount_paths = mount_paths;

        // Execute via lifecycle manager (fork → run → cleanup)
        tracing::info!(
            phase = "execute",
            mode = "chat",
            "Starting direct execution"
        );
        let exec_result = self
            .lifecycle
            .spawn_and_run(&seed, Priority::Normal)
            .await?;
        self.lifecycle.reap_zombies();

        let metrics = get_metrics();
        metrics
            .orch_duration
            .observe(orch_start.elapsed().as_secs_f64());
        if exec_result.success {
            metrics.agents_completed.inc();
        } else {
            metrics.agents_failed.inc();
        }

        Ok(OrchestrationResult {
            session_id: Some(session_id),
            primary_project_id,
            project_tag: Some(project_tag),
            active_mount_ids: active_mount_ids.clone(),
            mount_tag: mount_tag_opt.clone(),
            response: exec_result.output.clone(),
            seed_id: Some(seed.id),
            agent_id: None,
            phase_reached: Phase::Execute,
            evaluation_passed: None,
            output: Some(exec_result.output),
            tool_calls: exec_result.tool_calls,
            interview_questions: None,
            interview_round: None,
            interview_ambiguity: None,
            mode: "chat".to_string(),
        })
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
        // Snapshot the config under a single read guard so a concurrent
        // `update_evolution_config` call can't split max_iterations and
        // score_threshold across two different config versions (TOCTOU).
        let (max_iterations, threshold) = {
            let cfg = self.evolution_config.read();
            (cfg.max_iterations, cfg.score_threshold)
        };

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
            // Update best if this iteration *strictly* improved. A tie
            // keeps the earlier (first-seen) result so a flat or wobbling
            // score sequence doesn't let later iterations clobber the
            // original.
            if best_eval
                .as_ref()
                .is_none_or(|b| evaluation.score > b.score)
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
                return Ok((
                    best_result,
                    best_eval.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Evolve loop exited with threshold met but no evaluation was produced"
                        )
                    })?,
                    best_seed,
                ));
            }

            // max_iterations == 0 → evaluate only, no evolution.
            if max_iterations == 0 {
                return Ok((
                    best_result,
                    best_eval.ok_or_else(|| {
                        anyhow::anyhow!("No iterations configured and no evaluation was produced")
                    })?,
                    best_seed,
                ));
            }

            // Evolve: produce an improved seed.
            let evolved = self.ouroboros.evolve(&current_seed, &evaluation).await?;
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
                    return Ok((
                        best_result,
                        best_eval.ok_or_else(|| {
                            anyhow::anyhow!(
                                "Evolve returned no seed and no evaluation was produced"
                            )
                        })?,
                        best_seed,
                    ));
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

        self.git_commit(&format!("seeds/{key}.json"), "ourobors: save seed");

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
        let mut task = subtasks.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!("execute_single_subtask called with an empty subtask list")
        })?;
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
            project_id: parent_seed.project_id,
            workspace_context: parent_seed.workspace_context.clone(),
            mount_paths: parent_seed.mount_paths.clone(),
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

        // Preserve subtask_count: a `None` slot means the task panicked or
        // was lost. Previously flatten() dropped them, so `all(success)` on
        // an empty vec returned true — total failure reported as success.
        let completed: Vec<SubTask> = results
            .into_iter()
            .enumerate()
            .map(|(idx, opt)| {
                opt.unwrap_or_else(|| SubTask {
                    id: Uuid::new_v4(),
                    description: format!("subtask {idx} (failed)"),
                    required_capability: None,
                    result: Some("Task panicked or did not complete".into()),
                    success: false,
                    role: AgentRole::default(),
                })
            })
            .collect();
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
            &format!("agent_groups/{group_id}.json"),
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

fn default_chat_mode() -> String {
    "chat".into()
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
    /// Active Mount IDs for this message (RFC-025), primary first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_mount_ids: Vec<MountId>,
    /// Mount decoration tag for the response (e.g. "[🔧 oxios + oxi-sdk]").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_tag: Option<String>,
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
    /// Whether evaluation passed.
    ///
    /// - `None` — evaluation was not applicable (interview, chat, non-task).
    /// - `Some(true)` — evaluation passed.
    /// - `Some(false)` — evaluation failed or execution unsuccessful.
    pub evaluation_passed: Option<bool>,
    /// Output or notes from evaluation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Tool calls recorded during execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<oxios_ouroboros::ToolCallRecord>,
    /// Structured interview questions (chat UI redesign — interactive
    /// interview). Populated when the interview phase needs clarification
    /// and the LLM produced a structured form of the questions. The
    /// Gateway forwards this to the WebSocket as an `interview` chunk;
    /// the Web UI renders it as interactive widgets (chips, yes/no
    /// buttons). When `None`, the frontend falls back to rendering
    /// `response` as plain markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interview_questions:
        Option<Vec<oxios_ouroboros::ouroboros_engine::InterviewQuestionOutput>>,
    /// Current interview round (1-based). Populated alongside
    /// `interview_questions`. Drives the "Round N/M" indicator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interview_round: Option<u32>,
    /// Current ambiguity score (0.0 = clear, 1.0 = fully ambiguous).
    /// Populated alongside `interview_questions`. Drives the progress bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interview_ambiguity: Option<f64>,
    /// Execution mode: "chat" (default agent) | "ouroboros" (spec-first pipeline).
    #[serde(default = "default_chat_mode")]
    pub mode: String,
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
            // Char-boundary safe: roll back to avoid splitting a
            // multibyte UTF-8 sequence (Korean, CJK, emoji).
            let mut end = 500;
            while end > 0 && !exec.output.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &exec.output[..end])
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
        format!("Multi-agent execution completed:\n\n{combined}")
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
        project_id: parent_seed.project_id,
        workspace_context: parent_seed.workspace_context.clone(),
        mount_paths: parent_seed.mount_paths.clone(),
    };
    match lifecycle.spawn_and_run(&child_seed, Priority::Normal).await {
        Ok(result) => (result.output, result.success),
        Err(e) => (format!("Failed: {e}"), false),
    }
}

/// Render the body of the `## Workspace Context` prompt section (RFC-025).
///
/// The caller (`build_system_prompt`) wraps this in the `## Workspace
/// Context` header. Returns `None` when there are no Mounts to describe.
///
/// Fill order respects the prompt budget (~1500 tokens soft):
/// 1. Primary Mount — full (description + summary + path).
/// 2. Secondary Mounts — name + path + one-line summary only.
fn build_workspace_context_body(mounts: &[crate::mount::Mount]) -> Option<String> {
    if mounts.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str("### Active Mounts\n");

    for (i, m) in mounts.iter().enumerate() {
        let primary = i == 0;
        let path = m
            .primary_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "(no path)".to_string());

        if primary {
            out.push_str(&format!("- **{}** → {}\n", m.name, path));
            if !m.auto_description.is_empty() {
                // First ~3 lines of the agent-written description.
                let desc: String = m
                    .auto_description
                    .lines()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n  ");
                out.push_str(&format!("  {}\n", desc));
            }
            let summary = m.summary_line();
            if !summary.is_empty() {
                out.push_str(&format!("  _{}_\n", summary));
            }
            if m.enrichment_pending {
                out.push_str("  _(content changed — consider re-scanning this Mount)_\n");
            }
        } else {
            // Secondary: name + path + one-line summary only.
            let summary = m.summary_line();
            let suffix = if summary.is_empty() {
                String::new()
            } else {
                format!(" — {}", summary)
            };
            out.push_str(&format!("- **{}** → {}{}\n", m.name, path, suffix));
        }
    }

    Some(out)
}

#[cfg(test)]
mod mount_workspace_tests {
    use super::*;
    use crate::mount::{Mount, MountSource};
    use std::path::PathBuf;

    #[test]
    fn test_workspace_context_primary_full_secondary_terse() {
        let mut oxios =
            Mount::from_name_and_path("oxios", PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"));
        oxios.auto_description = "Agent OS.\nRust + tokio.".to_string();
        oxios.auto_meta.summary = "Rust agent OS".to_string();

        let mut oxi = Mount::from_name_and_path("oxi", PathBuf::from("/oxi"));
        oxi.auto_meta.summary = "SDK".to_string();

        let body = build_workspace_context_body(&[oxios, oxi]).unwrap();
        assert!(body.contains("### Active Mounts"));
        // Primary gets full description.
        assert!(body.contains("Agent OS."));
        assert!(body.contains("_Rust agent OS_"));
        // Secondary is terse.
        assert!(body.contains("**oxi** → /oxi — SDK"));
    }

    #[test]
    fn test_workspace_context_empty_is_none() {
        assert!(build_workspace_context_body(&[]).is_none());
    }

    /// End-to-end: a real MountManager + Orchestrator-less call to
    /// `resolve_mount_workspace` proves that detection seeds the primary,
    /// builds the context body, and collects all paths (multi-path access).
    #[test]
    fn test_resolve_mount_workspace_detects_and_collects_paths() {
        use crate::mount::MountManager;
        use oxios_memory::memory::sqlite::MemoryDatabase;
        use std::sync::Arc;

        let db = Arc::new(MemoryDatabase::open_in_memory(64).unwrap());
        let mm = Arc::new(MountManager::new(db, None).unwrap());

        // Register two mounts.
        let oxios = mm
            .create_mount(
                "oxios".to_string(),
                vec![PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios")],
                MountSource::Manual,
            )
            .unwrap();
        let oxi_sdk = mm
            .create_mount(
                "oxi-sdk".to_string(),
                vec![PathBuf::from("/Users/me/oxi")],
                MountSource::Manual,
            )
            .unwrap();
        mm.update_enrichment(oxios.id, Some("Agent OS in Rust.".to_string()), None)
            .unwrap();

        // Build a minimal Orchestrator-free resolver path: replicate what
        // resolve_mount_workspace does, but against the manager directly,
        // since the full Orchestrator needs many subsystems.
        let mounts = mm.get_mounts_ordered(&[oxios.id, oxi_sdk.id]);
        assert_eq!(mounts.len(), 2);

        let body = build_workspace_context_body(&mounts).unwrap();
        assert!(body.contains("oxios"));
        assert!(body.contains("Agent OS in Rust."));
        assert!(body.contains("oxi-sdk"));

        // Collect paths like the orchestrator does.
        let mut paths = Vec::new();
        for m in &mounts {
            for p in &m.paths {
                if !paths.contains(p) {
                    paths.push(p.clone());
                }
            }
        }
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"));
        assert_eq!(paths[1], PathBuf::from("/Users/me/oxi"));
    }

    /// Detection layer 1 (name match) seeds the primary when no explicit
    /// mount_ids are given — the core promise of RFC-025.
    #[test]
    fn test_detection_seeds_primary_on_name_mention() {
        use crate::mount::{DetectionResult, detect_mounts};

        let oxios =
            Mount::from_name_and_path("oxios", PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"));
        let result = detect_mounts("oxios 코드리뷰해줘", std::slice::from_ref(&oxios));
        assert!(matches!(result, DetectionResult::Found(id) if id == oxios.id));
    }
}
