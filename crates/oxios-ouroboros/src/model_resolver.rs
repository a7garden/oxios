//! Model resolution port — the single source of truth for "which model does
//! this task use".
//!
//! Both the Ouroboros phases (interview / seed / evaluate / evolve) and the
//! kernel's `AgentRuntime` (execute) resolve the active model through this
//! port, reading the live engine's default model. This eliminates the
//! divergence where interview used a boot-time-captured model while execute
//! re-resolved a frozen string — the phases now agree by construction, and a
//! bad model ID surfaces at the first phase call rather than silently at
//! execute.
//!
//! # Why a trait here (not the kernel)?
//!
//! `OuroborosEngine` lives in this crate and cannot depend on `oxios-kernel`
//! (the dependency runs the other way: kernel → ouroboros). So the engine
//! holds an `Arc<dyn ModelResolver>` — a port it owns — and the kernel's
//! `EngineHandle` implements it. Classic dependency inversion: the trait
//! belongs to the consumer.

use std::sync::Arc;

use anyhow::Result;
use oxi_sdk::{Model, Provider};

/// A model resolved against the live engine for one LLM-bound operation.
///
/// Cheap to clone (`Arc` inside the provider). Carries the resolved model, a
/// ready-to-use provider, and the canonical `provider/model` id for logging
/// and cost attribution.
#[derive(Clone)]
pub struct ResolvedModel {
    /// The resolved model descriptor.
    pub model: Model,
    /// A provider ready to stream completions for this model.
    pub provider: Arc<dyn Provider>,
    /// Canonical `"provider/model"` string.
    pub model_id: String,
}

/// Port for resolving the engine's current default model.
///
/// Implemented by the kernel's `EngineHandle`. `OuroborosEngine` holds an
/// `Arc<dyn ModelResolver>` and calls [`resolve_default`](Self::resolve_default)
/// at the start of every LLM-bound phase, so each phase reads the live,
/// post-hot-swap model.
pub trait ModelResolver: Send + Sync {
    /// Resolve the engine's live default model + provider.
    ///
    /// Implementations MUST validate the model ID and return an error for
    /// unknown models / unconfigured providers, so callers fail fast.
    fn resolve_default(&self) -> Result<ResolvedModel>;
}

/// A [`ModelResolver`] that always returns the same fixed model.
///
/// Intended for tests and deterministic fixtures. Production code resolves
/// through the kernel's `EngineHandle`, which reads the live default and
/// honors hot-swaps.
pub struct StaticModelResolver {
    model: Model,
    provider: Arc<dyn Provider>,
    model_id: String,
}

impl StaticModelResolver {
    /// Create a resolver that always resolves to `model` + `provider`.
    ///
    /// `model_id` is the canonical `"provider/model"` string used for logging.
    pub fn new(model: Model, provider: Arc<dyn Provider>, model_id: impl Into<String>) -> Self {
        Self {
            model,
            provider,
            model_id: model_id.into(),
        }
    }
}

impl ModelResolver for StaticModelResolver {
    fn resolve_default(&self) -> Result<ResolvedModel> {
        Ok(ResolvedModel {
            model: self.model.clone(),
            provider: Arc::clone(&self.provider),
            model_id: self.model_id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::OuroborosProtocol;

    #[test]
    fn static_resolver_returns_fixed_model() {
        let model = Model::new(
            "zai/glm-5-turbo",
            "GLM-5-Turbo",
            oxi_sdk::Api::OpenAiCompletions,
            "zai",
            "",
        );
        // Provider isn't actually called here; we only verify identity passthrough.
        // Use a cheap stand-in via a new provider-less path is not possible, so we
        // rely on the fact that resolve_default does not invoke the provider.
        let provider: Arc<dyn Provider> = unreachable_provider();
        let resolver = StaticModelResolver::new(model.clone(), provider, "zai/glm-5-turbo");
        let resolved = resolver.resolve_default().expect("static resolve");
        assert_eq!(resolved.model.id, model.id);
        assert_eq!(resolved.model_id, "zai/glm-5-turbo");
    }

    // A provider value that can never actually be called — sufficient for tests
    // that only exercise identity passthrough via StaticModelResolver.
    fn unreachable_provider() -> Arc<dyn Provider> {
        // Build a minimal OpenAI provider against an inert base URL + dummy key.
        // It is never streamed in tests; construction must just succeed.
        Arc::new(oxi_sdk::OpenAiProvider::with_base_url_and_key(
            "https://invalid.invalid/v1",
            Some("unused".to_string()),
        ))
    }
    /// A resolver that always fails — used to verify OuroborosEngine surfaces
    /// model-resolution errors at the FIRST phase (interview), not silently
    /// deferred to execute. This is the regression guard for the user-reported
    /// bug where interview succeeded but execute failed with "Model not found".
    struct FailingResolver;

    impl ModelResolver for FailingResolver {
        fn resolve_default(&self) -> Result<ResolvedModel> {
            anyhow::bail!("model unavailable: zai-coding-plan/glm-5-turbo")
        }
    }

    #[tokio::test]
    async fn interview_fails_fast_when_model_unresolvable() {
        let engine = crate::OuroborosEngine::new(std::sync::Arc::new(FailingResolver));
        let result = engine
            .interview("fetch the top 3 hacker news stories")
            .await;
        assert!(
            result.is_err(),
            "interview must fail when the model can't resolve"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("model unavailable"),
            "expected the resolver error to propagate, got: {msg}"
        );
    }
}
