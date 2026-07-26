//! ApprovalGate — runtime approval evaluation.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use super::policy::{ApprovalConfig, ApprovalMode, ToolPolicy, DEFAULT_TOOL_POLICIES};
use super::resolver::GlobalResolver;

/// Tool call context for approval evaluation.
pub struct ToolCall<'a> {
    /// Tool name ("exec", "read", "web_search" ...).
    pub tool: &'a str,
    /// For exec: the binary ("curl") or "shell".
    pub binary: Option<&'a str>,
    /// Raw call arguments (used by dynamic resolvers / blacklist matching).
    pub args: &'a Value,
}

impl ToolCall<'_> {
    /// Grant key. Hybrid: tool + (exec binary).
    pub fn grant_key(&self) -> String {
        match self.tool {
            "exec" => format!("exec:{}", self.binary.unwrap_or("shell")),
            other => other.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Execute immediately.
    Allow,
    /// Surface an approval card to the user.
    RequireApproval { reason: String },
}

pub struct ApprovalGate {
    tool_policies: HashMap<String, ToolPolicy>,
    config: Arc<parking_lot::RwLock<ApprovalConfig>>,
    global_resolvers: Vec<Box<dyn GlobalResolver>>,
}

impl ApprovalGate {
    pub fn new(tool_policies: HashMap<String, ToolPolicy>, config: ApprovalConfig) -> Self {
        Self::with_global_resolvers(tool_policies, config, Vec::new())
    }
    pub fn with_global_resolvers(tool_policies: HashMap<String, ToolPolicy>, config: ApprovalConfig, global_resolvers: Vec<Box<dyn GlobalResolver>>) -> Self {
        Self { tool_policies, config: Arc::new(parking_lot::RwLock::new(config)), global_resolvers }
    }
    pub fn with_shared_config(tool_policies: HashMap<String, ToolPolicy>, config: Arc<parking_lot::RwLock<ApprovalConfig>>, global_resolvers: Vec<Box<dyn GlobalResolver>>) -> Self {
        Self { tool_policies, config, global_resolvers }
    }

    pub fn evaluate(&self, call: &ToolCall<'_>) -> ApprovalDecision {
        let config = self.config.read();
        let mut policy = self.tool_policies.get(call.tool).copied().unwrap_or(ToolPolicy::OnDemand);
        if let Some(&override_p) = config.tool_overrides.get(call.tool) { policy = override_p; }
        for resolver in &self.global_resolvers { if let Some(p) = resolver.resolve(call) { policy = policy.max(p); } }
        use {ApprovalMode::*, ToolPolicy::*};
        match (config.mode, policy) {
            (_, Auto) => ApprovalDecision::Allow,
            (_, Always) => require(call, "always-policy tool"),
            (AutoRun, OnDemand) => ApprovalDecision::Allow,
            (AllowList, OnDemand) if config.allow_list.iter().any(|k| k == &call.grant_key()) => ApprovalDecision::Allow,
            (_, OnDemand) => require(call, "approval required"),
        }
    }
}

fn require(call: &ToolCall<'_>, why: &str) -> ApprovalDecision {
    ApprovalDecision::RequireApproval {
        reason: format!("{}: {}", call.tool, why),
    }
}

/// Build the default tool_policies map from the const table.
pub fn default_tool_policy_map() -> HashMap<String, ToolPolicy> {
    DEFAULT_TOOL_POLICIES
        .iter()
        .map(|(n, p)| (n.to_string(), *p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::policy::{ApprovalConfig, ApprovalMode::*, ToolPolicy};
    use serde_json::json;
    use std::collections::HashMap;


    fn gate(mode: ApprovalMode, allow_list: &[&str]) -> ApprovalGate {
        let policies = default_tool_policy_map();
        let config = ApprovalConfig {
            mode,
            allow_list: allow_list.iter().map(|s| s.to_string()).collect(),
            tool_overrides: HashMap::new(),
        };
        ApprovalGate::new(policies, config)
    }

    fn call<'a>(tool: &'a str, binary: Option<&'a str>) -> ToolCall<'a> {
        static EMPTY_ARGS: std::sync::LazyLock<serde_json::Value> =
            std::sync::LazyLock::new(|| serde_json::json!({}));
        ToolCall { tool, binary, args: &EMPTY_ARGS }
    }
    // Auto tools: always allow, regardless of mode.
    #[test]
    fn auto_allow_in_manual() {
        assert!(matches!(gate(Manual, &[]).evaluate(&call("read", None)), ApprovalDecision::Allow));
    }
    #[test]
    fn auto_allow_in_allowlist() {
        assert!(matches!(gate(AllowList, &[]).evaluate(&call("read", None)), ApprovalDecision::Allow));
    }
    #[test]
    fn auto_allow_in_autorun() {
        assert!(matches!(gate(AutoRun, &[]).evaluate(&call("read", None)), ApprovalDecision::Allow));
    }

    // OnDemand + AutoRun → Allow
    #[test]
    fn ondemand_autorun_allows() {
        assert!(matches!(gate(AutoRun, &[]).evaluate(&call("exec", Some("curl"))), ApprovalDecision::Allow));
    }

    // OnDemand + AllowList → Allow iff grant
    #[test]
    fn ondemand_allowlist_grant_allows() {
        assert!(matches!(gate(AllowList, &["exec:curl"]).evaluate(&call("exec", Some("curl"))), ApprovalDecision::Allow));
    }
    #[test]
    fn ondemand_allowlist_no_grant_prompts() {
        assert!(matches!(gate(AllowList, &[]).evaluate(&call("exec", Some("curl"))), ApprovalDecision::RequireApproval { .. }));
    }

    // OnDemand + Manual → prompt
    #[test]
    fn ondemand_manual_prompts() {
        assert!(matches!(gate(Manual, &[]).evaluate(&call("exec", Some("curl"))), ApprovalDecision::RequireApproval { .. }));
    }

    // tool_overrides escalate to Always → prompt even in AutoRun
    #[test]
    fn always_override_prompts_in_autorun() {
        let policies = default_tool_policy_map();
        let mut overrides = HashMap::new();
        overrides.insert("exec".to_string(), ToolPolicy::Always);
        let config = ApprovalConfig { mode: AutoRun, allow_list: vec![], tool_overrides: overrides };
        let g = ApprovalGate::new(policies, config);
        assert!(matches!(g.evaluate(&call("exec", Some("curl"))), ApprovalDecision::RequireApproval { .. }));
    }

    // security blacklist escalates to Always even with override to Auto
    #[test]
    fn blacklist_beats_auto_override() {
        let policies = default_tool_policy_map();
        let mut overrides = HashMap::new();
        overrides.insert("exec".to_string(), ToolPolicy::Auto); // user tries to weaken
        let config = ApprovalConfig { mode: AutoRun, allow_list: vec![], tool_overrides: overrides };
        let blacklist = super::super::blacklist::SecurityBlacklist::new(
            super::super::blacklist::default_blacklist_rules(),
        );
        let g = ApprovalGate::with_global_resolvers(policies, config, vec![Box::new(blacklist)]);
        let args = json!({"mode": "shell", "command": "rm -rf /etc"});
        let rm_call = ToolCall { tool: "exec", binary: None, args: &args };
        assert!(matches!(g.evaluate(&rm_call), ApprovalDecision::RequireApproval { .. }));
    }

    #[test]
    fn grant_key_exec_includes_binary() {
        assert_eq!(call("exec", Some("curl")).grant_key(), "exec:curl");
        assert_eq!(call("exec", None).grant_key(), "exec:shell");
        assert_eq!(call("read", None).grant_key(), "read");
    }
    #[test]
    fn shared_config_mutation_takes_effect_live() {
        let shared = Arc::new(parking_lot::RwLock::new(ApprovalConfig {
            mode: ApprovalMode::AllowList,
            allow_list: vec![],
            tool_overrides: HashMap::new(),
        }));
        let gate = ApprovalGate::with_shared_config(default_tool_policy_map(), shared.clone(), vec![]);
        let curl = call("exec", Some("curl"));
        assert!(matches!(gate.evaluate(&curl), ApprovalDecision::RequireApproval { .. }));
        shared.write().allow_list.push("exec:curl".to_string());
        assert!(matches!(gate.evaluate(&curl), ApprovalDecision::Allow));
        shared.write().allow_list.clear();
        shared.write().mode = ApprovalMode::AutoRun;
        assert!(matches!(gate.evaluate(&curl), ApprovalDecision::Allow));
    }
}
