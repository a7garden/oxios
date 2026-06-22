//! IntentEngine: LLM-backed implementation of the unified intent protocol.
//!
//! Three methods, one engine:
//! 1. **assess** — classify the message (conversation / clarify / task + scope)
//! 2. **crystallize** — turn substantial tasks into a structured Directive
//! 3. **review** — check the result against acceptance criteria
//!
//! See RFC-027 (`docs/rfc-027-unified-intent-handling.md`) for the full design.

use anyhow::Result;
use futures::StreamExt;
use oxi_sdk::{Context, Message, ProviderEvent, UserMessage};
use parking_lot::Mutex;
use serde::Deserialize;
use std::sync::Arc;

use crate::assessment::{Assessment, Question, QuestionKind, QuestionOption, Scope};
use crate::degraded;
use crate::directive::{Directive, Verdict};
use crate::evaluation::MechanicalEvalResult;
use crate::model_resolver::{ModelResolver, ResolvedModel};
use crate::prompts::{ASSESS_SYSTEM_PROMPT, CRYSTALLIZE_SYSTEM_PROMPT, REVIEW_SYSTEM_PROMPT};
use crate::protocol::ExecutionResult;

// ---------------------------------------------------------------------------
// JSON response shapes
// ---------------------------------------------------------------------------

/// Expected LLM response shape for the assess phase.
#[derive(Debug, Deserialize)]
struct AssessResponse {
    /// "conversation" | "clarify" | "task"
    kind: String,
    /// Conversational reply (when kind=conversation)
    #[serde(default)]
    reply: String,
    /// Clarification questions (when kind=clarify)
    #[serde(default)]
    questions: Vec<AssessQuestion>,
    /// "trivial" | "substantial" (when kind=task)
    #[serde(default)]
    scope: String,
    /// Ambiguity scores (when kind=task)
    #[serde(default)]
    #[allow(dead_code)]
    scores: Option<AmbiguityScores>,
}

/// Question with optional structured options.
#[derive(Debug, Deserialize)]
struct AssessQuestion {
    id: String,
    text: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    options: Vec<AssessOption>,
}

fn default_kind() -> String {
    "free_text".to_string()
}

#[derive(Debug, Deserialize)]
struct AssessOption {
    value: String,
    label: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AmbiguityScores {
    goal_clarity: f64,
    constraint_clarity: f64,
    success_criteria: f64,
}

/// Expected LLM response shape for the crystallize phase.
#[derive(Debug, Deserialize)]
struct CrystallizeResponse {
    goal: String,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    output_schema: Option<serde_json::Value>,
}

/// Expected LLM response shape for the review phase.
#[derive(Debug, Deserialize)]
struct ReviewResponse {
    passed: bool,
    score: f64,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    gaps: Vec<String>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Trait for the intent engine's three core operations (RFC-027).
///
/// Exists so tests can provide a mock implementation without making
/// real LLM calls. Production code uses [`IntentEngine`].
#[async_trait::async_trait]
pub trait IntentEngineOps: Send + Sync {
    /// Classify a user message and decide what should happen next.
    async fn assess(&self, msg: &str, ctx: &crate::directive::MsgCtx) -> Result<Assessment>;

    /// Turn a substantial task into a structured Directive.
    async fn crystallize(&self, msg: &str, ctx: &crate::directive::MsgCtx) -> Result<Directive>;

    /// Check execution output against a Directive's acceptance criteria.
    async fn review(&self, directive: &Directive, result: &ExecutionResult) -> Result<Verdict>;
}

/// LLM-backed intent engine.
///
/// Resolves the live default model from the injected [`ModelResolver`] at
/// the start of every LLM-bound call. This keeps assess/crystallize/review
/// in lockstep with the agent execution phase and with hot-swaps.
pub struct IntentEngine {
    resolver: Arc<dyn ModelResolver>,
    /// Optional lightweight model ID for assess/crystallize/review calls.
    /// When None, uses the resolver's default model.
    lightweight_model: Option<String>,
    /// Optional persona system prompt, prepended to every LLM call.
    persona_prompt: Mutex<Option<String>>,
}

impl IntentEngine {
    /// Create a new engine backed by the given model resolver.
    pub fn new(resolver: Arc<dyn ModelResolver>) -> Self {
        Self {
            resolver,
            lightweight_model: None,
            persona_prompt: Mutex::new(None),
        }
    }

