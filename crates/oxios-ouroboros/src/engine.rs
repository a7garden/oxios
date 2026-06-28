//! IntentEngine: external review for directives that carry acceptance criteria.
//!
//! RFC-033 removed the `assess` and `crystallize` external LLM gates.
//! Every message now streams through the agent loop directly (see
//! `Orchestrator::handle`); the agent's own UNDERSTAND → PLAN → EXECUTE
//! → VERIFY → REPORT protocol replaces them. The only surviving external
//! call is `review`, gated on a Directive that carries acceptance criteria.

use anyhow::Result;
use futures::StreamExt;
use oxi_sdk::{Context, Message, ProviderEvent, UserMessage};
use parking_lot::Mutex;
use serde::Deserialize;
use std::sync::Arc;

use crate::directive::{Directive, Verdict};
use crate::fallback;
use crate::fallback::MechanicalEvalResult;
use crate::model_resolver::{ModelResolver, ResolvedModel};
use crate::prompts::REVIEW_SYSTEM_PROMPT;
use crate::types::ExecutionResult;

// ---------------------------------------------------------------------------
// JSON response shapes
// ---------------------------------------------------------------------------

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

/// External review operation for the intent engine (RFC-033).
///
/// RFC-033 removed the `assess` and `crystallize` gates — every message
/// now streams through the agent loop, so the only remaining external
/// LLM call is `review`, which fires when a Directive carries acceptance
/// criteria (see `Directive::needs_review`). Exists so tests can provide
/// a mock implementation without real LLM calls.
#[async_trait::async_trait]
pub trait IntentEngineOps: Send + Sync {
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
                return Ok(fallback::degraded_verdict(all_mechanical));
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
    async fn review(&self, directive: &Directive, result: &ExecutionResult) -> Result<Verdict> {
        IntentEngine::review(self, directive, result).await
    }
}

// ---------------------------------------------------------------------------
// Prompt builders
// ---------------------------------------------------------------------------

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
