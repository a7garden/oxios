# Approval Mode System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port lobehub's 3-tier tool approval model (Auto/OnDemand/Always × manual/allow-list/auto-run) into Oxios so users pick "how open" via a dropdown instead of being prompted on every exec call.

**Architecture:** New `approval/` module in `oxios-kernel` with `ApprovalGate` that evaluates each tool call after the existing 4-layer gate. Phase pipeline: declared policy → config override → dynamic resolver → global resolvers (security blacklist always wins) → user mode × final policy. The existing `exec_tool.rs` shell-mode approval is removed and absorbed by the gate.

**Tech Stack:** Rust 2024 (kernel), React + TypeScript 5 (web), axum (HTTP API), oxi-sdk tool registry.

## Global Constraints

- **Rust edition 2024, MSRV 1.96.** `#![warn(missing_docs)]` on public crates.
- **anyhow for apps, thiserror for libs.** `oxios-kernel` is a lib → use `thiserror` for new error types.
- **oxi-sdk is crates.io only.** Never reimplement what it provides.
- **Kernel is intentionally monolithic.** Do not propose splitting `oxios-kernel`.
- **Tool registration entry point:** `tools/kernel_bridge.rs::OxiosKernelBridge::register_tools()` delegates to `tools/builtin/mod.rs::register_all_kernel_tools()` and `tools/registration.rs` tier helpers. Policy is attached at these registration sites.
- **Code, comments, commits, docs in English.** Agent conversational replies follow user's language; Web UI is bilingual (ko/en).
- **Commit format:** `<type>(<scope>): <description>` — scopes: `kernel`, `web`, `docs`.
- **Tests:** unit tests in `#[cfg(test)] mod tests`. Verify with `cargo test -p oxios-kernel <name>`.
- **Design doc:** `docs/designs/2026-07-27-approval-mode-system-design.md` (committed). Read it for full rationale before starting.

## File Structure

**New (kernel):**
- `crates/oxios-kernel/src/approval/mod.rs` — module root, re-exports
- `crates/oxios-kernel/src/approval/policy.rs` — `ToolPolicy`, `ApprovalMode`, `ApprovalConfig`, `DEFAULT_TOOL_POLICIES`
- `crates/oxios-kernel/src/approval/resolver.rs` — `ToolPolicyResolver` trait, `GlobalResolver` trait, `ExecPolicyResolver`
- `crates/oxios-kernel/src/approval/blacklist.rs` — `ArgMatcher`, `BlacklistRule`, `SecurityBlacklist`, `DEFAULT_BLACKLIST_RULES`
- `crates/oxios-kernel/src/approval/gate.rs` — `ApprovalGate`, `ToolCall`, `ApprovalDecision`

**New (web):**
- `web/src/components/chat/approval-mode-selector.tsx` — dropdown
- `web/src/hooks/use-approval-config.ts` — config fetch/mutate
- `web/src/types/approval.ts` — TS mirror of kernel types

**Modify (kernel):**
- `crates/oxios-kernel/src/lib.rs` — `pub mod approval;`
- `crates/oxios-kernel/src/config.rs` — `SecurityConfig.approval: ApprovalConfig`
- `crates/oxios-kernel/src/tools/gated_tool.rs` — call `ApprovalGate` after gate pass
- `crates/oxios-kernel/src/tools/exec_tool.rs` — remove shell approval block (lines ~598-622)
- `crates/oxios-kernel/src/tools/builtin/mod.rs` — `register_all_kernel_tools` takes `PolicyMap`
- `crates/oxios-kernel/src/tools/registration.rs` — tier helpers attach policy
- `crates/oxios-kernel/src/event_bus.rs` — `ApprovalRequested.remember_supported: bool`
- `crates/oxios-kernel/src/kernel_handle/security_api.rs` — approval config accessors
- `src/api/` (binary) — `/api/security/approval` routes; `/api/chat/tool-approval/{id}/respond` `remember` param

**Modify (web):**
- `web/src/components/chat/tool-approval-card.tsx` — "remember" checkbox (allow-list mode only)
- `web/src/stores/chat.ts` — `resolveToolApproval(id, approved, remember)`
- `web/src/routes/security.tsx` — approval mode + allow_list management UI
- `web/src/i18n/locales/{en,ko}.json` — new strings
- `web/public/locales/{en,ko}/common.json` — same

**Modify (config):**
- `share/default-config.toml` — `[security.approval]` section

---

# Phase 1: Kernel — Approval Module

## Task 1: `ToolPolicy`, `ApprovalMode`, `ApprovalConfig`, default policy map

**Files:**
- Create: `crates/oxios-kernel/src/approval/mod.rs`
- Create: `crates/oxios-kernel/src/approval/policy.rs`
- Modify: `crates/oxios-kernel/src/lib.rs` (add `pub mod approval;`)

**Interfaces:**
- Produces: `ToolPolicy::{Auto, OnDemand, Always}`, `ToolPolicy::max(self, other) -> Self`, `ApprovalMode::{Manual, AllowList, AutoRun}`, `ApprovalConfig { mode, allow_list, tool_overrides }`, `DEFAULT_TOOL_POLICIES: &[(&str, ToolPolicy)]`

**Consumes:** (none — foundation)

- [ ] **Step 1: Write failing tests for `ToolPolicy::max`**

