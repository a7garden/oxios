//! Engine provider — thin wrapper around oxi-sdk's Oxi.
//!
//! oxios uses oxi-sdk as the AI engine. This module re-exports the
//! EngineProvider trait for the kernel so it can be swapped for testing.
//!
//! All provider/model resolution goes through `oxi_sdk::OxiBuilder`.
//! The `OxiosEngine` struct wraps the SDK instance and exposes a clean API.

use anyhow::Result;
use oxi_sdk::{Oxi, OxiBuilder};
use std::sync::Arc;

/// The kernel's engine — wraps oxi-sdk's Oxi instance.
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
}

impl OxiosEngine {
    /// Create a new engine with the given default model.
    ///
    /// Internally calls `OxiBuilder::new().with_builtins()` to load all
    /// 50+ built-in models and providers.
    pub fn new(default_model_id: impl Into<String>) -> Self {
        let oxi = OxiBuilder::new().with_builtins().build();
        Self {
            oxi,
            default_model_id: default_model_id.into(),
        }
    }

    /// Get a reference to the underlying Oxi instance.
    ///
    /// Use this when you need to pass the engine to oxi-sdk APIs directly
    /// (e.g., `AgentBuilder`, `MessageBus`, `AgentGroup`).
    pub fn oxi(&self) -> &Oxi {
        &self.oxi
    }

    /// Resolve a model ID to a Model.
    ///
    /// Accepts both `"provider/model"` and bare `"model"` forms.
    /// When no provider prefix is given, defaults to `"anthropic"`.
    pub fn resolve_model(&self, model_id: &str) -> Result<oxi_sdk::Model> {
        self.oxi.resolve_model(model_id)
    }

    /// Create a provider for the given provider name.
    ///
    /// Checks custom providers first, then falls back to built-in
    /// providers (stateless creation).
    pub fn create_provider(&self, name: &str) -> Result<Arc<dyn oxi_sdk::Provider>> {
        self.oxi.create_provider(name)
    }
}

// ---------------------------------------------------------------------------
// EngineProvider trait (kept for API compatibility)
// ---------------------------------------------------------------------------

/// Engine provider trait — abstracts how the kernel obtains AI providers.
///
/// This trait is implemented by `OxiEngineProvider` and can be replaced
/// with a mock for testing.
pub trait EngineProvider: Send + Sync {
    /// Create a provider for the given provider name.
    fn create_provider(&self, provider_name: &str) -> Result<Arc<dyn oxi_sdk::Provider>>;

    /// Resolve a "provider/model" string to a Model.
    fn resolve_model(&self, model_id: &str) -> Result<oxi_sdk::Model>;

    /// Get the default model ID.
    fn default_model_id(&self) -> &str;
}

/// Default engine provider using oxi-sdk.
pub struct OxiEngineProvider {
    engine: OxiosEngine,
}

impl OxiEngineProvider {
    /// Create a new engine provider with the given default model ID.
    pub fn new(default_model_id: impl Into<String>) -> Self {
        Self {
            engine: OxiosEngine::new(default_model_id),
        }
    }
}

impl EngineProvider for OxiEngineProvider {
    fn create_provider(&self, provider_name: &str) -> Result<Arc<dyn oxi_sdk::Provider>> {
        self.engine.create_provider(provider_name)
    }

    fn resolve_model(&self, model_id: &str) -> Result<oxi_sdk::Model> {
        self.engine.resolve_model(model_id)
    }

    fn default_model_id(&self) -> &str {
        &self.engine.default_model_id
    }
}

impl std::fmt::Debug for OxiEngineProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxiEngineProvider")
            .field("default_model_id", &self.engine.default_model_id)
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
        let model = engine.resolve_model("claude-sonnet-4-20250514").unwrap();
        assert_eq!(model.provider, "anthropic");
    }

    #[test]
    fn test_default_model_id() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        assert_eq!(
            engine.default_model_id(),
            "anthropic/claude-sonnet-4-20250514"
        );
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
    }

    #[test]
    fn test_create_provider_not_found() {
        let engine = OxiEngineProvider::new("anthropic/claude-sonnet-4-20250514");
        let result = engine.create_provider("nonexistent_provider");
        assert!(result.is_err());
    }
}
