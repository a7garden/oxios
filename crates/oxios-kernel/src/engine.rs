//! Engine provider — wraps oxi-sdk's `Oxi` for the kernel.
//!
//! All provider/model resolution goes through `oxi_sdk::OxiBuilder`.
//! The `OxiosEngine` struct wraps the SDK instance and exposes a clean API
//! with support for routing, credentials, provider pooling, and multi-provider fallback.
//!
//! # Architecture
//!
//! ```text
//! OxiosEngine (OxiBuilder → Oxi)
//!   ├── resolve_model("provider/model") → Model
//!   ├── create_provider("anthropic")     → Arc<dyn Provider>
//!   ├── pooled_provider("anthropic")     → Arc<dyn Provider> (rate-limited)
//!   ├── oxi()                            → &Oxi (for AgentBuilder, etc.)
//!   └── agent(AgentConfig)               → AgentBuilder
//! ```

use anyhow::Result;
use oxi_sdk::{Oxi, OxiBuilder, ProviderPool, RateLimitPolicy};
use std::sync::Arc;

use crate::credential::{discover_auth_store_providers, CredentialStore};

/// The kernel's engine — wraps oxi-sdk's Oxi instance.
///
/// Created via [`OxiosEngine::new()`] or [`OxiosEngine::builder()`].
/// Provides access to providers, models, routing, pooling, and agent construction.
///
/// # RFC-014 Phase D
///
/// `authorizer` / `tracer` / `cost_tracker` are optional, engine-level
/// observability and security handles. When set, they are propagated to
/// every agent built via [`OxiosEngine::oxi().agent()`][Oxi::agent] using
/// the new `AgentBuilder::authorizer()` / `.tracer()` / `.cost_tracker()`
/// API. All three are `None` by default, keeping the existing call sites
/// fully backward compatible.
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
    /// Runtime routing control for dynamic model selection.
    routing_control: Option<oxi_sdk::RoutingControl>,
    /// Pooled providers with rate limiting.
    /// Key: provider name (e.g. "anthropic"), Value: ProviderPool wrapper.
    pools: parking_lot::RwLock<std::collections::HashMap<String, Arc<dyn oxi_sdk::Provider>>>,
    /// ── RFC-014 Phase D: engine-level observability/security handles ──
    /// When `Some`, these are attached to every `Agent` built via the
    /// `AgentBuilder` API in `agent_runtime.rs::run_agent()`.
    /// Default: `None` (preserves pre-Phase-D behavior).
    authorizer: Option<Arc<oxi_sdk::Authorizer>>,
    tracer: Option<Arc<oxi_sdk::Tracer>>,
    cost_tracker: Option<Arc<oxi_sdk::CostTracker>>,
}

impl OxiosEngine {
    /// Create a new engine with the given default model.
    ///
    /// Internally calls `OxiBuilder::new().with_builtins()` to load all
    /// built-in models and providers.
    pub fn new(default_model_id: impl Into<String>) -> Self {
        let model_id = default_model_id.into();
        let oxi = OxiBuilder::new().with_builtins().build();
        Self {
            oxi,
            default_model_id: model_id,
            routing_control: None,
            pools: parking_lot::RwLock::new(std::collections::HashMap::new()),
            // RFC-014 Phase D: optional, off by default
            authorizer: None,
            tracer: None,
            cost_tracker: None,
        }
    }

