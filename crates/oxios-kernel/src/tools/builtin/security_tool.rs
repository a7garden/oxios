//! Security tool — wraps `SecurityApi` audit methods behind the `AgentTool` interface.
//!
//! Provides agents with security audit query capabilities.
//! Actions: verify_chain, query_audit, audit_count.
//!
//! ## Example
//!
//! ```json
//! { "action": "verify_chain" }
//! { "action": "query_audit", "from_seq": 0, "to_seq": 100 }
//! { "action": "audit_count" }
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use oxi_sdk::observability::AuditTrail;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{Value, json};

use crate::kernel_handle::KernelHandle;

/// Agent tool for security audit operations.
///
/// Wraps the audit-related methods of the `SecurityApi` domain. Allows agents
/// to verify audit chain integrity, query audit entries, and check entry count.
///
/// ## Actions
///
/// | Action          | Description                   | Required params | Optional params           |
/// |-----------------|-------------------------------|-----------------|---------------------------|
/// | `verify_chain`  | Verify audit chain integrity  | —               | —                         |
/// | `query_audit`   | Query audit entries by range  | —               | `from_seq`, `to_seq`      |
/// | `audit_count`   | Get total audit entry count   | —               | —                         |
pub struct SecurityTool {
    audit_trail: Arc<AuditTrail>,
}

impl SecurityTool {
    /// Create a new `SecurityTool` from a `KernelHandle`.
    ///
    /// Extracts the `AuditTrail` Arc from the kernel's Security API.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            audit_trail: kernel.security.audit_trail.clone(),
        }
    }
}

impl std::fmt::Debug for SecurityTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityTool").finish()
    }
}

#[async_trait]

impl AgentTool for SecurityTool {
    fn name(&self) -> &str {
        "security"
    }

    fn label(&self) -> &str {
        "Security"
    }

    fn description(&self) -> &'static str {
        "Query security audit trail — verify chain integrity, list entries, check count. \
         Actions: verify_chain, query_audit, audit_count."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["verify_chain", "query_audit", "audit_count"],
                    "description": "Security operation to perform"
                },
                "from_seq": {
                    "type": "integer",
                    "description": "Start sequence number for query_audit (default: 0)"
                },
                "to_seq": {
                    "type": "integer",
                    "description": "End sequence number for query_audit (default: latest)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        match action {
            "verify_chain" => match self.audit_trail.verify() {
                Ok(valid) => Ok(AgentToolResult::success(
                    serde_json::to_string(&json!({
                        "chain_integrity": valid,
                        "status": if valid { "intact" } else { "TAMPERED" },
                    }))
                    .unwrap_or_default(),
                )),
                Err(e) => Ok(AgentToolResult::error(format!(
                    "Chain verification failed: {e:?}"
                ))),
            },

            "query_audit" => {
                let from_seq = params["from_seq"].as_u64().unwrap_or(0);
                let to_seq = params["to_seq"].as_u64().unwrap_or(u64::MAX);

                let entries = self.audit_trail.entries(from_seq, to_seq);
                if entries.is_empty() {
                    return Ok(AgentToolResult::success(
                        "No audit entries found in the specified range.",
                    ));
                }

                let display: Vec<Value> = entries
                    .iter()
                    .map(|entry| {
                        json!({
                            "seq": entry.seq,
                            "actor": entry.actor,
                            "action": format!("{:?}", entry.action),
                            "resource": entry.resource,
                            "timestamp": entry.timestamp,
                        })
                    })
                    .collect();

                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&json!({
                        "entries": display,
                        "count": display.len(),
                    }))
                    .unwrap_or_default(),
                ))
            }

            "audit_count" => {
                let count = self.audit_trail.len();
                Ok(AgentToolResult::success(
                    serde_json::to_string(&json!({ "audit_entry_count": count }))
                        .unwrap_or_default(),
                ))
            }

            other => Err(format!(
                "Unknown security action '{other}'. Valid: verify_chain, query_audit, audit_count"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_structure() {
        let schema = json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["verify_chain", "query_audit", "audit_count"]
                },
                "from_seq": { "type": "integer" },
                "to_seq": { "type": "integer" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().any(|a| a == "verify_chain"));
        assert!(actions.iter().any(|a| a == "query_audit"));
        assert!(actions.iter().any(|a| a == "audit_count"));
    }
}