    /// Create a new engine with a lightweight model for intent-handling calls.
    pub fn with_lightweight(
        resolver: Arc<dyn ModelResolver>,
        lightweight_model: Option<String>,
    ) -> Self {
        Self {
            resolver,
            lightweight_model,
            persona_prompt: Mutex::new(None),
        }
    }

    /// Set the persona system prompt (voice customization).
    pub fn set_persona_prompt(&self, prompt: Option<String>) {
        *self.persona_prompt.lock() = prompt;
    }

    /// Resolve the model to use for an intent-handling call.
    ///
    /// When a lightweight model is configured, it is resolved through the
    /// same ModelResolver port that supplies the default model. If the
    /// resolver does not support lightweight model IDs (the current trait
    /// only has resolve_default), we fall back to the default model.
    fn resolve_model(&self) -> Result<ResolvedModel> {
        // TODO: add a `resolve(id)` method to the ModelResolver trait so
        // lightweight_model can override the model. For now, always use
        // the default model — lightweight_model is stored but unused.
        let _ = self.lightweight_model.as_ref();
        self.resolver.resolve_default()
    }
    async fn llm_complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let effective_system = if let Some(ref persona) = *self.persona_prompt.lock() {
            format!("{persona}\n\n{system_prompt}")
        } else {
            system_prompt.to_string()
        };

        let resolved = self.resolve_model()?;

        let mut ctx = Context::new();
        ctx.set_system_prompt(effective_system);
        ctx.add_message(Message::User(UserMessage::new(user_message)));

        let stream = resolved
            .provider
            .stream(&resolved.model, &ctx, None)
            .await?;

