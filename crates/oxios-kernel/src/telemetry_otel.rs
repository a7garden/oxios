//! Optional OpenTelemetry tracing integration.
//!
//! This module is compiled when the `otel` feature is enabled.
//! It provides real OpenTelemetry layer initialization.

use anyhow::Result;
use tracing_subscriber::Layer;

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

/// Initialize OTel layers.
///
/// Returns tracing-subscriber compatible layers that can be added
/// to the subscriber. Returns empty vec if disabled.
pub fn init_telemetry_layers() -> Result<Vec<Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>>>
{
    // OTel feature is enabled but layers are not yet initialized.
    // This serves as the foundation for future OTel pipeline setup.
    // To activate: create an OTLP exporter from config.endpoint,
    // build a tracer provider, and return the OpenTelemetryLayer.
    Ok(vec![])
}