```rust
// crates/oxios-kernel/src/approval/policy.rs (bottom)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_returns_stronger_policy() {
        assert_eq!(ToolPolicy::Auto.max(ToolPolicy::OnDemand), ToolPolicy::OnDemand);
        assert_eq!(ToolPolicy::OnDemand.max(ToolPolicy::Auto), ToolPolicy::OnDemand);
        assert_eq!(ToolPolicy::Auto.max(ToolPolicy::Always), ToolPolicy::Always);
        assert_eq!(ToolPolicy::Always.max(ToolPolicy::Auto), ToolPolicy::Always);
        assert_eq!(ToolPolicy::OnDemand.max(ToolPolicy::Always), ToolPolicy::Always);
        assert_eq!(ToolPolicy::Auto.max(ToolPolicy::Auto), ToolPolicy::Auto);
    }

    #[test]
    fn default_tool_policies_cover_core_tools() {
        let names: Vec<_> = DEFAULT_TOOL_POLICIES.iter().map(|(n, _)| *n).collect();
        for required in ["read", "write", "edit", "exec", "web_search", "grep", "ls"] {
            assert!(names.contains(&required), "missing default policy for {required}");
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxios-kernel --lib approval::policy 2>&1 | tail -20`
Expected: FAIL — module does not exist / types undefined.

- [ ] **Step 3: Implement `policy.rs`**

```rust
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
```

- [ ] **Step 4: Create `approval/mod.rs` and wire into `lib.rs`**

```rust
//! crates/oxios-kernel/src/approval/mod.rs
//! Tool approval mode system (RFC-035).

pub mod policy;

pub use policy::{ApprovalConfig, ApprovalMode, ToolPolicy, DEFAULT_TOOL_POLICIES};
```

In `crates/oxios-kernel/src/lib.rs`, add near the other `pub mod` declarations:
```rust
pub mod approval;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p oxios-kernel --lib approval::policy 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/oxios-kernel/src/approval/ crates/oxios-kernel/src/lib.rs
git commit -m "feat(kernel): add ToolPolicy and ApprovalConfig (RFC-035)"
```

---

## Task 2: `ToolPolicyResolver` and `GlobalResolver` traits, `ExecPolicyResolver`

**Files:**
- Create: `crates/oxios-kernel/src/approval/resolver.rs`
- Modify: `crates/oxios-kernel/src/approval/mod.rs` (add `pub mod resolver;`)

**Interfaces:**
- Produces:
  - `trait ToolPolicyResolver { fn resolve(&self, args: &serde_json::Value) -> Option<ToolPolicy>; }`
  - `trait GlobalResolver { fn resolve(&self, call: &super::gate::ToolCall) -> Option<ToolPolicy>; }`
  - `struct ExecPolicyResolver { allowed_commands: Arc<parking_lot::RwLock<Vec<String>>> }`

**Consumes:** `ToolPolicy` from Task 1.

- [ ] **Step 1: Write failing tests for `ExecPolicyResolver`**

```rust
// crates/oxios-kernel/src/approval/resolver.rs (bottom)
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxios-kernel --lib approval::resolver 2>&1 | tail -20`
Expected: FAIL — types undefined.

- [ ] **Step 3: Implement `resolver.rs`**

Note: `GlobalResolver` references `super::gate::ToolCall` which is created in Task 4. To avoid a forward dependency, define `GlobalResolver` here using a forward-declared signature. Since `ToolCall` doesn't exist yet, define `GlobalResolver` against a minimal borrowed shape now and adjust the import in Task 4.

```rust
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

/// Pipeline-wide resolver (Phase 4 of the evaluation). Security blacklist,
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
```

- [ ] **Step 4: Wire `resolver` module in `approval/mod.rs`**

