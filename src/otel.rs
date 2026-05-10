//! OpenTelemetry tracing configuration.
//!
//! When enabled in config.toml `[otel]`, exports spans via OTLP/gRPC.
//! When disabled (default), this module is a no-op.

use anyhow::Result;
use oxios_kernel::config::OtelConfig;

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