        let mut text = String::new();
        tokio::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                ProviderEvent::TextDelta { delta, .. } => text.push_str(&delta),
                ProviderEvent::Done { .. } => break,
                ProviderEvent::Error { error, .. } => {
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
                    &raw[..raw.floor_char_boundary(raw.len().min(500))]
                );
                let retry_raw = self.llm_complete(system_prompt, &retry_msg).await?;
                Self::parse_json::<T>(&retry_raw)
                    .map_err(|e2| anyhow::anyhow!("JSON parse failed after retry: {e2}"))
            }
        }
    }

    /// Parse JSON from LLM output, handling markdown fences and prose wrapping.
    fn parse_json<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T> {
        let trimmed = raw.trim();
        let json_str = if trimmed.starts_with("```") {
            let after_open = trimmed.find('\n').map(|i| i + 1).unwrap_or(0);
            let before_close = trimmed
                .rfind("```")
                .filter(|&i| i >= after_open)
                .unwrap_or(trimmed.len());
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

    // -----------------------------------------------------------------------
    // assess
    // -----------------------------------------------------------------------

    /// Classify a user message and decide what should happen next.
    pub async fn assess(&self, msg: &str, ctx: &crate::directive::MsgCtx) -> Result<Assessment> {
        let user_message = build_assess_prompt(msg, ctx);

        let parsed: AssessResponse = match self
            .llm_json::<AssessResponse>(ASSESS_SYSTEM_PROMPT, &user_message)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "assess JSON parse failed after retry, using degraded fallback");
                return Ok(degraded::degraded_assessment(msg));
            }
        };

        Ok(map_assess_response(parsed, msg))
    }

    // -----------------------------------------------------------------------
    // crystallize
    // -----------------------------------------------------------------------

    /// Turn a substantial task into a structured Directive.
    pub async fn crystallize(
        &self,
        msg: &str,
        ctx: &crate::directive::MsgCtx,
    ) -> Result<Directive> {
        let user_message = build_crystallize_prompt(msg, ctx);

        let parsed: CrystallizeResponse = match self
            .llm_json::<CrystallizeResponse>(CRYSTALLIZE_SYSTEM_PROMPT, &user_message)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "crystallize JSON parse failed after retry, using degraded fallback");
                return Ok(degraded::degraded_directive(msg));
            }
        };

        Ok(Directive {
            goal: parsed.goal,
            original_request: msg.to_string(),
            constraints: parsed.constraints,
            acceptance_criteria: parsed.acceptance_criteria,
            output_schema: parsed.output_schema,
        })
    }

    // -----------------------------------------------------------------------
    // review
    // -----------------------------------------------------------------------

    /// Check execution output against a Directive's acceptance criteria.
    pub async fn review(&self, directive: &Directive, result: &ExecutionResult) -> Result<Verdict> {
        // Stage 1: mechanical (LLM-free) check
        let mechanical =
            MechanicalEvalResult::evaluate(&directive.acceptance_criteria, &result.output);
        let all_mechanical = mechanical.all_passed;

        // Stage 2: semantic (LLM) check
        let user_message = build_review_prompt(directive, result);
        let parsed: ReviewResponse = match self
            .llm_json::<ReviewResponse>(REVIEW_SYSTEM_PROMPT, &user_message)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "review JSON parse failed after retry, using degraded fallback");
                return Ok(degraded::degraded_verdict(all_mechanical));
            }
        };

        // Merge mechanical and semantic — if mechanical failed, force not passed
        let passed = parsed.passed && all_mechanical;
        let mut gaps = parsed.gaps;
        if !all_mechanical {
            for c in &mechanical.criterion_results {
                if !c.passed {
                    gaps.push(format!("{} ({})", c.criterion, c.reason));
                }
            }
        }

        let mut notes = parsed.notes;
        if !all_mechanical {
            for c in &mechanical.criterion_results {
                if c.passed {
                    notes.push(format!("✓ {}", c.criterion));
                } else {
                    notes.push(format!("✗ {}", c.criterion));
                }
            }
        }

        tracing::info!(
            score = parsed.score,
            passed,
            mechanical = all_mechanical,
            notes = notes.len(),
            gaps = gaps.len(),
            "Review complete"
        );

        Ok(Verdict {
            passed,
            score: parsed.score,
            notes,
            gaps,
        })
    }
}

impl std::fmt::Debug for IntentEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntentEngine")
            .field("lightweight_model", &self.lightweight_model)
            .finish()
    }
}

#[async_trait::async_trait]
impl IntentEngineOps for IntentEngine {
    async fn assess(&self, msg: &str, ctx: &crate::directive::MsgCtx) -> Result<Assessment> {
        IntentEngine::assess(self, msg, ctx).await
    }

    async fn crystallize(&self, msg: &str, ctx: &crate::directive::MsgCtx) -> Result<Directive> {
        IntentEngine::crystallize(self, msg, ctx).await
    }

    async fn review(&self, directive: &Directive, result: &ExecutionResult) -> Result<Verdict> {
        IntentEngine::review(self, directive, result).await
    }
}

// ---------------------------------------------------------------------------
// Prompt builders
// ---------------------------------------------------------------------------

fn build_assess_prompt(msg: &str, ctx: &crate::directive::MsgCtx) -> String {
    let mut parts = Vec::new();

    if !ctx.history.is_empty() {
        let history = ctx
            .history
            .iter()
            .map(|e| format!("User: {}\nAgent: {}", e.user, e.agent))
            .collect::<Vec<_>>()
            .join("\n\n");
        parts.push(format!("## Conversation History\n{history}\n"));
    }

    parts.push(format!(
        "## The user just said\n\"{msg}\"\n\n\
         LANGUAGE: Write ALL text output (questions, chat_response, structured question \
         labels and descriptions) in the SAME language as the user's message above.\n\n\
         Analyze this message and produce a JSON object with:\n\
         - \"kind\": \"conversation\" | \"clarify\" | \"task\"\n\
         - \"reply\": (only when kind=conversation) A natural, friendly response in the user's language. Empty array when kind!=conversation.\n\
         - \"questions\": (only when kind=clarify) Up to 3 Socratic clarifying questions in the user's language.\n\
         - \"scope\": (only when kind=task) \"trivial\" for clear single-action requests, \"substantial\" for multi-step tasks.\n\
         - \"scores\": (only when kind=task) {{ \"goal_clarity\": 0.0-1.0, \"constraint_clarity\": 0.0-1.0, \"success_criteria\": 0.0-1.0 }}."
    ));

    parts.join("\n")
}