```rust
pub mod resolver;
pub use resolver::{ExecPolicyResolver, GlobalResolver, ToolPolicyResolver};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p oxios-kernel --lib approval::resolver 2>&1 | tail -20`
Expected: PASS — 4 tests. (Compilation will fail until Task 4 defines `gate::ToolCall`; that's expected — the tests here only exercise `ExecPolicyResolver`, not `GlobalResolver`. If the forward reference breaks compilation, temporarily gate `GlobalResolver` with `#[allow(dead_code)]` and confirm in Task 4.)

- [ ] **Step 6: Commit**

```bash
git add crates/oxios-kernel/src/approval/
git commit -m "feat(kernel): add ToolPolicyResolver and ExecPolicyResolver (RFC-035)"
```

---

## Task 3: `ArgMatcher`, `BlacklistRule`, `SecurityBlacklist`, defaults

**Files:**
- Create: `crates/oxios-kernel/src/approval/blacklist.rs`
- Modify: `crates/oxios-kernel/src/approval/mod.rs`

**Interfaces:**
- Produces: `enum ArgMatcher { Prefix(String), Glob(glob::Pattern), Regex(regex::Regex) }`, `struct BlacklistRule { description, matchers }`, `struct SecurityBlacklist { rules }` (impls `GlobalResolver`), `fn default_blacklist_rules() -> Vec<BlacklistRule>`

**Consumes:** `GlobalResolver` (Task 2), `ToolCall` (Task 4 — see note).

- [ ] **Step 1: Write failing tests**

```rust
// crates/oxios-kernel/src/approval/blacklist.rs (bottom)
#[cfg(test)]
mod tests {
    use super::*;
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
        // Simulated ToolCall — see Task 4 for the real type; here we test the
        // command-extraction path via a helper.
        let blocked = bl.matches_command("rm -rf /etc");
        assert!(blocked);
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxios-kernel --lib approval::blacklist 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Implement `blacklist.rs`**

```rust
//! Security blacklist — always-enforced dangerous-command patterns.

use glob::Pattern;
use regex::Regex;
use serde_json::Value;

use super::policy::ToolPolicy;

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
    pub fn new_prefix(p: &str) -> Self { Self::Prefix(p.into()) }
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

#[derive(Debug, Clone)]
pub struct BlacklistRule {
    pub description: String,
    /// Argument key → matcher. Conventionally `{"command": ...}` or `{"binary": ...}`.
    pub matchers: Vec<(String, ArgMatcher)>,
}

/// Security blacklist. Impl `GlobalResolver` once `ToolCall` exists (Task 4).
pub struct SecurityBlacklist {
    pub rules: Vec<BlacklistRule>,
}

impl SecurityBlacklist {
    pub fn new(rules: Vec<BlacklistRule>) -> Self { Self { rules } }

    /// Extract command string from args and test against rules.
    pub fn matches_command(&self, command: &str) -> bool {
        self.rules.iter().any(|r| {
            r.matchers.iter().any(|(_, m)| m.matches(command))
        })
    }

    /// Resolve against a `Value` args bag (used by `GlobalResolver` impl in Task 4).
    pub fn matches_args(&self, args: &Value) -> bool {
        let cmd = args
            .get("command").and_then(|v| v.as_str())
            .or_else(|| args.get("binary").and_then(|v| v.as_str()));
        match cmd {
            Some(c) => self.matches_command(c),
            None => false,
        }
    }

    pub fn policy_for(&self, args: &Value) -> Option<ToolPolicy> {
        if self.matches_args(args) { Some(ToolPolicy::Always) } else { None }
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
        rule("rm -rf system",    "command", ArgMatcher::new_prefix("rm -rf /")),
        rule("rm -rf home",      "command", ArgMatcher::new_prefix("rm -rf ~")),
        rule("sudo escalation",  "command", ArgMatcher::new_glob("sudo *").unwrap()),
        rule("fork bomb",        "command", ArgMatcher::new_prefix(":(){ :|:& };:")),
        rule("disk format",      "command", ArgMatcher::new_glob("mkfs*").unwrap()),
        rule("raw disk write",   "command", ArgMatcher::new_glob("dd *of=/dev/*").unwrap()),
        rule("force push",       "command", ArgMatcher::new_prefix("git push --force")),
        rule("chmod 777 system", "command", ArgMatcher::new_prefix("chmod -R 777 /")),
    ]
}
```

- [ ] **Step 4: Wire in `approval/mod.rs`**

```rust
pub mod blacklist;
pub use blacklist::{default_blacklist_rules, ArgMatcher, BlacklistRule, SecurityBlacklist};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p oxios-kernel --lib approval::blacklist 2>&1 | tail -20`
Expected: PASS — 5 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/oxios-kernel/src/approval/
git commit -m "feat(kernel): add SecurityBlacklist and ArgMatcher (RFC-035)"
```

---

## Task 4: `ToolCall`, `ApprovalDecision`, `ApprovalGate`

**Files:**
- Create: `crates/oxios-kernel/src/approval/gate.rs`
- Modify: `crates/oxios-kernel/src/approval/mod.rs`
- Modify: `crates/oxios-kernel/src/approval/blacklist.rs` (add `GlobalResolver` impl)

**Interfaces:**
- Produces:
  - `struct ToolCall<'a> { tool, binary, args }` with `grant_key() -> String`
  - `enum ApprovalDecision { Allow, RequireApproval { reason } }`
  - `struct ApprovalGate { tool_policies, dynamic_resolvers, global_resolvers, config }`
  - `impl ApprovalGate { pub fn evaluate(&self, call: &ToolCall) -> ApprovalDecision }`

**Consumes:** Tasks 1-3.

- [ ] **Step 1: Write failing tests — 9-combination decision table**

```rust
// crates/oxios-kernel/src/approval/gate.rs (bottom)
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::policy::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn gate(mode: ApprovalMode, allow_list: Vec<&str>) -> ApprovalGate {
        let mut policies = HashMap::new();
        for (n, p) in DEFAULT_TOOL_POLICIES { policies.insert(n.to_string(), *p); }
        let config = ApprovalConfig {
            mode,
            allow_list: allow_list.into_iter().map(String::from).collect(),
            tool_overrides: HashMap::new(),
        };
        ApprovalGate::new(policies, config)
    }

    fn call(tool: &str, binary: Option<&str>) -> ToolCall<'_> {
        ToolCall { tool, binary, args: &json!({}) }
    }

    // Auto tools: always allow, regardless of mode.
    #[test] fn auto_allow_in_manual()   { assert!(matches!(gate(Manual, []).evaluate(&call("read", None)), ApprovalDecision::Allow)); }
    #[test] fn auto_allow_in_allowlist(){ assert!(matches!(gate(AllowList, []).evaluate(&call("read", None)), ApprovalDecision::Allow)); }
    #[test] fn auto_allow_in_autorun()  { assert!(matches!(gate(AutoRun, []).evaluate(&call("read", None)), ApprovalDecision::Allow)); }

    // OnDemand + AutoRun → Allow
    #[test] fn ondemand_autorun_allows() { assert!(matches!(gate(AutoRun, []).evaluate(&call("exec", Some("curl"))), ApprovalDecision::Allow)); }

    // OnDemand + AllowList → Allow iff grant
    #[test] fn ondemand_allowlist_grant_allows() { assert!(matches!(gate(AllowList, ["exec:curl"]).evaluate(&call("exec", Some("curl"))), ApprovalDecision::Allow)); }
    #[test] fn ondemand_allowlist_no_grant_prompts() { assert!(matches!(gate(AllowList, []).evaluate(&call("exec", Some("curl"))), ApprovalDecision::RequireApproval { .. })); }

    // OnDemand + Manual → prompt
    #[test] fn ondemand_manual_prompts() { assert!(matches!(gate(Manual, []).evaluate(&call("exec", Some("curl"))), ApprovalDecision::RequireApproval { .. })); }

    // tool_overrides escalate to Always → prompt even in AutoRun
    #[test] fn always_override_prompts_in_autorun() {
        let mut policies = HashMap::new();
        for (n, p) in DEFAULT_TOOL_POLICIES { policies.insert(n.to_string(), *p); }
        let mut overrides = HashMap::new();
        overrides.insert("exec".to_string(), ToolPolicy::Always);
        let config = ApprovalConfig { mode: AutoRun, allow_list: vec![], tool_overrides: overrides };
        let g = ApprovalGate::new(policies, config);
        assert!(matches!(g.evaluate(&call("exec", Some("curl"))), ApprovalDecision::RequireApproval { .. }));
    }

    // security blacklist escalates to Always even with override to Auto
    #[test] fn blacklist_beats_auto_override() {
        let mut policies = HashMap::new();
        for (n, p) in DEFAULT_TOOL_POLICIES { policies.insert(n.to_string(), *p); }
        let mut overrides = HashMap::new();
        overrides.insert("exec".to_string(), ToolPolicy::Auto);  // user tries to weaken
        let config = ApprovalConfig { mode: AutoRun, allow_list: vec![], tool_overrides: overrides };
        let blacklist = super::super::blacklist::SecurityBlacklist::new(
            super::super::blacklist::default_blacklist_rules()
        );
        let g = ApprovalGate::with_global_resolvers(policies, config, vec![Box::new(blacklist)]);
        let args = json!({"mode": "shell", "command": "rm -rf /etc"});
        let rm_call = ToolCall { tool: "exec", binary: None, args: &args };
        assert!(matches!(g.evaluate(&rm_call), ApprovalDecision::RequireApproval { .. }));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxios-kernel --lib approval::gate 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Implement `gate.rs`**

```rust
//! ApprovalGate — runtime approval evaluation.

use std::collections::HashMap;

use serde_json::Value;

use super::policy::{ApprovalConfig, ApprovalMode, ToolPolicy, DEFAULT_TOOL_POLICIES};
use super::resolver::GlobalResolver;

/// Tool call context for approval evaluation.
pub struct ToolCall<'a> {
    pub tool: &'a str,
    /// For exec: the binary ("curl") or "shell".
    pub binary: Option<&'a str>,
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
    Allow,
    RequireApproval { reason: String },
}

