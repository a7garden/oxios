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

/// Register OpenAI-compatible providers via factory.
///
/// Each provider is registered lazily — credentials are resolved at first use,
/// not at build time. This allows the engine to be built without all
/// credentials available upfront.
fn register_compatible_providers(builder: OxiBuilder, _default_provider: &str) -> OxiBuilder {
    let compatible_providers: &[(&str, &str)] = &[
        ("zai", "https://api.z.ai/api/coding/paas/v4"),
        // Future OpenAI-compatible providers can be added here
    ];

    let mut builder = builder;
    for (name, default_url) in compatible_providers {
        let name_owned = name.to_string();
        let url_owned = default_url.to_string();
        builder = builder.provider_factory(name, move || {
            let api_key =
                crate::credential::CredentialStore::resolve(&name_owned, None).map(|(key, _)| key);
            let base_url = std::env::var(format!("{}_BASE_URL", name_owned.to_uppercase()))
                .unwrap_or_else(|_| url_owned.clone());
            let provider = oxi_ai::OpenAiProvider::with_base_url_and_key(&base_url, api_key);
            tracing::info!(
                "Registered {} provider (OpenAI-compatible, base_url: {})",
                name_owned,
                base_url
            );
            Ok(Arc::new(provider))
        });
    }
    builder
}

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
        let model_id = default_model_id.into();
        let provider_name = model_id
            .split_once('/')
            .map(|(p, _)| p)
            .unwrap_or("anthropic");

        // Workaround: create_builtin_provider("zai") uses OpenAiProvider::with_base_url()
        // without an API key. We register a custom provider with the key attached.
        let mut builder = OxiBuilder::new().with_builtins();

        // Register OpenAI-compatible providers via factory (lazy credential resolution)
        builder = register_compatible_providers(builder, provider_name);

        let oxi = builder.build();
        Self {
            oxi,
            default_model_id: model_id,
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