fn build_crystallize_prompt(msg: &str, ctx: &crate::directive::MsgCtx) -> String {
    let mut parts = Vec::new();

    parts.push(format!("## Original Request\n{msg}"));

    // Include Q&A from clarify history if present
    if !ctx.history.is_empty() {
        let qa = ctx
            .history
            .iter()
            .map(|e| format!("Q: {}\nA: {}", e.user, e.agent))
            .collect::<Vec<_>>()
            .join("\n\n");
        parts.push(format!("## Clarification Q&A\n{qa}"));
    }

    parts.push(
        "\nLANGUAGE: Write the goal and all text fields in the SAME language as the user's original request above.\n\n\
         Generate a Directive specification. Produce a JSON object with:\n\
         - \"goal\": a single clear goal in the user's language\n\
         - \"constraints\": list of constraints\n\
         - \"acceptance_criteria\": list of measurable acceptance criteria\n\
         - \"output_schema\": optional JSON Schema if the task requires structured output"
            .to_string(),
    );

    parts.join("\n")
}

fn build_review_prompt(directive: &Directive, result: &ExecutionResult) -> String {
    let criteria = if directive.acceptance_criteria.is_empty() {
        "(none)".to_string()
    } else {
        directive
            .acceptance_criteria
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "## Directive\n\
         Goal: {}\n\
         Constraints: {}\n\
         Acceptance Criteria:\n{}\n\n\
         ## Execution Output\n{}\n\n\
         Produce a JSON object with:\n\
         - \"passed\": true if all criteria met\n\
         - \"score\": 0.0-1.0\n\
         - \"notes\": human-readable assessment notes\n\
         - \"gaps\": specific failures (for retry context) — empty array when passed",
        directive.goal,
        directive.constraints.join(", "),
        criteria,
        result.output,
    )
}

// ---------------------------------------------------------------------------
// Response mapping
// ---------------------------------------------------------------------------

fn map_assess_response(parsed: AssessResponse, original_msg: &str) -> Assessment {
    match parsed.kind.as_str() {
        "conversation" => {
            let reply = if parsed.reply.is_empty() {
                "Hello! How can I help you today?".to_string()
            } else {
                parsed.reply
            };
            Assessment::Conversation(reply)
        }
        "clarify" => {
            let questions = parsed
                .questions
                .into_iter()
                .map(|q| {
                    let kind = match q.kind.as_str() {
                        "single_choice" => QuestionKind::SingleChoice,
                        "multi_choice" => QuestionKind::MultiChoice,
                        "yes_no" => QuestionKind::YesNo,
                        _ => QuestionKind::FreeText,
                    };
                    let options = q
                        .options
                        .into_iter()
                        .map(|o| QuestionOption {
                            value: o.value,
                            label: o.label,
                        })
                        .collect();
                    Question {
                        id: if q.id.is_empty() {
                            "q1".to_string()
                        } else {
                            q.id
                        },
                        text: q.text,
                        kind,
                        options,
                    }
                })
                .collect();
            Assessment::Clarify { questions }
        }
        "task" => {
            let scope = match parsed.scope.as_str() {
                "trivial" => Scope::Trivial,
                _ => Scope::Substantial,
            };
            Assessment::Task(scope)
        }
        _ => {
            // Unknown kind — treat as conversation
            Assessment::Conversation(format!("I'm not sure how to handle: {original_msg}"))
        }
    }
}
