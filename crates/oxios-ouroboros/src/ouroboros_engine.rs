//! Ouroboros engine: LLM-backed implementation of the Ouroboros protocol.
//!
//! Uses an oxi-ai Provider to drive the five-phase lifecycle:
//! interview → seed → execute → evaluate → evolve.
//!
//! The interview and generate_seed phases use LLM calls to produce
//! Socratic questions and crystallize answers into structured Seeds.
//! The execute phase is delegated to an external executor (AgentRuntime).
//! The evaluate phase uses three-stage verification.

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use oxi_sdk::{Context, Message, Model, Provider, UserMessage};
use serde::Deserialize;
use std::sync::Arc;

use crate::evaluation::EvaluationResult;
use crate::interview::InterviewResult;
use crate::protocol::{ExecutionResult, OuroborosProtocol, Phase};
use crate::seed::{AmbiguityScore, Entity, Seed};

// ---------------------------------------------------------------------------
// JSON shapes we parse from LLM responses
// ---------------------------------------------------------------------------

/// Expected LLM response shape for the interview phase.
#[derive(Debug, Deserialize)]
struct InterviewResponse {
    /// Whether this message requires task execution (tools, files, etc.)
    /// or is just conversational (greetings, questions, small talk).
    #[serde(default = "default_true")]
    is_task: bool,
    /// Direct conversational response (only used when is_task = false).
    #[serde(default)]
    chat_response: String,
    /// Socratic questions to ask the user (only used when is_task = true).
    #[serde(default)]
    questions: Vec<String>,
    /// Ambiguity scores along each dimension (0.0–1.0 clarity).
    scores: Option<AmbiguityScores>,
}

fn default_true() -> bool {
    true
}

/// Ambiguity sub-scores from the LLM.
#[derive(Debug, Deserialize)]
struct AmbiguityScores {
    goal_clarity: f64,
    constraint_clarity: f64,
    success_criteria: f64,
}

/// Expected LLM response shape for the seed generation phase.
#[derive(Debug, Deserialize)]
struct SeedResponse {
    goal: String,
    constraints: Vec<String>,
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    ontology: Vec<Entity>,
}

/// Expected LLM response shape for the evaluation phase.
#[derive(Debug, Deserialize)]
struct EvaluationResponse {
    mechanical_pass: bool,
    semantic_pass: bool,
    score: f64,
    notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// LLM-powered implementation of the Ouroboros protocol.
///
/// The engine uses the injected `oxi_sdk::Provider` to make LLM calls
/// for interview, seed generation, evaluation, and evolution.
pub struct OuroborosEngine {
    provider: Arc<dyn Provider>,
    model: Model,
    phase: parking_lot::Mutex<Phase>,
    /// Optional persona system prompt, prepended to every LLM call.
    persona_prompt: parking_lot::Mutex<Option<String>>,
    /// Evaluation cache for avoiding redundant LLM calls.
    eval_cache: crate::eval_cache::EvalCache,
}

impl OuroborosEngine {
    /// Create a new engine with the given provider and LLM model.
    pub fn new(provider: Arc<dyn Provider>, model: Model) -> Self {
        Self {
            provider,
            model,
            phase: parking_lot::Mutex::new(Phase::Interview),
            persona_prompt: parking_lot::Mutex::new(None),
            eval_cache: crate::eval_cache::EvalCache::new(256),
        }
    }

    /// Returns the current phase.
    pub fn phase(&self) -> Phase {
        *self.phase.lock()
    }

    /// Set the current phase.
    fn set_phase(&self, phase: Phase) {
        *self.phase.lock() = phase;
    }

    /// Set or clear the persona system prompt.
    #[allow(dead_code)]
    fn set_persona_prompt(&self, prompt: Option<String>) {
        *self.persona_prompt.lock() = prompt;
    }

    /// Run a non-tool LLM completion and return the text content.
    async fn llm_complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        // Prepend persona prompt if set.
        let effective_system = if let Some(ref persona) = *self.persona_prompt.lock() {
            format!("{}\n\n{}", persona, system_prompt)
        } else {
            system_prompt.to_string()
        };

