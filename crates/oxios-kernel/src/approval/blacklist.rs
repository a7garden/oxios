
//! Security blacklist — always-enforced dangerous-command patterns.

use glob::Pattern;
use regex::Regex;
use serde_json::Value;

use super::policy::ToolPolicy;
use super::resolver::GlobalResolver;

/// Argument matcher (lobehub `ArgumentMatcher` analog).
#[derive(Debug, Clone)]
pub enum ArgMatcher {
    /// Prefix match: `"git push --force"`.
    Prefix(String),
    /// Glob: `"sudo *"`, `"mkfs*"`.
    Glob(Pattern),
    /// Regex for precise control.
    Regex(Regex),
}

impl ArgMatcher {
    pub fn new_prefix(p: &str) -> Self {
        Self::Prefix(p.into())
    }
    pub fn new_glob(p: &str) -> Result<Self, glob::PatternError> {
        Ok(Self::Glob(Pattern::new(p)?))
    }
    pub fn new_regex(r: &str) -> Result<Self, regex::Error> {
        Ok(Self::Regex(Regex::new(r)?))
    }

    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::Prefix(p) => value.starts_with(p),
            Self::Glob(p) => p.matches(value),
            Self::Regex(r) => r.is_match(value),
        }
    }
}

/// One blacklist rule: a description plus matchers keyed by argument name.
#[derive(Debug, Clone)]
pub struct BlacklistRule {
    /// Human-readable reason (surfaced to the user on the approval card).
    pub description: String,
    /// Argument key → matcher. Conventionally `{"command": ...}` or `{"binary": ...}`.
    pub matchers: Vec<(String, ArgMatcher)>,
}

/// Security blacklist. Implements `GlobalResolver` so it slots into the
/// approval pipeline's Phase 4 (always-enforced escalation to `Always`).
pub struct SecurityBlacklist {
    /// Rules. Defaults always apply; users extend (not replace) via config.
    pub rules: Vec<BlacklistRule>,
}

impl SecurityBlacklist {
    /// Create a blacklist with the given rules.
    pub fn new(rules: Vec<BlacklistRule>) -> Self {
        Self { rules }
    }

    /// Test a command string against all rules.
    pub fn matches_command(&self, command: &str) -> bool {
        self.rules
            .iter()
            .any(|r| r.matchers.iter().any(|(_, m)| m.matches(command)))
    }

    /// Extract the command/binary from a `Value` args bag and test.
    pub fn matches_args(&self, args: &Value) -> bool {
        let cmd = args
            .get("command")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("binary").and_then(|v| v.as_str()));
        match cmd {
            Some(c) => self.matches_command(c),
            None => false,
        }
    }

    /// Return `Some(Always)` if the args match any rule, else `None`.
    pub fn policy_for(&self, args: &Value) -> Option<ToolPolicy> {
        if self.matches_args(args) {
            Some(ToolPolicy::Always)
        } else {
            None
        }
    }
}

impl GlobalResolver for SecurityBlacklist {
    fn resolve(&self, call: &super::gate::ToolCall<'_>) -> Option<ToolPolicy> {
        self.policy_for(call.args)
    }
}

/// Default always-on rules. Users extend (not replace) via config.
pub fn default_blacklist_rules() -> Vec<BlacklistRule> {
    fn rule(desc: &str, key: &str, matcher: ArgMatcher) -> BlacklistRule {
        BlacklistRule {
            description: desc.into(),
            matchers: vec![(key.into(), matcher)],
        }
    }
    vec![
        rule("rm -rf system", "command", ArgMatcher::new_prefix("rm -rf /")),
        rule("rm -rf home", "command", ArgMatcher::new_prefix("rm -rf ~")),
        rule(
            "sudo escalation",
            "command",
            ArgMatcher::new_glob("sudo *").unwrap(),
        ),
        rule(
            "fork bomb",
            "command",
            ArgMatcher::new_prefix(":(){ :|:& };:"),
        ),
        rule("disk format", "command", ArgMatcher::new_glob("mkfs*").unwrap()),
        rule(
            "raw disk write",
            "command",
            ArgMatcher::new_glob("dd *of=/dev/*").unwrap(),
        ),
        rule(
            "force push",
            "command",
            ArgMatcher::new_prefix("git push --force"),
        ),
        rule(
            "chmod 777 system",
            "command",
            ArgMatcher::new_prefix("chmod -R 777 /"),
        ),
    ]
}

#[cfg(test)]

mod tests {
    use super::*;
    use super::super::gate::ToolCall;
    use serde_json::json;

    #[test]
    fn prefix_matcher_matches_start() {
        assert!(ArgMatcher::new_prefix("rm -rf /").matches("rm -rf /etc"));
        assert!(!ArgMatcher::new_prefix("rm -rf /").matches("ls -la"));
    }

    #[test]
    fn glob_matcher_matches_wildcard() {
        assert!(ArgMatcher::new_glob("sudo *").unwrap().matches("sudo apt install"));
        assert!(!ArgMatcher::new_glob("sudo *").unwrap().matches("apt install"));
    }

    #[test]
    fn default_rules_block_rm_rf_root() {
        let bl = SecurityBlacklist::new(default_blacklist_rules());
        assert!(bl.matches_command("rm -rf /etc"));
    }

    #[test]
    fn default_rules_block_fork_bomb() {
        let bl = SecurityBlacklist::new(default_blacklist_rules());
        assert!(bl.matches_command(":(){ :|:& };:"));
    }

    #[test]
    fn default_rules_allow_curl() {
        let bl = SecurityBlacklist::new(default_blacklist_rules());
        assert!(!bl.matches_command("curl https://wttr.in/Seoul"));
    }

    #[test]
    fn global_resolver_impl_returns_always_on_match() {
        let bl = SecurityBlacklist::new(default_blacklist_rules());
        let args = json!({"command": "rm -rf /etc"});
        let call = ToolCall { tool: "exec", binary: None, args: &args };
        assert_eq!(super::super::resolver::GlobalResolver::resolve(&bl, &call), Some(ToolPolicy::Always));
    }

    #[test]
    fn global_resolver_impl_returns_none_on_safe_command() {
        let bl = SecurityBlacklist::new(default_blacklist_rules());
        let args = json!({"command": "curl https://example.com"});
        let call = ToolCall { tool: "exec", binary: None, args: &args };
        assert_eq!(super::super::resolver::GlobalResolver::resolve(&bl, &call), None);
    }
}
