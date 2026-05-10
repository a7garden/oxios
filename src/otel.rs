//! OpenTelemetry tracing configuration.
//!
//! When enabled in config.toml `[otel]`, exports spans via OTLP/gRPC.
//! When disabled (default), this module is a no-op.
//!
//! ## Enabling OTel
//! Add to Cargo.toml workspace dependencies:
//! ```toml
//! tracing-opentelemetry = "0.26"
//! opentelemetry = "0.25"
//! opentelemetry-otlp = "0.25"
//! opentelemetry_sdk = { version = "0.25", features = ["rt-tokio"] }
//! ```
//! Then replace the `init_otel` stub below with the real implementation
//! (see git history or ARCHITECTURE.md for the full version).

use anyhow::Result;

/// OTel configuration from config.toml.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// Enable OTLP export (default: false).
    pub enabled: bool,
    /// OTLP endpoint (e.g., "http://localhost:4317" for gRPC).
    pub endpoint: String,
    /// Service name for traces.
    #[allow(dead_code)]
    pub service_name: String,
    /// Sampling ratio (0.0 to 1.0).
    #[allow(dead_code)]
    pub sampling_ratio: f64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".to_string(),
            service_name: "oxios".to_string(),
            sampling_ratio: 1.0,
        }
    }
}

/// Initialize OTel tracing layer.
///
/// Currently a no-op stub. See module documentation for enabling real OTel.
pub async fn init_otel(config: &OtelConfig) -> Result<OtelGuard> {
    if config.enabled {
        tracing::warn!(
            endpoint = %config.endpoint,
            "OTel is enabled in config but OTLP dependencies are not compiled. \
             Add tracing-opentelemetry and opentelemetry-otlp to Cargo.toml to enable."
        );
    }
    Ok(OtelGuard)
}

/// OTel shutdown guard. No-op when OTel is disabled.
pub struct OtelGuard;

impl Drop for OtelGuard {
    fn drop(&mut self) {
        // No-op: OTel dependencies not compiled
    }
}
