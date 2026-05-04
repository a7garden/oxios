//! Engine provider abstraction for the kernel → oxi-ai bridge.
//!
//! The [`EngineProvider`] trait formalizes how the kernel obtains LLM
//! providers and models. It abstracts away `oxi_ai::get_provider()` and
//! `oxi_ai::get_model()` so the kernel doesn't directly depend on
//! oxi-ai's factory functions.
//!
//! The default implementation ([`OxiEngineProvider`]) uses oxi-ai's
//! built-in registry. This can be replaced with a mock for testing.

use anyhow::Result;
use oxi_ai::{Model, Provider};
use std::sync::Arc;

/// How the kernel obtains AI providers and models.
///
/// This is the single entry point through which all oxi-ai
/// functionality is accessed. It can be mocked for testing.
pub trait EngineProvider: Send + Sync {
    /// Create a provider for the given provider name.
    fn create_provider(&self, provider_name: &str) -> Result<Arc<dyn Provider>>;

    /// Resolve a "provider/model" string to a [`Model`] struct.
    ///
    /// Accepts both `"provider/model"` and bare `"model"` forms.
    /// When no provider prefix is given, defaults to `"anthropic"`.
    fn resolve_model(&self, model_id: &str) -> Result<Model>;

    /// Get the default model ID (from config or hardcoded).
    fn default_model_id(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default implementation using oxi-ai's built-in registry
// ---------------------------------------------------------------------------

/// Default [`EngineProvider`] that uses oxi-ai's built-in registry.
pub struct OxiEngineProvider {
    default_model_id: String,
}

impl OxiEngineProvider {
    /// Create a new engine provider with the given default model ID.
    pub fn new(default_model_id: impl Into<String>) -> Self {
        Self {
            default_model_id: default_model_id.into(),
        }
    }
}

impl EngineProvider for OxiEngineProvider {
    fn create_provider(&self, provider_name: &str) -> Result<Arc<dyn Provider>> {
        oxi_ai::get_provider(provider_name)
            .map(|p| Arc::from(p) as Arc<dyn Provider>)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", provider_name))
    }

    fn resolve_model(&self, model_id: &str) -> Result<Model> {
        let parts: Vec<&str> = model_id.splitn(2, '/').collect();
        let (provider, model) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("anthropic", parts[0])
        };

        oxi_ai::get_model(provider, model)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_id))
    }

    fn default_model_id(&self) -> &str {
        &self.default_model_id
    }
}

impl std::fmt::Debug for OxiEngineProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxiEngineProvider")
            .field("default_model_id", &self.default_model_id)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_model_with_provider_prefix() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        let model = engine.resolve_model("openai/gpt-4o").unwrap();
        assert_eq!(model.provider, "openai");
        assert_eq!(model.id, "gpt-4o");
    }

    #[test]
    fn test_resolve_model_without_provider_prefix() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        // Without provider prefix, defaults to anthropic. Use an anthropic model.
        let model = engine.resolve_model("claude-sonnet-4-20250514").unwrap();
        assert_eq!(model.provider, "anthropic");
    }

    #[test]
    fn test_default_model_id() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        assert_eq!(engine.default_model_id(), "anthropic/claude-sonnet-4-20250514");
    }

    #[test]
    fn test_resolve_model_not_found() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        let result = engine.resolve_model("nonexistent/model-xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_provider_anthropic() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        let provider = engine.create_provider("anthropic");
        assert!(provider.is_ok());
        assert_eq!(provider.unwrap().name(), "anthropic");
    }

    #[test]
    fn test_create_provider_not_found() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        let result = engine.create_provider("nonexistent_provider");
        assert!(result.is_err());
    }
}
