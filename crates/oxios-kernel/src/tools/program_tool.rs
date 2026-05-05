//! Program tool — placeholder for Phase 2.
//!
//! ProgramTool will be implemented in Phase 2 (Program-Tool integration).
//! This placeholder allows compilation while other tools are developed.

use std::sync::Arc;

use async_trait::async_trait;
use oxi_agent::{AgentTool, AgentToolResult, ToolError};
use serde_json::Value;
use tokio::sync::oneshot;

/// Placeholder for ProgramTool (Phase 2).
pub struct ProgramTool {
    _placeholder: (),
}

impl std::fmt::Debug for ProgramTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramTool").finish()
    }
}

#[async_trait]
impl AgentTool for ProgramTool {
    fn name(&self) -> &str {
        "program_placeholder"
    }

    fn label(&self) -> &str {
        "Program Placeholder"
    }

    fn description(&self) -> &'static str {
        "Placeholder — not yet implemented"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        _params: Value,
        _signal: Option<oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, ToolError> {
        Ok(AgentToolResult::error(
            "ProgramTool not yet implemented (Phase 2)",
        ))
    }
}