pub struct ApprovalGate {
    tool_policies: HashMap<String, ToolPolicy>,
    config: ApprovalConfig,
    global_resolvers: Vec<Box<dyn GlobalResolver>>,
}

impl ApprovalGate {
    pub fn new(tool_policies: HashMap<String, ToolPolicy>, config: ApprovalConfig) -> Self {
        Self { tool_policies, config, global_resolvers: Vec::new() }
    }

    pub fn with_global_resolvers(
        tool_policies: HashMap<String, ToolPolicy>,
        config: ApprovalConfig,
        global_resolvers: Vec<Box<dyn GlobalResolver>>,
    ) -> Self {
        Self { tool_policies, config, global_resolvers }
    }

    pub fn config(&self) -> &ApprovalConfig { &self.config }

    pub fn evaluate(&self, call: &ToolCall<'_>) -> ApprovalDecision {
        // Phase 1: declared policy (DEFAULT_TOOL_POLICIES via tool_policies map)
        let mut policy = self.tool_policies.get(call.tool).copied()
            .unwrap_or(ToolPolicy::OnDemand);

        // Phase 2: config tool_overrides — user override replaces declared
        if let Some(&override_p) = self.config.tool_overrides.get(call.tool) {
            policy = override_p;
        }

        // Phase 3: global resolvers — max-merge, can only escalate.
        // (Dynamic per-tool resolvers would be Phase 3 as well; they're attached
        // by the tool registration layer, see Task 7. The gate receives their
        // result via tool_policies being pre-resolved, OR via a future field.)
        for resolver in &self.global_resolvers {
            if let Some(p) = resolver.resolve(call) {
                policy = policy.max(p);
            }
        }

        // Phase 4: user mode × final policy
        use (ApprovalMode::*, ToolPolicy::*);
        match (self.config.mode, policy) {
            (_, Auto) => ApprovalDecision::Allow,
            (_, Always) => require(call, "always-policy tool"),
            (AutoRun, OnDemand) => ApprovalDecision::Allow,
            (AllowList, OnDemand) if self.has_grant(call) => ApprovalDecision::Allow,
            (AllowList, OnDemand) => require(call, "not in allow-list"),
            (Manual, OnDemand) => require(call, "manual mode"),
        }
    }

    fn has_grant(&self, call: &ToolCall<'_>) -> bool {
        self.config.allow_list.iter().any(|k| k == &call.grant_key())
    }
}

fn require(call: &ToolCall<'_>, why: &str) -> ApprovalDecision {
    ApprovalDecision::RequireApproval {
        reason: format!("{}: {}", call.tool, why),
    }
}

