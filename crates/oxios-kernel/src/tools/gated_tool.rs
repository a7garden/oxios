//! Gated tool registry — intercepts all tool executions through AccessGate.
//!
//! Instead of wrapping individual tools (which requires access to tool internals),
//! this module provides a registry-level proxy that checks permissions before
//! delegating to the real tool. This means:
//!
//! - No changes to individual tool code
//! - New tools are automatically protected
//! - oxi-sdk crate tools (ReadTool, WriteTool, etc.) are covered without modification

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::Value;

use crate::access_manager::{
    AccessDenied, AccessGate, AgentContext, CheckRequest, DenyLayer, PathMode,
};

// ─── Path Extraction ────────────────────────────────────────────────────────

/// Tool names that perform file operations and need path checking.
const FILE_TOOLS: &[&str] = &["read", "write", "edit", "ls", "find", "grep"];

/// Extract the target path from tool parameters.
fn extract_path_from_params(tool_name: &str, params: &Value) -> Option<String> {
    if !FILE_TOOLS.contains(&tool_name) {
        return None;
    }

    // Most file tools use "path" parameter
    params
        .get("path")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Determine path access mode from tool name.
fn path_mode_for_tool(tool_name: &str) -> PathMode {
    match tool_name {
        "write" | "edit" => PathMode::Write,
        _ => PathMode::Read,
    }
}

/// Format an access denied error for tool execution.
fn format_denied(denied: &AccessDenied) -> String {
    let layer_tag = match denied.layer {
        DenyLayer::Capability => "[CSpace]",
        DenyLayer::Rbac => "[RBAC]",
        DenyLayer::Permission => "[Permissions]",
        DenyLayer::ExecPolicy => "[ExecPolicy]",
    };
    format!(
        "🔒 Access denied: {} — {} {}",
        denied.reason,
        denied.suggestion.as_deref().unwrap_or(""),
        layer_tag
    )
}

// ─── Gated Tool ─────────────────────────────────────────────────────────────

/// A tool wrapper that checks permissions before execution.
///
/// Wraps any `AgentTool` and performs access control before delegating
/// to the inner tool's `execute` method.
pub struct GatedTool<T: AgentTool> {
    inner: T,
    gate: Arc<AccessGate>,
    context: AgentContext,
}

impl<T: AgentTool> GatedTool<T> {
    /// Create a new gated tool wrapping the given tool.
    pub fn new(inner: T, gate: Arc<AccessGate>, context: AgentContext) -> Self {
        Self {
            inner,
            gate,
            context,
        }
    }
}

impl<T: AgentTool> std::fmt::Debug for GatedTool<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatedTool")
            .field("name", &self.inner.name())
            .finish()
    }
}

#[async_trait]
impl<T: AgentTool + 'static> AgentTool for GatedTool<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn description(&self) -> &'static str {
        "Execute commands and access system resources. Permissions enforced by AccessGate."
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    async fn execute(
        &self,
        tool_call_id: &str,
        params: Value,
        signal: Option<tokio::sync::oneshot::Receiver<()>>,
        ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let tool_name = self.inner.name();

        // Step 1: Check tool access permission
        let check = CheckRequest::Tool {
            context: &self.context,
            tool_name,
        };

        if let Err(denied) = self.gate.check(check) {
            tracing::warn!(
                agent = %denied.agent,
                tool = %tool_name,
                layer = ?denied.layer,
                "GatedTool: tool access denied"
            );
            return Ok(AgentToolResult::error(format_denied(&denied)));
        }

        // Step 2: For file tools, check path access permission
        if let Some(path) = extract_path_from_params(tool_name, &params) {
            let mode = path_mode_for_tool(tool_name);
            let path_check = CheckRequest::Path {
                context: &self.context,
                path: Path::new(&path),
                mode,
            };

            if let Err(denied) = self.gate.check(path_check) {
                tracing::warn!(
                    agent = %denied.agent,
                    path = %path,
                    tool = %tool_name,
                    layer = ?denied.layer,
                    "GatedTool: path access denied"
                );
                return Ok(AgentToolResult::error(format!(
                    "🔒 Path access denied: {}",
                    denied.reason
                )));
            }
        }

        // Step 3: Permission granted — delegate to inner tool
        self.inner.execute(tool_call_id, params, signal, ctx).await
    }
}

/// Wrap a tool with access control.
///
/// Convenience function for creating `GatedTool` instances.
pub fn gate_tool<T: AgentTool + 'static>(
    tool: T,
    gate: Arc<AccessGate>,
    context: AgentContext,
) -> GatedTool<T> {
    GatedTool::new(tool, gate, context)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access_manager::{AccessManager, AgentPermissions, NoOpAuditSink, Role, Subject};
    use crate::config::ExecConfig;
    use oxi_sdk::ReadTool;
    use parking_lot::Mutex;

    fn make_gate_for_test() -> Arc<AccessGate> {
        let mut access = AccessManager::new();
        let perms = AgentPermissions::for_new_agent("test-agent");
        access.set_permissions(perms);

        let subject = Subject::Agent(uuid::Uuid::new_v4());
        access
            .rbac_manager_mut()
            .assign_role(subject, Role::Superuser);

        Arc::new(AccessGate::new(
            Arc::new(Mutex::new(access)),
            Arc::new(ExecConfig::default()),
            Arc::new(NoOpAuditSink),
        ))
    }

    #[test]
    fn test_gated_tool_preserves_name() {
        let gate = make_gate_for_test();
        let ctx = AgentContext::test_fixture("test-agent");
        let tool = GatedTool::new(ReadTool::new(), gate, ctx);
        assert_eq!(tool.name(), "read");
    }

    #[test]
    fn test_extract_path_read_tool() {
        let params = serde_json::json!({"path": "/workspace/file.rs"});
        assert_eq!(
            extract_path_from_params("read", &params),
            Some("/workspace/file.rs".to_string())
        );
    }

    #[test]
    fn test_extract_path_exec_tool() {
        let params = serde_json::json!({"command": "echo hello"});
        assert_eq!(extract_path_from_params("exec", &params), None);
    }

    #[test]
    fn test_path_mode_for_tool() {
        assert_eq!(path_mode_for_tool("write"), PathMode::Write);
        assert_eq!(path_mode_for_tool("edit"), PathMode::Write);
        assert_eq!(path_mode_for_tool("read"), PathMode::Read);
        assert_eq!(path_mode_for_tool("ls"), PathMode::Read);
    }

    #[test]
    fn test_format_denied() {
        let denied = AccessDenied {
            agent: "test".into(),
            resource: "exec".into(),
            layer: DenyLayer::ExecPolicy,
            reason: "not in allowlist".into(),
            suggestion: Some("add to config".into()),
        };
        let s = format_denied(&denied);
        assert!(s.contains("🔒"));
        assert!(s.contains("[ExecPolicy]"));
    }
}