    /// Create a new engine with credentials from config.
    ///
    /// Resolves API keys from CredentialStore for each known provider
    /// and injects them into the OxiBuilder. This enables the engine
    /// to create properly authenticated providers.
    ///
    /// Resolution order (per provider): env var → config.toml → ~/.oxi/auth.json
    pub fn from_config(default_model_id: impl Into<String>, config_api_key: Option<&str>) -> Self {
        let model_id = default_model_id.into();

        // Resolve the primary provider's credential
        let primary_provider = model_id
            .split_once('/')
            .map(|(p, _)| p)
            .unwrap_or("anthropic");

        let mut builder = OxiBuilder::new().with_builtins();

        // Collect all providers that need credential injection:
        // 1. Known major providers (always try to resolve)
        // 2. Any provider found in ~/.oxi/auth.json (discovered dynamically)
        // 3. The primary provider (from the default model)
        let mut providers_to_try: Vec<String> = vec![
            "anthropic".into(),
            "openai".into(),
            "google".into(),
            "deepseek".into(),
            "xai".into(),
            "groq".into(),
            "openrouter".into(),
            "mistral".into(),
            "cerebras".into(),
            "fireworks".into(),
            "github-copilot".into(),
            "huggingface".into(),
            "together".into(),
            "minimax".into(),
            "moonshotai".into(),
            "kimi-coding".into(),
            "zai".into(),
            "opencode".into(),
        ];

        // Discover any additional providers from auth.json that aren't in the
        // known list (e.g. custom/third-party providers).
        if let Ok(extra) = discover_auth_store_providers() {
            for p in extra {
                if !providers_to_try.contains(&p) {
                    providers_to_try.push(p);
                }
            }
        }

        // Ensure the primary provider is always included.
        let primary_owned = primary_provider.to_string();
        if !providers_to_try.contains(&primary_owned) {
            providers_to_try.push(primary_owned);
        }

        for provider in &providers_to_try {
            // Use the config-level key only for the primary provider;
            // other providers resolve from env/auth.json.
            let config_key = if provider == primary_provider {
                config_api_key
            } else {
                None
            };

            if let Some((key, source)) = CredentialStore::resolve(provider, config_key) {
                tracing::debug!(
                    provider,
                    source = ?source,
                    "Injected credential into engine"
                );
                builder = builder.api_key(provider, key);
            }
        }

        let oxi = builder.build();
        Self {
            oxi,
            default_model_id: model_id,
            routing_control: None,
            pools: parking_lot::RwLock::new(std::collections::HashMap::new()),
            // RFC-014 Phase D: optional, off by default
            authorizer: None,
            tracer: None,
            cost_tracker: None,
        }
    }

    /// Create an engine builder for advanced configuration.
    ///
    /// Use this when you need credential injection, routing, or
    /// custom provider registration.
    ///
    /// # RFC-014 Phase D
    ///
    /// The builder also exposes `.with_authorizer()` / `.with_tracer()` /
    /// `.with_cost_tracker()` for attaching engine-level observability
    /// and security handles. All three are `None` by default.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use oxios_kernel::engine::OxiosEngine;
    ///
    /// let engine = OxiosEngine::builder()
    ///     .default_model("anthropic/claude-sonnet-4-20250514")
    ///     .api_key("anthropic", "sk-ant-...")
    ///     .build();
    /// ```
    pub fn builder() -> OxiosEngineBuilder {
        OxiosEngineBuilder {
            inner: OxiBuilder::new().with_builtins(),
            default_model_id: "anthropic/claude-sonnet-4-20250514".to_string(),
            // RFC-014 Phase D: optional, off by default
            authorizer: None,
            tracer: None,
            cost_tracker: None,
        }
    }

    /// Get a reference to the underlying Oxi instance.
    ///
    /// Use this when you need to pass the engine to oxi-sdk APIs directly
    /// (e.g., `AgentBuilder`, `MessageBus`, `AgentGroup`).
    pub fn oxi(&self) -> &Oxi {
        &self.oxi
    }

    /// RFC-014 Phase D: get the engine-level `Authorizer`, if any.
    ///
    /// When `Some`, the authorizer is attached to every `Agent` built via
    /// `Oxi::agent().authorizer(...)` in `agent_runtime.rs::run_agent()`.
    pub fn authorizer(&self) -> Option<&Arc<oxi_sdk::Authorizer>> {
        self.authorizer.as_ref()
    }

    /// RFC-014 Phase D: get the engine-level `Tracer`, if any.
    ///
    /// When `Some`, the tracer is attached to every `Agent` built via
    /// `Oxi::agent().tracer(...)` in `agent_runtime.rs::run_agent()`.
    pub fn tracer(&self) -> Option<&Arc<oxi_sdk::Tracer>> {
        self.tracer.as_ref()
    }

    /// RFC-014 Phase D: get the engine-level `CostTracker`, if any.
    ///
    /// When `Some`, the cost tracker is attached to every `Agent` built via
    /// `Oxi::agent().cost_tracker(...)` in `agent_runtime.rs::run_agent()`.
    pub fn cost_tracker(&self) -> Option<&Arc<oxi_sdk::CostTracker>> {
        self.cost_tracker.as_ref()
    }