        let mut ctx = Context::new();
        ctx.set_system_prompt(effective_system);
        ctx.add_message(Message::User(UserMessage::new(user_message)));

        let stream = self.provider.stream(&self.model, &ctx, None).await?;

        // Collect the stream into a single text string.
        let mut text = String::new();
        tokio::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                oxi_sdk::ProviderEvent::TextDelta { delta, .. } => {
                    text.push_str(&delta);
                }
                oxi_sdk::ProviderEvent::Done { .. } => break,
                oxi_sdk::ProviderEvent::Error { error, .. } => {
                    // Try to extract text from the error message.
                    let msg_text = error.text_content();
                    if !msg_text.is_empty() {
                        text = msg_text;
                    } else {
                        anyhow::bail!("LLM stream error");
                    }
                    break;
                }
                _ => {}
            }
        }

        Ok(text)
    }

    /// Parse JSON from LLM output, handling markdown fences and prose wrapping.
    fn parse_json<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T> {
        let trimmed = raw.trim();
        let json_str = if trimmed.starts_with("```") {
            let after_open = trimmed.find('\n').map(|i| i + 1).unwrap_or(0);
            let before_close = trimmed.rfind("```").unwrap_or(trimmed.len());
            &trimmed[after_open..before_close]
        } else if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else if let Some(start) = trimmed.find('[') {
            if let Some(end) = trimmed.rfind(']') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        };
        Ok(serde_json::from_str(json_str.trim())?)
    }

    /// Run LLM completion, parse as JSON, retry once on failure.
    async fn llm_json<T: serde::de::DeserializeOwned>(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<T> {
        let raw = self.llm_complete(system_prompt, user_message).await?;
        match Self::parse_json::<T>(&raw) {
            Ok(parsed) => Ok(parsed),
            Err(e) => {
                tracing::warn!(error = %e, "JSON parse failed, retrying with correction");
                let retry_msg = format!(
                    "Your previous response was invalid JSON. The error was: {}\n\n\
                     Your raw output was:\n```\n{}\n```\n\n\
                     Please respond with ONLY valid JSON matching the requested schema. \
                     Do not include any text before or after the JSON object.",
                    e,
                    &raw[..raw.len().min(500)]
                );
                let retry_raw = self.llm_complete(system_prompt, &retry_msg).await?;
                Self::parse_json::<T>(&retry_raw)
                    .map_err(|e2| anyhow::anyhow!("JSON parse failed after retry: {}", e2))
            }
        }
    }
}

#[async_trait]
impl OuroborosProtocol for OuroborosEngine {
    fn set_persona_prompt(&self, prompt: Option<String>) {
        *self.persona_prompt.lock() = prompt;
    }

