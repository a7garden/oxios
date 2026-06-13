//! Stub telemetry module (no OTel feature).
//!
//! Provides the same public API as the `otel` version but is a no-op.

use anyhow::Result;

/// Telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable OpenTelemetry tracing.
    pub enabled: bool,
    /// OTLP endpoint (e.g., "http://localhost:4317").
    pub endpoint: Option<String>,
    /// Service name for traces.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "oxios".into(),
        }
    }
}

/// Initialize OTel layers — no-op when compiled without the `otel` feature.
pub fn init_telemetry_layers()
-> Result<Vec<Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>>> {
    Ok(vec![])
}