    /// Resolve a model ID to a Model.
    pub fn resolve_model(&self, model_id: &str) -> Result<oxi_sdk::Model> {
        self.oxi.resolve_model(model_id)
    }

    /// Create a provider for the given provider name.
    pub fn create_provider(&self, name: &str) -> Result<Arc<dyn oxi_sdk::Provider>> {
        self.oxi.create_provider(name)
    }

    /// Get the default model ID.
    pub fn default_model_id(&self) -> &str {
        &self.default_model_id
    }

    /// Get the routing control, if routing is enabled.
    pub fn routing_control(&self) -> Option<&oxi_sdk::RoutingControl> {
        self.routing_control.as_ref()
    }

    /// Get a rate-limited provider from the pool.
    ///
    /// On first call for a provider name, creates a `ProviderPool` wrapping
    /// the base provider with the given RPM/concurrency limits.
    /// Subsequent calls return the same pooled instance.
    ///
    /// If no rate limit is needed, returns the base provider directly.
    pub fn pooled_provider(&self, name: &str, rpm: u32) -> Result<Arc<dyn oxi_sdk::Provider>> {
        // Check if already pooled.
        {
            let pools = self.pools.read();
            if let Some(pooled) = pools.get(name) {
                return Ok(pooled.clone());
            }
        }

        // Create new pool.
        let base = self.create_provider(name)?;
        let policy = RateLimitPolicy::rpm(rpm);
        let pool = ProviderPool::new(base, policy, name);
        let pooled: Arc<dyn oxi_sdk::Provider> = Arc::new(pool);

        // Cache it.
        {
            let mut pools = self.pools.write();
            pools.insert(name.to_string(), pooled.clone());
        }

        tracing::info!(provider = name, rpm, "Created provider pool");
        Ok(pooled)
    }
}

// ---------------------------------------------------------------------------
// EngineBuilder
// ---------------------------------------------------------------------------

/// Builder for creating an `OxiosEngine` with advanced configuration.
pub struct OxiosEngineBuilder {
    inner: OxiBuilder,
    default_model_id: String,
    // ── RFC-014 Phase D: optional engine-level observability/security handles ──
    // All default to `None` so existing builder chains remain unchanged.
    authorizer: Option<Arc<oxi_sdk::Authorizer>>,
    tracer: Option<Arc<oxi_sdk::Tracer>>,
    cost_tracker: Option<Arc<oxi_sdk::CostTracker>>,
}

impl OxiosEngineBuilder {
    /// Set the default model ID.
    pub fn default_model(mut self, model_id: impl Into<String>) -> Self {
        self.default_model_id = model_id.into();
        self
    }

    /// Register an API key for a specific provider.
    pub fn api_key(self, provider: &str, key: impl Into<String>) -> Self {
        Self {
            inner: self.inner.api_key(provider, key),
            default_model_id: self.default_model_id,
            authorizer: self.authorizer,
            tracer: self.tracer,
            cost_tracker: self.cost_tracker,
        }
    }

    /// Register a full credential (API key + optional base URL).
    pub fn credential(
        self,
        provider: &str,
        api_key: impl Into<String>,
        base_url: Option<&str>,
    ) -> Self {
        Self {
            inner: self.inner.credential(provider, api_key, base_url),
            default_model_id: self.default_model_id,
            authorizer: self.authorizer,
            tracer: self.tracer,
            cost_tracker: self.cost_tracker,
        }
    }

    /// Register a custom provider.
    pub fn provider(self, name: &str, p: impl oxi_sdk::Provider + 'static) -> Self {
        Self {
            inner: self.inner.provider(name, p),
            default_model_id: self.default_model_id,
            authorizer: self.authorizer,
            tracer: self.tracer,
            cost_tracker: self.cost_tracker,
        }
    }

    /// Build the engine.
    pub fn build(self) -> OxiosEngine {
        OxiosEngine {
            oxi: self.inner.build(),
            default_model_id: self.default_model_id,
            routing_control: None,
            pools: parking_lot::RwLock::new(std::collections::HashMap::new()),
            // RFC-014 Phase D: optional, off by default
            authorizer: self.authorizer,
            tracer: self.tracer,
            cost_tracker: self.cost_tracker,
        }
    }

