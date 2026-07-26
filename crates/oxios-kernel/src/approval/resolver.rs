//! Tool policy resolvers — dynamic (per-call) and global (pipeline-wide).

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::Value;

use super::policy::ToolPolicy;

/// Per-tool dynamic resolver: returns `Some(policy)` to override the declared
/// default based on call arguments, or `None` to keep the declared policy.
/// lobehub `resolveDynamicPolicy` analog.
pub trait ToolPolicyResolver: Send + Sync {
    fn resolve(&self, args: &Value) -> Option<ToolPolicy>;
}

/// Pipeline-wide resolver (Phase 3 of the evaluation). Security blacklist,
/// audit, rate-limit, etc. Return `Some(policy)` to escalate; the gate adopts
/// it via `ToolPolicy::max`, so it can only strengthen — never weaken.
pub trait GlobalResolver: Send + Sync {
    fn resolve(&self, call: &super::gate::ToolCall<'_>) -> Option<ToolPolicy>;
}

/// ExecTool dynamic policy. Preserves current behavior:
/// structured + allowed_commands binary → Auto; shell / unknown → OnDemand.
pub struct ExecPolicyResolver {
    /// Mirror of `ExecConfig.allowed_commands`. Updated when config reloads.
    pub allowed_commands: Arc<RwLock<Vec<String>>>,
}

impl ToolPolicyResolver for ExecPolicyResolver {
    fn resolve(&self, args: &Value) -> Option<ToolPolicy> {
        let mode = args.get("mode")?.as_str()?;
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("binary").and_then(|v| v.as_str()))?;
        let allowed = self.allowed_commands.read();
        match mode {
            "structured" if allowed.iter().any(|c| c == command) => Some(ToolPolicy::Auto),
            _ => Some(ToolPolicy::OnDemand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resolver(allowed: &[&str]) -> ExecPolicyResolver {
        ExecPolicyResolver {
            allowed_commands: std::sync::Arc::new(parking_lot::RwLock::new(
                allowed.iter().map(|s| s.to_string()).collect(),
            )),
        }
    }

    #[test]
    fn structured_allowed_binary_is_auto() {
        let r = resolver(&["curl", "ls"]);
        assert_eq!(
            r.resolve(&json!({"mode": "structured", "binary": "curl"})),
            Some(ToolPolicy::Auto)
        );
    }

    #[test]
    fn structured_unknown_binary_is_ondemand() {
        let r = resolver(&["curl"]);
        assert_eq!(
            r.resolve(&json!({"mode": "structured", "binary": "rm"})),
            Some(ToolPolicy::OnDemand)
        );
    }

    #[test]
    fn shell_mode_is_ondemand() {
        let r = resolver(&["curl"]);
        assert_eq!(
            r.resolve(&json!({"mode": "shell", "command": "ls -la"})),
            Some(ToolPolicy::OnDemand)
        );
    }

    #[test]
    fn missing_mode_returns_none() {
        let r = resolver(&["curl"]);
        assert_eq!(r.resolve(&json!({"binary": "curl"})), None);
    }
}