/// Convenience: build the default tool_policies map from the const table.
pub fn default_tool_policy_map() -> HashMap<String, ToolPolicy> {
    DEFAULT_TOOL_POLICIES.iter().map(|(n, p)| (n.to_string(), *p)).collect()
}
```

> **Note on dynamic resolvers**: The design's Phase 3 (per-tool `ToolPolicyResolver`) is folded into the registration layer in Task 7 — the registration layer pre-resolves the dynamic policy and passes the resulting `ToolPolicy` into the gate call. This keeps the gate stateless per-call. An alternative (gate holds `dynamic_resolvers: HashMap<String, Box<dyn ToolPolicyResolver>>`) is also valid; pick whichever integrates cleaner with `gated_tool.rs` in Task 8.

- [ ] **Step 4: Add `GlobalResolver` impl for `SecurityBlacklist` in `blacklist.rs`**

```rust
// append to crates/oxios-kernel/src/approval/blacklist.rs
impl super::resolver::GlobalResolver for SecurityBlacklist {
    fn resolve(&self, call: &super::gate::ToolCall<'_>) -> Option<super::policy::ToolPolicy> {
        self.policy_for(call.args)
    }
}
```

- [ ] **Step 5: Wire `gate` module in `approval/mod.rs`**

```rust
pub mod gate;
pub use gate::{default_tool_policy_map, ApprovalDecision, ApprovalGate, ToolCall};
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p oxios-kernel --lib approval:: 2>&1 | tail -30`
Expected: PASS — all approval module tests (Tasks 1-4).

- [ ] **Step 7: Commit**

```bash
git add crates/oxios-kernel/src/approval/
git commit -m "feat(kernel): add ApprovalGate with 9-combination decision (RFC-035)"
```

---

## Task 5: Wire `ApprovalConfig` into `SecurityConfig`

**Files:**
- Modify: `crates/oxios-kernel/src/config.rs` (`SecurityConfig` struct, `Default` impl)

**Interfaces:**
- Produces: `SecurityConfig.approval: ApprovalConfig` field.

**Consumes:** Task 1.

- [ ] **Step 1: Write failing test — config parses `[security.approval]`**

```rust
// crates/oxios-kernel/src/config.rs (test section)
#[test]
fn security_config_parses_approval_section() {
    let toml = r#"
[security.approval]
mode = "auto-run"
allow_list = ["exec:curl", "web_search"]
[security.approval.tool_overrides]
exec = "always"
"#;
    let cfg: OxiosConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.security.approval.mode, ApprovalMode::AutoRun);
    assert_eq!(cfg.security.approval.allow_list, vec!["exec:curl", "web_search"]);
    assert_eq!(
        cfg.security.approval.tool_overrides.get("exec"),
        Some(&ToolPolicy::Always)
    );
}