    /// Build the engine with routing enabled.
    ///
    /// Returns `(OxiosEngine, RoutingControl)` for runtime routing control.
    pub fn build_with_routing(self) -> (OxiosEngine, oxi_sdk::RoutingControl) {
        use oxi_sdk::RoutingControl;

        let routing_config = oxi_sdk::routing::RoutingConfig::default();
        let routing_control = RoutingControl::new(routing_config);
        let engine = OxiosEngine {
            oxi: self.inner.build(),
            default_model_id: self.default_model_id,
            routing_control: Some(routing_control.clone()),
            pools: parking_lot::RwLock::new(std::collections::HashMap::new()),
            // RFC-014 Phase D: optional, off by default
            authorizer: self.authorizer,
            tracer: self.tracer,
            cost_tracker: self.cost_tracker,
        };
        (engine, routing_control)
    }

    // ── RFC-014 Phase D: engine-level observability/security handles ──
    //
    // These methods let callers attach shared `Authorizer` / `Tracer` /
    // `CostTracker` instances to the engine. `agent_runtime.rs::run_agent()`
    // reads them via `OxiosEngine::authorizer()` / `.tracer()` /
    // `.cost_tracker()` and propagates them to the new `AgentBuilder` API.
    //
    // Backward compatible: all three are `None` by default.

    /// Attach an `Authorizer` to the engine. Agents built via `Oxi::agent()`
    /// will receive this authorizer through the new `AgentBuilder::authorizer()` API.
    pub fn with_authorizer(mut self, authorizer: Arc<oxi_sdk::Authorizer>) -> Self {
        self.authorizer = Some(authorizer);
        self
    }

    /// Attach a `Tracer` to the engine. Agents built via `Oxi::agent()`
    /// will receive this tracer through the new `AgentBuilder::tracer()` API.
    pub fn with_tracer(mut self, tracer: Arc<oxi_sdk::Tracer>) -> Self {
        self.tracer = Some(tracer);
        self
    }

    /// Attach a `CostTracker` to the engine. Agents built via `Oxi::agent()`
    /// will receive this cost tracker through the new `AgentBuilder::cost_tracker()` API.
    pub fn with_cost_tracker(mut self, cost_tracker: Arc<oxi_sdk::CostTracker>) -> Self {
        self.cost_tracker = Some(cost_tracker);
        self
    }
}

// ---------------------------------------------------------------------------
// EngineProvider trait (for testability and dependency inversion)
// ---------------------------------------------------------------------------

/// Engine provider trait — abstracts how the kernel obtains AI providers.
///
/// Implemented by `OxiosEngine` directly. Use a mock for testing.
pub trait EngineProvider: Send + Sync {
    /// Create a provider for the given provider name.
    fn create_provider(&self, provider_name: &str) -> Result<Arc<dyn oxi_sdk::Provider>>;

    /// Resolve a "provider/model" string to a Model.
    fn resolve_model(&self, model_id: &str) -> Result<oxi_sdk::Model>;

    /// Get the default model ID.
    fn default_model_id(&self) -> &str;
}

impl EngineProvider for OxiosEngine {
    fn create_provider(&self, provider_name: &str) -> Result<Arc<dyn oxi_sdk::Provider>> {
        self.create_provider(provider_name)
    }

    fn resolve_model(&self, model_id: &str) -> Result<oxi_sdk::Model> {
        self.resolve_model(model_id)
    }

    fn default_model_id(&self) -> &str {
        &self.default_model_id
    }
}

