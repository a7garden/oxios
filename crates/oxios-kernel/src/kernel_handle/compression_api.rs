//! CompressionApi — facade for session context compression.

use std::sync::Arc;

use crate::compression::CompressionService;
use crate::state_store::Session;

/// Facade for session context compression (LLM summaries).
#[derive(Clone)]
pub struct CompressionApi {
    service: Arc<CompressionService>,
}

impl CompressionApi {
    /// Create from a shared CompressionService.
    pub fn new(service: Arc<CompressionService>) -> Self {
        Self { service }
    }

    /// Trigger background compression for a session.
    pub fn spawn_compress(&self, session_id: String) {
        self.service.spawn_compress(session_id)
    }

    /// Check if a session needs compression.
    pub fn should_compress(&self, session: &Session) -> bool {
        self.service.should_compress(session)
    }

    /// Run compression synchronously (for testing / manual trigger).
    pub async fn compress_now(&self, session_id: &str) -> anyhow::Result<()> {
        self.service.compress(session_id).await
    }
}