#[test]
fn security_config_defaults_approval_to_manual() {
    let cfg = OxiosConfig::default();
    assert_eq!(cfg.security.approval.mode, ApprovalMode::Manual);
    assert!(cfg.security.approval.allow_list.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxios-kernel --lib config::tests::security_config_parses_approval 2>&1 | tail -20`
Expected: FAIL — no `approval` field.

- [ ] **Step 3: Add field to `SecurityConfig`**

In `config.rs`, locate `pub struct SecurityConfig` (~line 1683) and add:
```rust
use crate::approval::ApprovalConfig;

pub struct SecurityConfig {
    // ... existing fields ...
    /// Tool approval mode system (RFC-035).
    #[serde(default)]
    pub approval: ApprovalConfig,
}
```

In the `Default for SecurityConfig` impl (~line 1760), add `approval: ApprovalConfig::default(),`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxios-kernel --lib config::tests::security_config 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxios-kernel/src/config.rs
git commit -m "feat(kernel): wire ApprovalConfig into SecurityConfig (RFC-035)"
```

---

## Task 6: Add `[security.approval]` to `default-config.toml`

**Files:**
- Modify: `share/default-config.toml`

- [ ] **Step 1: Add documented section after `[security]` block**

After the existing `[security]` keys (`can_fork = false`, ~line 132), insert:

```toml
# Tool approval mode (RFC-035). Controls how often the agent prompts before
# running a tool. See docs/designs/2026-07-27-approval-mode-system-design.md.
[security.approval]
# "manual" (default — each tool's declared policy) | "allow-list" | "auto-run"
mode = "manual"
# Tools auto-run in allow-list mode. Format: tool name or "exec:<binary>".
# Filled automatically when you tick "don't ask again" on an approval card.
# allow_list = ["exec:curl", "web_search"]
allow_list = []
# Per-tool policy overrides. Declared policy is replaced.
# Values: "auto" | "ondemand" | "always".
# [security.approval.tool_overrides]
# exec = "always"
# web_search = "auto"

# Extend the always-enforced security blacklist (defaults always apply too).
# Matcher: command prefix/glob against the `command` arg.
# [[security.approval.blacklist]]
# description = "no kubectl delete namespace"
# command = { glob = "kubectl delete namespace *" }
```

> Note: the `[[security.approval.blacklist]]` deserialization needs a `BlacklistRule` serde impl + config struct. If not adding config-extensibility in this PR, omit that commented block and file a follow-up. Keep the section minimal for now.

- [ ] **Step 2: Verify config still loads**

Run: `cargo test -p oxios-kernel --lib config 2>&1 | tail -20`
Expected: PASS (defaults unchanged).

- [ ] **Step 3: Commit**

```bash
git add share/default-config.toml
git commit -m "docs(kernel): document [security.approval] in default-config (RFC-035)"
```

---

## Task 7: Attach `ToolPolicy` at tool registration sites

**Files:**
- Modify: `crates/oxios-kernel/src/tools/builtin/mod.rs::register_all_kernel_tools`
- Modify: `crates/oxios-kernel/src/tools/registration.rs::register_always_on`, `register_tools_from_cspace_gated`
- Modify: `crates/oxios-kernel/src/tools/kernel_bridge.rs::register_tools`

**Interfaces:**
- Produces: registration helpers accept a `&HashMap<String, ToolPolicy>` (or read `DEFAULT_TOOL_POLICIES`) and stash the policy alongside each registered tool for the gate to consult.

**Consumes:** Task 1, Task 4 (`default_tool_policy_map`).

> **Design note**: oxi-sdk's `ToolRegistry::register` takes ownership of the tool; we can't add a side-channel policy to the SDK type. Instead, the kernel maintains its own `HashMap<tool_name, ToolPolicy>` (built from `DEFAULT_TOOL_POLICIES` + overrides) and passes it to `ApprovalGate::new`. The registration sites are where we'd *extend* the defaults per-tool (e.g. MCP tools with `needs_approval` → `OnDemand`), but the policy map itself lives on the gate.

- [ ] **Step 1: Write a smoke test — registration produces a complete policy map**

This task is largely wiring; the test is that `register_all_kernel_tools` + `register_always_on` run without panic and the resulting `ApprovalGate` knows every tool name. Add to existing tool tests if present, else:

```rust
// crates/oxios-kernel/src/tools/builtin/mod.rs (test section)
#[test]
fn default_policy_map_covers_all_registered_tools() {
    let map = crate::approval::default_tool_policy_map();
    // Every tool the bridge registers must have a policy (fallback is OnDemand,
    // but we want explicit declaration for clarity).
    for name in ["exec", "read", "write", "edit", "web_search", "mcp"] {
        assert!(map.contains_key(name), "no default policy for {name}");
    }
}
```

- [ ] **Step 2: Run, expect fail-or-pass depending on Task 4 completion**

Run: `cargo test -p oxios-kernel --lib tools::builtin::tests::default_policy_map 2>&1 | tail -20`

- [ ] **Step 3: Update `register_all_kernel_tools` signature (optional, additive)**

If MCP tools should map their connector `permission` to a `ToolPolicy`, extend the function:

```rust
// builtin/mod.rs
pub fn register_all_kernel_tools(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    _agent_id: &str,
    policy_map: &mut HashMap<String, ToolPolicy>,  // ← new, fills MCP policies
) {
    // existing registrations unchanged
    policy_map.insert("exec".into(), ToolPolicy::OnDemand);
    // ... etc, or rely on DEFAULT_TOOL_POLICIES
}
```

Keep changes minimal: prefer reading `DEFAULT_TOOL_POLICIES` in the gate and only mutate the map for dynamic tools (MCP). If MCP integration is out of scope for this PR, skip the signature change and document as Phase 3 follow-up.

- [ ] **Step 4: Run full kernel test suite**

Run: `cargo test -p oxios-kernel --lib 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxios-kernel/src/tools/
git commit -m "feat(kernel): thread ToolPolicy through registration sites (RFC-035)"
```

---

## Task 8: Call `ApprovalGate` from `GatedTool`

**Files:**
- Modify: `crates/oxios-kernel/src/tools/gated_tool.rs`
- Modify: `crates/oxios-kernel/src/tools/exec_tool.rs` (build the `ToolCall` from exec params)

**Interfaces:**
- Produces: `GatedTool` holds an `Arc<ApprovalGate>` and consults it after the 4-layer gate passes. On `RequireApproval`, it publishes `KernelEvent::ApprovalRequested` and awaits the oneshot (same pattern as the current exec shell approval).

**Consumes:** Task 4, existing `PendingToolApprovals` + `EventBus`.

- [ ] **Step 1: Write integration test — gate-driven approval for exec**

```rust
// gated_tool.rs test section
#[tokio::test]
async fn gated_tool_prompts_when_gate_requires_approval() {
    // Build a GatedTool wrapping a stub tool, with an ApprovalGate in Manual mode.
    // Assert that executing it returns an approval-pending state and that
    // resolving the approval lets it proceed.
    // (Use the existing PendingToolApprovals test harness pattern.)
}
```

(Full test body depends on existing harness; mirror `exec_tool.rs` approval tests.)

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p oxios-kernel --lib tools::gated_tool 2>&1 | tail -20`

- [ ] **Step 3: Add `ApprovalGate` to `GatedTool`**

In `gated_tool.rs`, add a field `approval_gate: Option<Arc<ApprovalGate>>` and call it after the existing gate check:

```rust
// In GatedTool::execute, after `self.gate.check(...)?` succeeds:
if let Some(ag) = &self.approval_gate {
    let call = ToolCall {
        tool: self.inner.name(),
        binary: /* extract from params if exec */,
        args: &params,
    };
    match ag.evaluate(&call) {
        ApprovalDecision::Allow => { /* proceed */ }
        ApprovalDecision::RequireApproval { reason } => {
            // publish ApprovalRequested, await oneshot — same as exec_tool.rs:598
            return self.request_approval(reason, ...).await;
        }
    }
}
```

- [ ] **Step 4: Remove the exec shell approval block**

In `exec_tool.rs`, delete lines ~598-622 (the `if let (Some(approvals), Some(bus))` block). The gate now handles it. Confirm `shell_exec` still runs after the gate returns `Allow`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p oxios-kernel --lib tools:: 2>&1 | tail -30`
Expected: PASS — including the migration-parity scenarios (structured curl auto-runs in manual mode).

- [ ] **Step 6: Commit**

```bash
git add crates/oxios-kernel/src/tools/gated_tool.rs crates/oxios-kernel/src/tools/exec_tool.rs
git commit -m "refactor(kernel): route exec approval through ApprovalGate (RFC-035)"
```

---

## Task 9: HTTP API — `/api/security/approval`

**Files:**
- Modify: `src/api/` (the binary's HTTP surface — locate the existing security/approvals routes)
- Modify: `crates/oxios-kernel/src/kernel_handle/security_api.rs` (add `approval_config()`, `set_approval_config()`, `add_grant()`, `remove_grant()`)

**Interfaces:**
- Produces:
  - `GET /api/security/approval` → `ApprovalConfig` JSON
  - `PATCH /api/security/approval` → update `mode` / `allow_list` / `tool_overrides`, persists config
  - `POST /api/security/approval/allow-list` body `{key}` → add grant
  - `DELETE /api/security/approval/allow-list/{key}` → remove grant
  - `POST /api/chat/tool-approval/{id}/respond` gains `remember: bool` → on true + allow-list mode, adds grant

**Consumes:** Task 5.

- [ ] **Step 1: Add `SecurityApi` accessors**

```rust
// security_api.rs
impl SecurityApi {
    pub fn approval_config(&self) -> ApprovalConfig { /* read config */ }
    pub async fn set_approval_config(&self, cfg: ApprovalConfig) -> Result<()> { /* write + persist */ }
    pub async fn add_grant(&self, key: String) -> Result<()> { /* push to allow_list, persist */ }
    pub async fn remove_grant(&self, key: &str) -> Result<()> { /* retain !=, persist */ }
}
```

- [ ] **Step 2: Add axum routes mirroring existing `/api/approvals` patterns**

Locate the existing approval routes in `src/api/` (search for `tool-approval`). Add the four new endpoints next to them, following the same auth + error-handling middleware.

- [ ] **Step 3: Add `remember` to the respond handler**

In the `tool-approval/{id}/respond` handler, accept `remember: bool`. On `approved && remember && mode == AllowList`, call `add_grant(grant_key)` where `grant_key` is derived from the original `ApprovalRequested` event's tool/binary.

- [ ] **Step 4: Test endpoints**

Run the daemon and curl:
```bash
curl -X PATCH localhost:4200/api/security/approval -d '{"mode":"allow-list"}' -H 'Content-Type: application/json'
curl localhost:4200/api/security/approval
```
Expected: mode reflects `allow-list`.

- [ ] **Step 5: Commit**

```bash
git add src/api/ crates/oxios-kernel/src/kernel_handle/security_api.rs
git commit -m "feat(kernel): add /api/security/approval endpoints (RFC-035)"
```

---

# Phase 2: Web UI

## Task 10: TypeScript mirror types + `useApprovalConfig` hook

**Files:**
- Create: `web/src/types/approval.ts`
- Create: `web/src/hooks/use-approval-config.ts`

- [ ] **Step 1: Define TS types mirroring kernel**

```typescript
// web/src/types/approval.ts
export type ApprovalMode = 'manual' | 'allow-list' | 'auto-run'
export type ToolPolicy = 'auto' | 'ondemand' | 'always'

export interface ApprovalConfig {
  mode: ApprovalMode
  allow_list: string[]
  tool_overrides: Record<string, ToolPolicy>
}
```

- [ ] **Step 2: Implement hook with react-query**

Mirror `web/src/hooks/use-approvals.ts` patterns (queryKey, mutation, optimistic update):

```typescript
// web/src/hooks/use-approval-config.ts
export function useApprovalConfig() {
  return useQuery({
    queryKey: ['approval-config'],
    queryFn: () => api.get<ApprovalConfig>('/api/security/approval'),
  })
}

export function useUpdateApprovalMode() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (mode: ApprovalMode) =>
      api.patch('/api/security/approval', { mode }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['approval-config'] }),
  })
}

export function useAddGrant() { /* POST /api/security/approval/allow-list */ }
export function useRemoveGrant() { /* DELETE */ }
```

- [ ] **Step 3: Commit**

```bash
git add web/src/types/approval.ts web/src/hooks/use-approval-config.ts
git commit -m "feat(web): add approval config types and hook (RFC-035)"
```

---

## Task 11: `ApprovalModeSelector` dropdown

**Files:**
- Create: `web/src/components/chat/approval-mode-selector.tsx`
- Modify: chat input control bar to mount it (locate existing control bar, e.g. `web/src/components/chat/input-bar` or similar — search for where `TextSelectionBar` / send button live)
- Modify: `web/src/i18n/locales/{en,ko}.json` + `web/public/locales/{en,ko}/common.json`

- [ ] **Step 1: Implement dropdown (lobehub `ApprovalMode.tsx` pattern)**

Three menu items: Manual (Hand icon), AllowList (ListChecks), AutoRun (Zap). Uses `useApprovalConfig` + `useUpdateApprovalMode`. Disabled when not connected.

- [ ] **Step 2: Add i18n strings**

```json
{ "approval": { "mode": { "manual": "Manual approval", "allowList": "Allow list", "autoRun": "Auto-run", "tooltip": "Tool approval mode" } } }
```
Korean equivalents in `ko.json`.

- [ ] **Step 3: Mount in chat input control bar**

- [ ] **Step 4: Manual test in browser** — switch modes, verify `GET /api/security/approval` reflects change, verify a tool call behaves per mode.

- [ ] **Step 5: Commit**

```bash
git add web/src/components/chat/approval-mode-selector.tsx web/src/i18n web/public/locales
git commit -m "feat(web): add approval mode dropdown to chat input (RFC-035)"
```

---

## Task 12: "Remember" checkbox in `ToolApprovalCard`

**Files:**
- Modify: `web/src/components/chat/tool-approval-card.tsx`
- Modify: `web/src/stores/chat.ts::resolveToolApproval` (add `remember` param)

- [ ] **Step 1: Add checkbox, visible only when `mode === 'allow-list'`**

Read mode via `useApprovalConfig`. Pass `remember` through `resolveToolApproval(id, approved, remember)` → POST body.

- [ ] **Step 2: Update store signature**

```typescript
resolveToolApproval: (id: string, approved: boolean, remember?: boolean) => Promise<void>
```

- [ ] **Step 3: Manual test** — in allow-list mode, approve `exec:curl` with remember ticked, verify next `curl` auto-runs.

- [ ] **Step 4: Commit**

```bash
git add web/src/components/chat/tool-approval-card.tsx web/src/stores/chat.ts
git commit -m "feat(web): add 'remember' to approval card in allow-list mode (RFC-035)"
```

---

## Task 13: Security settings page — manage mode + allow_list

**Files:**
- Modify: `web/src/routes/security.tsx`

- [ ] **Step 1: Add approval config panel**

Mode dropdown (same as Task 11), allow_list list with remove buttons, tool_overrides advanced editor (optional — can defer), blacklist rule editor (defer to follow-up).

- [ ] **Step 2: Manual test**

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/security.tsx
git commit -m "feat(web): add approval config management to security page (RFC-035)"
```

---

# Phase 3: Integration & Migration

## Task 14: Migration parity test

**Files:**
- Create or extend: `crates/oxios-kernel/tests/approval_migration.rs`

- [ ] **Step 1: Encode the parity matrix from design §7.3**

```rust
#[test]
fn structured_allowed_binary_runs_without_approval_in_manual() {
    // Default config (manual mode), exec structured curl → Allow
}
#[test]
fn shell_mode_prompts_in_manual() {
    // exec shell → RequireApproval
}
#[test]
fn autorun_mode_runs_shell_without_approval() {
    // auto-run + shell → Allow
}
#[test]
fn blacklist_rm_rf_prompts_even_in_autorun() {
    // auto-run + rm -rf / → RequireApproval
}
```

- [ ] **Step 2: Run, verify pass**

Run: `cargo test -p oxios-kernel --test approval_migration 2>&1 | tail -20`

- [ ] **Step 3: Commit**

```bash
git add crates/oxios-kernel/tests/approval_migration.rs
git commit -m "test(kernel): add approval-mode migration parity tests (RFC-035)"
```

---

## Task 15: End-to-end user scenario

- [ ] **Step 1: Start daemon, open Web UI**

```bash
cargo run -- start &
# open http://localhost:4200
```

- [ ] **Step 2: Scenario A — weather query, no more prompts**

1. Set mode to `auto-run` via dropdown.
2. Ask "서울 날씨 어때".
3. Verify agent runs `curl wttr.in/Seoul` without an approval card.
4. Verify response includes weather.

- [ ] **Step 3: Scenario B — blacklist still blocks**

1. Stay in `auto-run`.
2. Ask agent to run `rm -rf /tmp/test` (or any blacklist match).
3. Verify approval card appears despite auto-run.

- [ ] **Step 4: Scenario C — allow-list remember**

1. Set mode to `allow-list`.
2. Trigger `exec git status` → card appears with "remember" checkbox.
3. Tick remember, approve.
4. Trigger `exec git status` again → auto-runs.
5. Trigger `exec git push` → card appears (different binary).

- [ ] **Step 5: Commit any test-fixture or doc updates**

```bash
git commit --allow-empty -m "chore: verify RFC-035 e2e scenarios (weather, blacklist, remember)"
```

---

## Task 16: Update docs

- [ ] **Step 1: Add `docs/ARCHITECTURE.md` section** referencing the approval gate and its layering above the 4-layer access gate.

- [ ] **Step 2: Update `docs/USER-GUIDE.md`** with the dropdown explanation and the three modes.

- [ ] **Step 3: Update `CHANGELOG.md`** under an Unreleased / next-version heading.

- [ ] **Step 4: Commit**

```bash
git add docs/ARCHITECTURE.md docs/USER-GUIDE.md CHANGELOG.md
git commit -m "docs: document approval mode system (RFC-035)"
```

---

# Self-Review Checklist

After writing this plan, the following were verified against the spec (`docs/designs/2026-07-27-approval-mode-system-design.md`):

- **§4 Data model** → Task 1 (policy types), Task 5 (config wiring). ✅
- **§5 Pipeline** → Task 4 (gate.evaluate). The 4 phases are present; note Phase 3 dynamic resolver is folded into registration (Task 7) rather than a separate gate field — documented inline. ✅
- **§6 Co-location** → Task 7. ✅
- **§7 Dynamic resolver** → Task 2 (ExecPolicyResolver). Migration parity in Task 14. ✅
- **§8 SecurityBlacklist** → Task 3 + Task 4 (GlobalResolver impl). ✅
- **§9 Config schema** → Task 5, Task 6. ✅
- **§10 Web UI** → Tasks 10-13. ✅
- **§11 Existing-code integration** → Task 8 (gated_tool + exec removal), Task 9 (API). ✅
- **§12 Migration** → Task 14. ✅
- **§13 Validation scenarios** → Task 15. ✅

**Gaps deferred to follow-up (call out in PR):**
- `[[security.approval.blacklist]]` config-extensibility deserialization (Task 6 — kept as comment for now).
- MCP connector `permission → ToolPolicy` mapping (Task 7 — minimal signature change; full MCP wiring is Phase 3 follow-up).
- `tool_overrides` and blacklist rule editing UI (Task 13 — defer to a later iteration).

**Type consistency:** `ToolPolicy::{Auto, OnDemand, Always}`, `ApprovalMode::{Manual, AllowList, AutoRun}`, `ApprovalGate::evaluate(&self, &ToolCall) -> ApprovalDecision` — names match across Tasks 1-4 and the Web TS mirror in Task 10. ✅

---

# Execution Handoff

Plan complete and saved to `docs/designs/2026-07-27-approval-mode-system-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