    async fn interview(&self, user_input: &str) -> Result<InterviewResult> {
        self.set_phase(Phase::Interview);

        let system_prompt = INTERVIEW_SYSTEM_PROMPT;
        let user_message = format!(
            "The user said:\n\"{}\"\n\n\
             Analyze this message and produce a JSON object with:\n\
             - \"is_task\": true if the message requests a concrete action (create, read, write, run, find, fix, analyze, deploy, etc.) or describes something to build/execute. false for greetings, small talk, questions, gratitude, opinions, or conversational messages.\n\
             - \"chat_response\": (only when is_task=false) A natural, friendly response in the user's language. Be warm, concise, and helpful. Skip this field when is_task=true.\n\
             - \"questions\": (only when is_task=true) Up to 3 Socratic clarifying questions. Empty array when is_task=false.\n\
             - \"scores\": (only when is_task=true) {{ \"goal_clarity\": 0.0-1.0, \"constraint_clarity\": 0.0-1.0, \"success_criteria\": 0.0-1.0 }}. Skip this field when is_task=false.\n\n\
             IMPORTANT SCORING (when is_task=true):\n\
             - Score GOAL_CLARITY 0.9+ if the user states ANY concrete action\n\
             - Score CONSTRAINT_CLARITY 0.8+ if the request includes ANY specifics (filename, content, path, language)\n\
             - Score SUCCESS_CRITERIA 0.7+ if you can infer what 'done' looks like\n\
             - Be GENEROUS with clarity scores. When in doubt, score higher.",
            user_input
        );

        let raw = self.llm_complete(system_prompt, &user_message).await?;
        let parsed: InterviewResponse = Self::parse_json(&raw).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to parse interview LLM response, using defaults");
            InterviewResponse {
                is_task: true,
                chat_response: String::new(),
                questions: vec!["Could you describe the goal in more detail?".into()],
                scores: Some(AmbiguityScores {
                    goal_clarity: 0.3,
                    constraint_clarity: 0.2,
                    success_criteria: 0.2,
                }),
            }
        });

        // Non-task message — return direct chat response
        if !parsed.is_task {
            let mut result = InterviewResult::new();
            result.is_task = false;
            result.chat_response = if parsed.chat_response.is_empty() {
                "Hello! How can I help you today?".to_string()
            } else {
                parsed.chat_response
            };
            result.ready_for_seed = false;

            tracing::info!(
                is_task = false,
                "Interview phase complete (chat)"
            );

            return Ok(result);
        }

        // Task message — evaluate ambiguity
        let scores = parsed.scores.unwrap_or(AmbiguityScores {
            goal_clarity: 0.5,
            constraint_clarity: 0.5,
            success_criteria: 0.5,
        });

        let ambiguity = AmbiguityScore::new(
            scores.goal_clarity,
            scores.constraint_clarity,
            scores.success_criteria,
        );

        let ambiguity_value = ambiguity.ambiguity();

        let mut result = InterviewResult::new();
        for q in &parsed.questions {
            result.add_exchange(q, "");
        }
        result.update_ambiguity(ambiguity);

        tracing::info!(
            ambiguity = ambiguity_value,
            ready = result.ready_for_seed,
            questions = parsed.questions.len(),
            "Interview phase complete (task)"
        );

        Ok(result)
    }

    async fn generate_seed(&self, interview: &InterviewResult) -> Result<Seed> {
        self.set_phase(Phase::Seed);

        let qa_block = interview
            .questions
            .iter()
            .zip(interview.answers.iter())
            .map(|(q, a)| format!("Q: {}\nA: {}", q, a))
            .collect::<Vec<_>>()
            .join("\n\n");

        let system_prompt = SEED_SYSTEM_PROMPT;
        let user_message = format!(
            "Based on the following interview, generate a Seed specification.\n\n\
             ## Interview\n\
             {}\n\n\
             Produce a JSON object with:\n\
             - \"goal\": a single clear goal statement\n\
             - \"constraints\": list of constraints\n\
             - \"acceptance_criteria\": list of measurable acceptance criteria\n\
             - \"ontology\": list of {{ \"name\": \"\", \"entity_type\": \"\", \"description\": \"\" }} domain entities",
            qa_block
        );

        let raw = self.llm_complete(system_prompt, &user_message).await?;
        let parsed: SeedResponse = Self::parse_json(&raw).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to parse seed LLM response, using defaults");
            SeedResponse {
                goal: "Task from user input".into(),
                constraints: vec![],
                acceptance_criteria: vec!["Task completes without errors".into()],
                ontology: vec![],
            }
        });

        let seed = Seed {
            id: uuid::Uuid::new_v4(),
            goal: parsed.goal,
            constraints: parsed.constraints,
            acceptance_criteria: parsed.acceptance_criteria,
            ontology: parsed.ontology,
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
        };

        tracing::info!(seed_id = %seed.id, goal = %seed.goal, "Seed generated");
        Ok(seed)
    }

    async fn execute(&self, seed: &Seed) -> Result<ExecutionResult> {
        self.set_phase(Phase::Execute);
        // Execution is delegated to the kernel's AgentRuntime via the Supervisor.
        // The OuroborosEngine itself does not run tools — it orchestrates.
        // The Orchestrator calls Supervisor::run_with_seed() directly.
        // This method exists for protocol completeness but the Orchestrator
        // does not invoke it; it uses the Supervisor instead.
        tracing::info!(seed_id = %seed.id, "Execute phase (delegated to AgentRuntime via Supervisor)");
        Ok(ExecutionResult {
            output: format!("Execution of seed {} delegated to agent runtime", seed.id),
            steps_completed: 0,
            success: false, // Caller should replace with actual result
        })
    }

    async fn evaluate(&self, seed: &Seed, execution: &ExecutionResult) -> Result<EvaluationResult> {
        self.set_phase(Phase::Evaluate);

        // Check cache first
        if let Some(cached) = self.eval_cache.get(seed, execution) {
            tracing::info!(seed_id = %seed.id, "Evaluation cache hit");
            return Ok(cached);
        }

        // Stage 1: Enhanced mechanical evaluation (language-agnostic)
        let mechanical = crate::evaluation::MechanicalEvalResult::evaluate(
            &seed.acceptance_criteria,
            &execution.output,
        );

        // If mechanical passes perfectly, skip LLM eval
        if mechanical.all_passed {
            let result = EvaluationResult {
                mechanical_pass: true,
                semantic_pass: None,
                consensus_pass: None,
                score: 1.0,
                notes: mechanical
                    .criterion_results
                    .iter()
                    .map(|r| format!("✓ {}", r.criterion))
                    .collect(),
            };
            self.eval_cache.put(seed, execution, result.clone());
            tracing::info!(seed_id = %seed.id, score = 1.0, "Mechanical evaluation passed, skipping LLM");
            return Ok(result);
        }

        // Stage 2: Semantic evaluation via LLM (with retry)
        let mechanical_notes: String = mechanical
            .criterion_results
            .iter()
            .map(|r| format!("- {}: {} ({})", r.criterion, r.passed, r.reason))
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = EVALUATE_SYSTEM_PROMPT;
        let user_message = format!(
            "## Goal\n{}\n\n## Acceptance Criteria\n{}\n\n\
             ## Mechanical Check Results\n{}\n\n\
             ## Execution Output (first 3000 chars)\n{}\n\n\
             Evaluate whether the execution output satisfies the goal and acceptance criteria.\n\
             Produce a JSON object:\n\
             - \"mechanical_pass\": {}\n\
             - \"semantic_pass\": true/false\n\
             - \"score\": 0.0 to 1.0\n\
             - \"notes\": list of evaluation notes",
            seed.goal,
            seed.acceptance_criteria
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{}. {}", i + 1, c))
                .collect::<Vec<_>>()
                .join("\n"),
            mechanical_notes,
            &execution.output[..execution.output.len().min(3000)],
            mechanical.all_passed,
        );

        let parsed = match self
            .llm_json::<EvaluationResponse>(system_prompt, &user_message)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "Evaluation JSON parse failed after retry, using mechanical-only");
                EvaluationResponse {
                    mechanical_pass: mechanical.all_passed,
                    semantic_pass: mechanical.all_passed,
                    score: if mechanical.all_passed { 0.7 } else { 0.3 },
                    notes: vec![format!("Evaluation parsing failed: {}", e)],
                }
            }
        };

        let result = EvaluationResult {
            mechanical_pass: parsed.mechanical_pass,
            semantic_pass: Some(parsed.semantic_pass),
            consensus_pass: None,
            score: parsed.score,
            notes: parsed.notes,
        };

        self.eval_cache.put(seed, execution, result.clone());

        tracing::info!(
            seed_id = %seed.id,
            mechanical = result.mechanical_pass,
            semantic = ?result.semantic_pass,
            score = result.score,
            "Evaluation complete"
        );

        Ok(result)
    }

    async fn evolve(&self, seed: &Seed, evaluation: &EvaluationResult) -> Result<Option<Seed>> {
        self.set_phase(Phase::Evolve);

        // If the evaluation passed, no need to evolve.
        if evaluation.all_passed() && evaluation.score >= 0.8 {
            tracing::info!(seed_id = %seed.id, "Evaluation passed, no evolution needed");
            return Ok(None);
        }

        let system_prompt = EVOLVE_SYSTEM_PROMPT;
        let user_message = format!(
            "## Original Seed\n\
             Goal: {}\n\
             Constraints: {}\n\
             Acceptance Criteria: {}\n\n\
             ## Evaluation Result\n\
             Mechanical pass: {}\n\
             Semantic pass: {}\n\
             Score: {}\n\
             Notes: {}\n\n\
             The evaluation did not fully pass. Generate an improved seed that addresses the issues.\n\
             Produce a JSON object:\n\
             - \"goal\": improved goal\n\
             - \"constraints\": updated constraints\n\
             - \"acceptance_criteria\": updated criteria\n\
             - \"ontology\": updated entities",
            seed.goal,
            seed.constraints.join(", "),
            seed.acceptance_criteria.join(", "),
            evaluation.mechanical_pass,
            evaluation
                .semantic_pass
                .map(|p| p.to_string())
                .unwrap_or_else(|| "not evaluated".into()),
            evaluation.score,
            evaluation.notes.join("; "),
        );

        let raw = self.llm_complete(system_prompt, &user_message).await?;
        let parsed: SeedResponse = Self::parse_json(&raw).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to parse evolve LLM response");
            SeedResponse {
                goal: seed.goal.clone(),
                constraints: seed.constraints.clone(),
                acceptance_criteria: seed.acceptance_criteria.clone(),
                ontology: seed.ontology.clone(),
            }
        });

        let evolved = Seed::evolved_from(seed);

        // Override fields with LLM-suggested improvements.
        let evolved = Seed {
            id: evolved.id,
            goal: parsed.goal,
            constraints: parsed.constraints,
            acceptance_criteria: parsed.acceptance_criteria,
            ontology: parsed.ontology,
            created_at: Utc::now(),
            generation: evolved.generation,
            parent_seed_id: evolved.parent_seed_id,
            cspace_hint: evolved.cspace_hint,
        };

        tracing::info!(
            original_seed = %seed.id,
            evolved_seed = %evolved.id,
            "Seed evolved"
        );

        Ok(Some(evolved))
    }
}

