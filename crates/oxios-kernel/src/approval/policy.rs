//! Tool approval policy types.
//!
//! 3-tier tool policy (lobehub `HumanInterventionPolicy`) crossed with
//! 3-mode user override (lobehub `ApprovalMode`). See
//! `docs/designs/2026-07-27-approval-mode-system-design.md`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Tool-declared approval policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolPolicy {
    /// Auto-execute, no approval. (read, ls, grep, structured allowed binaries)
    #[default]
    Auto,
    /// Approval required — bypassable by auto-run / allow-list grant. (exec, write, web_search)
    OnDemand,
    /// Always require approval — mode and grant cannot bypass. (user-flagged dangerous tools)
    Always,
}

impl ToolPolicy {
    /// Adopt the stronger of two policies (Always > OnDemand > Auto).
    pub fn max(self, other: Self) -> Self {
        use ToolPolicy::*;
        match (self, other) {
            (Always, _) | (_, Always) => Always,
            (OnDemand, _) | (_, OnDemand) => OnDemand,
            _ => Auto,
        }
    }
}

/// User-selected global approval mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
    /// Use each tool's declared policy (safe default).
    #[default]
    Manual,
    /// Only auto-run tools in `allow_list`.
    AllowList,
    /// Auto-run all tools (security-blacklist `Always` still enforced).
    AutoRun,
}

/// Persistent user approval configuration. Lives at `[security.approval]` in config.toml.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalConfig {
    /// Current approval mode.
    #[serde(default)]
    pub mode: ApprovalMode,
    /// Granted tool keys ("read", "exec:curl", "web_search" ...).
    #[serde(default)]
    pub allow_list: Vec<String>,
    /// Per-tool policy overrides (tool name → ToolPolicy).
    #[serde(default)]
    pub tool_overrides: HashMap<String, ToolPolicy>,
}

/// Default declared policy per tool name. Compiled in at registration sites.
pub const DEFAULT_TOOL_POLICIES: &[(&str, ToolPolicy)] = &[
    ("read", ToolPolicy::Auto),
    ("ls", ToolPolicy::Auto),
    ("grep", ToolPolicy::Auto),
    ("find", ToolPolicy::Auto),
    ("get_search_results", ToolPolicy::Auto),
    ("write", ToolPolicy::OnDemand),
    ("edit", ToolPolicy::OnDemand),
    ("exec", ToolPolicy::OnDemand),
    ("web_search", ToolPolicy::OnDemand),
    ("browser", ToolPolicy::OnDemand),
    ("mcp", ToolPolicy::OnDemand),
    ("a2a_delegate", ToolPolicy::OnDemand),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_returns_stronger_policy() {
        assert_eq!(
            ToolPolicy::Auto.max(ToolPolicy::OnDemand),
            ToolPolicy::OnDemand
        );
        assert_eq!(
            ToolPolicy::OnDemand.max(ToolPolicy::Auto),
            ToolPolicy::OnDemand
        );
        assert_eq!(ToolPolicy::Auto.max(ToolPolicy::Always), ToolPolicy::Always);
        assert_eq!(ToolPolicy::Always.max(ToolPolicy::Auto), ToolPolicy::Always);
        assert_eq!(
            ToolPolicy::OnDemand.max(ToolPolicy::Always),
            ToolPolicy::Always
        );
        assert_eq!(ToolPolicy::Auto.max(ToolPolicy::Auto), ToolPolicy::Auto);
    }

    #[test]
    fn default_tool_policies_cover_core_tools() {
        let names: Vec<_> = DEFAULT_TOOL_POLICIES.iter().map(|(n, _)| *n).collect();
        for required in ["read", "write", "edit", "exec", "web_search", "grep", "ls"] {
            assert!(
                names.contains(&required),
                "missing default policy for {required}"
            );
        }
    }

    #[test]
    fn approval_config_defaults_to_manual_empty() {
        let c = ApprovalConfig::default();
        assert_eq!(c.mode, ApprovalMode::Manual);
        assert!(c.allow_list.is_empty());
        assert!(c.tool_overrides.is_empty());
    }

    #[test]
    fn approval_mode_serde_kebab_case() {
        let s = serde_json::to_string(&ApprovalMode::AllowList).unwrap();
        assert_eq!(s, "\"allow-list\"");
        let m: ApprovalMode = serde_json::from_str("\"auto-run\"").unwrap();
        assert_eq!(m, ApprovalMode::AutoRun);
    }
}