impl std::fmt::Debug for OxiosEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxiosEngine")
            .field("default_model_id", &self.default_model_id)
            .field("routing_enabled", &self.routing_control.is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EngineHandle — hot-swappable engine reference
// ---------------------------------------------------------------------------

/// Shared, hot-swappable reference to the active [`OxiosEngine`].
///
/// Wraps `RwLock<Arc<OxiosEngine>>` so that:
/// - **Writers** (`EngineApi`) can atomically replace the engine on config change
/// - **Readers** (`AgentRuntime`) always get the current engine at execution time
///
/// # Cost
///
/// Rebuilding `OxiosEngine` is cheap: `OxiBuilder::new().with_builtins().build()`
/// populates registries from static `model_db` data (~1μs, no I/O, no network).
///
/// # Concurrency
///
/// - `parking_lot::RwLock` is not async-aware, but engine swap only occurs on
///   explicit user action (Web UI / CLI config change) — never in a hot path.
/// - Agent execution reads the engine once at the start of `execute()` and
///   uses the same `Arc<OxiosEngine>` for the entire run (consistent within one execution).
pub struct EngineHandle {
    inner: parking_lot::RwLock<Arc<OxiosEngine>>,
}

impl EngineHandle {
    /// Create a new handle wrapping the given engine.
    pub fn new(engine: Arc<OxiosEngine>) -> Self {
        Self {
            inner: parking_lot::RwLock::new(engine),
        }
    }

    /// Get a snapshot of the current engine.
    ///
    /// The returned `Arc` is stable — it won't change even if another thread
    /// calls `swap()` concurrently.
    pub fn get(&self) -> Arc<OxiosEngine> {
        Arc::clone(&self.inner.read())
    }

    /// Atomically replace the engine with a new one.
    ///
    /// Callers should rebuild `OxiosEngine` with updated credentials/model
    /// before calling this.
    pub fn swap(&self, new_engine: OxiosEngine) {
        let mut guard = self.inner.write();
        let old_id = guard.default_model_id().to_string();
        *guard = Arc::new(new_engine);
        tracing::info!(
            old_model = %old_id,
            new_model = %guard.default_model_id(),
            "Engine hot-swapped"
        );
    }
}

impl std::fmt::Debug for EngineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let engine = self.inner.read();
        f.debug_struct("EngineHandle")
            .field("current_model", &engine.default_model_id())
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
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let model = engine.resolve_model("openai/gpt-4o").unwrap();
        assert_eq!(model.provider, "openai");
        assert_eq!(model.id, "gpt-4o");
    }

    #[test]
    fn test_resolve_model_without_provider_prefix() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let model = engine.resolve_model("claude-sonnet-4-20250514").unwrap();
        assert_eq!(model.provider, "anthropic");
    }

    #[test]
    fn test_default_model_id() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        assert_eq!(
            engine.default_model_id(),
            "anthropic/claude-sonnet-4-20250514"
        );
    }

    #[test]
    fn test_resolve_model_not_found() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let result = engine.resolve_model("nonexistent/model-xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_provider_anthropic() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let provider = engine.create_provider("anthropic");
        assert!(provider.is_ok());
    }

    #[test]
    fn test_create_provider_not_found() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let result = engine.create_provider("nonexistent_provider");
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_credential() {
        let engine = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .credential("openai", "sk-test", None)
            .build();
        assert_eq!(engine.default_model_id(), "openai/gpt-4o");
    }

    #[test]
    fn test_engine_provider_trait_on_engine() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let provider: &dyn EngineProvider = &engine;
        assert!(provider.create_provider("anthropic").is_ok());
        assert!(provider.resolve_model("openai/gpt-4o").is_ok());
    }

    // ── EngineHandle tests ──

    #[test]
    fn test_engine_handle_get_returns_current() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let handle = EngineHandle::new(Arc::new(engine));
        let e = handle.get();
        assert_eq!(e.default_model_id(), "anthropic/claude-sonnet-4-20250514");
    }

    #[test]
    fn test_engine_handle_swap_updates() {
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let handle = EngineHandle::new(Arc::new(engine));

        let new_engine = OxiosEngine::new("openai/gpt-4o");
        handle.swap(new_engine);

        let e = handle.get();
        assert_eq!(e.default_model_id(), "openai/gpt-4o");
    }

    #[test]
    fn test_engine_handle_swap_preserves_old_arc() {
        // An Arc obtained before swap should remain valid.
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        let handle = EngineHandle::new(Arc::new(engine));

        let old = handle.get();
        assert_eq!(old.default_model_id(), "anthropic/claude-sonnet-4-20250514");

        handle.swap(OxiosEngine::new("openai/gpt-4o"));

        // `old` still points to the pre-swap engine.
        assert_eq!(old.default_model_id(), "anthropic/claude-sonnet-4-20250514");

        // New get() returns the swapped engine.
        let current = handle.get();
        assert_eq!(current.default_model_id(), "openai/gpt-4o");
    }

    // ── RFC-014 Phase D: engine-level observability/security handles ──

    #[test]
    fn test_rfc014_phase_d_default_fields_are_none() {
        // Backward compatibility: `OxiosEngine::new()` / `from_config()` /
        // `builder().build()` must all leave the new optional fields as
        // `None` so existing call sites are unaffected.
        let engine = OxiosEngine::new("anthropic/claude-sonnet-4-20250514");
        assert!(engine.authorizer().is_none());
        assert!(engine.tracer().is_none());
        assert!(engine.cost_tracker().is_none());

        let engine = OxiosEngine::from_config("anthropic/claude-sonnet-4-20250514", None);
        assert!(engine.authorizer().is_none());
        assert!(engine.tracer().is_none());
        assert!(engine.cost_tracker().is_none());

        let engine = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .build();
        assert!(engine.authorizer().is_none());
        assert!(engine.tracer().is_none());
        assert!(engine.cost_tracker().is_none());

        let (engine, _rc) = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .build_with_routing();
        assert!(engine.authorizer().is_none());
        assert!(engine.tracer().is_none());
        assert!(engine.cost_tracker().is_none());
    }

    #[test]
    fn test_rfc014_phase_d_with_tracer() {
        // `with_tracer` attaches a `Tracer`; accessor returns `Some`.
        let tracer = Arc::new(oxi_sdk::Tracer::new());
        let engine = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .with_tracer(tracer.clone())
            .build();
        assert!(engine.tracer().is_some());
        assert!(engine.authorizer().is_none());
        assert!(engine.cost_tracker().is_none());
    }

    #[test]
    fn test_rfc014_phase_d_with_cost_tracker() {
        // `with_cost_tracker` attaches a `CostTracker`; accessor returns `Some`.
        // `CostTracker::new` needs an `Arc<ModelRegistry>`; the engine's
        // own registry (via `models_arc`) is fine for construction-only
        // assertions like this one.
        let oxi_for_registry = oxi_sdk::OxiBuilder::new().with_builtins().build();
        let model_registry = oxi_for_registry.models_arc();
        let cost_tracker = Arc::new(oxi_sdk::CostTracker::new(
            model_registry,
            oxi_sdk::CostTrackerConfig::default(),
        ));
        let engine = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .with_cost_tracker(cost_tracker)
            .build();
        assert!(engine.cost_tracker().is_some());
        assert!(engine.authorizer().is_none());
        assert!(engine.tracer().is_none());
    }

    #[test]
    fn test_rfc014_phase_d_with_authorizer() {
        // `with_authorizer` attaches an `Authorizer`; accessor returns `Some`.
        let audit = Arc::new(oxi_sdk::AuditLog::new(16));
        let authorizer = Arc::new(oxi_sdk::Authorizer::new(audit));
        let engine = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .with_authorizer(authorizer)
            .build();
        assert!(engine.authorizer().is_some());
        assert!(engine.tracer().is_none());
        assert!(engine.cost_tracker().is_none());
    }

    #[test]
    fn test_rfc014_phase_d_all_three_handles() {
        // All three handles can be set at once. The build chain must
        // preserve them through `api_key` / `credential` / `provider`
        // builder methods (they should be no-ops for the new fields).
        let audit = Arc::new(oxi_sdk::AuditLog::new(16));
        let authorizer = Arc::new(oxi_sdk::Authorizer::new(audit));
        let tracer = Arc::new(oxi_sdk::Tracer::new());
        let oxi_for_registry = oxi_sdk::OxiBuilder::new().with_builtins().build();
        let model_registry = oxi_for_registry.models_arc();
        let cost_tracker = Arc::new(oxi_sdk::CostTracker::new(
            model_registry,
            oxi_sdk::CostTrackerConfig::default(),
        ));

        let engine = OxiosEngine::builder()
            .default_model("openai/gpt-4o")
            .api_key("openai", "sk-test")
            .with_authorizer(authorizer)
            .with_tracer(tracer)
            .with_cost_tracker(cost_tracker)
            .build();

        assert!(engine.authorizer().is_some());
        assert!(engine.tracer().is_some());
        assert!(engine.cost_tracker().is_some());
        assert_eq!(engine.default_model_id(), "openai/gpt-4o");
    }
}