// ---------------------------------------------------------------------------
// System prompts
// ---------------------------------------------------------------------------

const INTERVIEW_SYSTEM_PROMPT: &str = "\
You are a Socratic interviewer for an AI agent operating system. \
Your job is to analyze the user's request and determine how ambiguous it is.

You must assess three dimensions:
1. **Goal clarity** — Is it clear what the user wants to achieve?
2. **Constraint clarity** — Are the boundaries and limitations well-defined?
3. **Success criteria clarity** — Is it clear how to know when the task is done?

Ask probing questions that help clarify the user's intent. \
Focus on what the user is assuming but not stating explicitly. \
Be concise — each question should target a specific ambiguity.

Always respond with valid JSON in the exact format requested. \
Do not include any text outside the JSON object.";

const SEED_SYSTEM_PROMPT: &str = "\
You are a Seed Architect for an AI agent operating system. \
Your job is to crystallize interview answers into an immutable specification called a Seed.

A Seed must be:
- **Complete** — Contains everything the agent needs to execute
- **Specific** — No room for misinterpretation
- **Measurable** — Acceptance criteria can be objectively verified

The Seed you produce will be executed by an autonomous agent with tools \
for reading, writing, editing files, and running shell commands.

Always respond with valid JSON in the exact format requested. \
Do not include any text outside the JSON object.";

const EVALUATE_SYSTEM_PROMPT: &str = "\
You are an Evaluator for an AI agent operating system. \
Your job is to verify whether an agent's execution output satisfies the specification.

Evaluate in two stages:
1. **Mechanical** — Does the output literally mention/address each acceptance criterion?
2. **Semantic** — Does the output actually solve the user's intent, not just tick boxes?

Be fair but rigorous. A score of 1.0 means perfect execution, 0.0 means complete failure.

Always respond with valid JSON in the exact format requested. \
Do not include any text outside the JSON object.";

const EVOLVE_SYSTEM_PROMPT: &str = "\
You are a Seed Evolver for an AI agent operating system. \
Your job is to improve a Seed specification based on evaluation feedback.

When evolving:
- Keep what worked, change what didn't
- Add new constraints to prevent known failure modes
- Tighten acceptance criteria if they were too loose
- Broaden acceptance criteria if they were too strict
- Preserve the original intent

The evolved Seed will be re-executed by an autonomous agent.

Always respond with valid JSON in the exact format requested. \
Do not include any text outside the JSON object.";

impl std::fmt::Debug for OuroborosEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OuroborosEngine")
            .field("phase", &self.phase())
            .field("model", &self.model.id)
            .finish()
    }
}
