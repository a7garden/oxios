//! Access Manager — least-privilege security for agents.
//!
//! Inspired by OWASP Agentic AI security guidelines:
//! - Least privilege by default
//! - Agent identity and audit logging
//! - Sandbox boundaries (path restrictions)
//! - Tool access control (which agent can use which tools)
//!
//! Every agent starts with minimal permissions and must be explicitly granted
//! access to tools, paths, and network resources.

use chrono::{DateTime, Utc};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Permissions for a single agent.
///
/// Agents start with minimal permissions (least privilege).
/// Additional permissions must be explicitly granted via configuration
/// or an authorized request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermissions {
    /// Name of the agent this permission set applies to.
    pub agent_name: String,
    /// Set of allowed tool names. Empty means no tools allowed.
    #[serde(default)]
    pub allowed_tools: HashSet<String>,
    /// Allowed path patterns (glob). Used for file operations.
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    /// Denied path patterns (glob). Always blocked, even if allowed_paths matches.
    #[serde(default)]
    pub denied_paths: Vec<String>,
    /// Whether this agent can make network requests.
    #[serde(default)]
    pub network_access: bool,
    /// Maximum execution time in seconds (0 = unlimited).
    #[serde(default)]
    pub max_execution_time_secs: u64,
    /// Maximum memory in MB (0 = unlimited).
    #[serde(default)]
    pub max_memory_mb: u64,
    /// Whether this agent can spawn sub-agents.
    #[serde(default)]
    pub can_fork: bool,
}

impl Default for AgentPermissions {
    fn default() -> Self {
        Self {
            agent_name: String::new(),
            // By default, agents get basic file tools.
            // Network access is denied by default.
            allowed_tools: ["read", "write", "edit", "bash", "grep", "find"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            allowed_paths: vec!["/workspace/**".to_string()],
            denied_paths: vec![
                "/etc/**".to_string(),
                "/root/**".to_string(),
                "/sys/**".to_string(),
                "/proc/**".to_string(),
                ".oxios/**".to_string(),
            ],
            network_access: false,
            max_execution_time_secs: 300,
            max_memory_mb: 512,
            can_fork: false,
        }
    }
}

impl AgentPermissions {
    /// Creates permissions for a new agent with the default restrictive set.
    pub fn for_new_agent(agent_name: &str) -> Self {
        Self {
            agent_name: agent_name.to_string(),
            ..Default::default()
        }
    }

    /// Adds a tool to the allowed set.
    pub fn allow_tool(&mut self, tool: &str) {
        self.allowed_tools.insert(tool.to_string());
    }

    /// Removes a tool from the allowed set.
    pub fn deny_tool(&mut self, tool: &str) {
        self.allowed_tools.remove(tool);
    }

    /// Adds a path pattern to the allowed set.
    pub fn allow_path(&mut self, path: &str) {
        if !self.allowed_paths.contains(&path.to_string()) {
            self.allowed_paths.push(path.to_string());
        }
    }

    /// Adds a path pattern to the denied set.
    pub fn deny_path(&mut self, path: &str) {
        if !self.denied_paths.contains(&path.to_string()) {
            self.denied_paths.push(path.to_string());
        }
    }

    /// Enables network access for this agent.
    pub fn enable_network(&mut self) {
        self.network_access = true;
    }

    /// Enables agent forking (spawning sub-agents).
    pub fn enable_forking(&mut self) {
        self.can_fork = true;
    }

    /// Checks if a path matches any denied pattern.
    fn is_path_denied(&self, path: &str) -> bool {
        for pattern in &self.denied_paths {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(path) {
                    return true;
                }
            }
        }
        false
    }

    /// Checks if a path matches any allowed pattern.
    fn is_path_allowed(&self, path: &str) -> bool {
        for pattern in &self.allowed_paths {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(path) {
                    return true;
                }
            }
        }
        false
    }
}

/// An entry in the security audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the action occurred.
    pub timestamp: DateTime<Utc>,
    /// Agent that performed the action.
    pub agent_name: String,
    /// The action attempted (e.g., "use_tool", "access_path", "network_request").
    pub action: String,
    /// The resource involved (e.g., "bash", "/workspace/file.txt").
    pub resource: String,
    /// Whether the action was allowed.
    pub allowed: bool,
    /// Reason for the decision (e.g., "path not in allowed list").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AuditEntry {
    /// Creates a new audit entry.
    fn new(
        agent_name: &str,
        action: &str,
        resource: &str,
        allowed: bool,
        reason: Option<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            agent_name: agent_name.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            allowed,
            reason,
        }
    }
}

/// Access Manager.
///
/// Manages agent permissions, enforces security boundaries, and maintains
/// an audit log of all security-relevant actions.
///
/// # Usage
/// ```rust,ignore
/// let mut access = AccessManager::new();
///
/// // Create permissions for a new agent
/// access.set_permissions(AgentPermissions::for_new_agent("code-agent"));
///
/// // Check permissions
/// if access.can_use_tool("code-agent", "bash") {
///     // allow bash execution
/// }
///
/// // Check path access
/// if access.can_access_path("code-agent", "/workspace/project/file.rs") {
///     // allow file access
/// }
///
/// // View audit log
/// for entry in access.audit_log() {
///     println!("{:?} {} {} -> {}", entry.timestamp, entry.agent_name, entry.action, entry.allowed);
/// }
/// ```
pub struct AccessManager {
    /// Permissions for each agent.
    permissions: HashMap<String, AgentPermissions>,
    /// Audit log of all access decisions.
    audit_log: Vec<AuditEntry>,
    /// Maximum audit log entries to retain.
    max_audit_entries: usize,
}

impl AccessManager {
    /// Creates a new access manager.
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
            audit_log: Vec::new(),
            max_audit_entries: 10_000,
        }
    }

    /// Creates a new access manager with custom settings.
    ///
    /// # Arguments
    /// * `max_audit_entries` - Maximum audit log size (oldest entries are pruned)
    pub fn with_max_audit_entries(max_audit_entries: usize) -> Self {
        Self {
            permissions: HashMap::new(),
            audit_log: Vec::new(),
            max_audit_entries,
        }
    }

    /// Checks if an agent is allowed to use a specific tool.
    ///
    /// Logs the access decision to the audit log.
    pub fn can_use_tool(&mut self, agent_name: &str, tool: &str) -> bool {
        let allowed = match self.permissions.get(agent_name) {
            Some(perms) => perms.allowed_tools.contains(tool),
            None => {
                tracing::warn!(agent = %agent_name, tool = %tool, "Agent not found in access manager, denying");
                false
            }
        };

        let reason = if allowed {
            None
        } else {
            Some("tool not in allowed set".to_string())
        };

        self.log_access(agent_name, "use_tool", tool, allowed, reason);

        allowed
    }

    /// Checks if an agent is allowed to access a specific path.
    ///
    /// Enforces both allowed_paths and denied_paths rules.
    /// A path is only allowed if it matches an allowed pattern AND
    /// does not match any denied pattern.
    ///
    /// Logs the access decision to the audit log.
    pub fn can_access_path(&mut self, agent_name: &str, path: &str) -> bool {
        let allowed = match self.permissions.get(agent_name) {
            Some(perms) => {
                // First check denials (they take precedence).
                if perms.is_path_denied(path) {
                    false
                } else {
                    perms.is_path_allowed(path)
                }
            }
            None => {
                tracing::warn!(agent = %agent_name, path = %path, "Agent not found, denying path access");
                false
            }
        };

        let reason = if allowed {
            None
        } else {
            Some("path not in allowed set or is denied".to_string())
        };

        self.log_access(agent_name, "access_path", path, allowed, reason);

        allowed
    }

    /// Checks if an agent is allowed to make network requests.
    ///
    /// Logs the access decision to the audit log.
    pub fn can_access_network(&mut self, agent_name: &str) -> bool {
        let allowed = match self.permissions.get(agent_name) {
            Some(perms) => perms.network_access,
            None => false,
        };

        let reason = if allowed {
            None
        } else {
            Some("network access not enabled".to_string())
        };

        self.log_access(agent_name, "network_request", "<network>", allowed, reason);

        allowed
    }

    /// Checks if an agent can execute for the given duration (in seconds).
    ///
    /// Returns true if unlimited (max_execution_time_secs = 0) or
    /// if the requested duration is within the limit.
    pub fn can_execute_for(&self, agent_name: &str, duration_secs: u64) -> bool {
        match self.permissions.get(agent_name) {
            Some(perms) => {
                perms.max_execution_time_secs == 0
                    || duration_secs <= perms.max_execution_time_secs
            }
            None => false,
        }
    }

    /// Checks if an agent can use the specified amount of memory (in MB).
    ///
    /// Returns true if unlimited (max_memory_mb = 0) or
    /// if the requested memory is within the limit.
    pub fn can_use_memory(&self, agent_name: &str, memory_mb: u64) -> bool {
        match self.permissions.get(agent_name) {
            Some(perms) => perms.max_memory_mb == 0 || memory_mb <= perms.max_memory_mb,
            None => false,
        }
    }

    /// Checks if an agent can fork (spawn sub-agents).
    pub fn can_fork(&self, agent_name: &str) -> bool {
        match self.permissions.get(agent_name) {
            Some(perms) => perms.can_fork,
            None => false,
        }
    }

    /// Gets the permission set for an agent.
    ///
    /// Returns None if no permissions are defined for the agent.
    pub fn get_permissions(&self, agent_name: &str) -> Option<&AgentPermissions> {
        self.permissions.get(agent_name)
    }

    /// Gets the permission set for an agent, creating a default one if needed.
    ///
    /// Useful for dynamically creating permissions on first access.
    pub fn get_or_create_permissions(&mut self, agent_name: &str) -> &mut AgentPermissions {
        self.permissions
            .entry(agent_name.to_string())
            .or_insert_with(|| AgentPermissions::for_new_agent(agent_name))
    }

    /// Sets the permissions for an agent.
    ///
    /// Overwrites any existing permissions for this agent.
    pub fn set_permissions(&mut self, permissions: AgentPermissions) {
        let agent_name = permissions.agent_name.clone();
        self.permissions.insert(agent_name, permissions);
    }

    /// Removes permissions for an agent.
    ///
    /// After removal, all access by this agent will be denied.
    pub fn remove_permissions(&mut self, agent_name: &str) {
        self.permissions.remove(agent_name);
        tracing::info!(agent = %agent_name, "Agent permissions removed");
    }

    /// Lists all agents with defined permissions.
    pub fn list_agents(&self) -> Vec<String> {
        self.permissions.keys().cloned().collect()
    }

    /// Gets the full audit log.
    ///
    /// Returns entries in chronological order (oldest first).
    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// Gets recent audit log entries.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of entries to return (from the end)
    pub fn audit_log_recent(&self, limit: usize) -> Vec<AuditEntry> {
        let start = self.audit_log.len().saturating_sub(limit);
        self.audit_log[start..].to_vec()
    }

    /// Gets audit log entries for a specific agent.
    pub fn audit_log_for_agent(&self, agent_name: &str) -> Vec<AuditEntry> {
        self.audit_log
            .iter()
            .filter(|e| e.agent_name == agent_name)
            .cloned()
            .collect()
    }

    /// Searches audit log for denied actions.
    pub fn denied_actions(&self) -> Vec<&AuditEntry> {
        self.audit_log.iter().filter(|e| !e.allowed).collect()
    }

    /// Clears the audit log.
    pub fn clear_audit_log(&mut self) {
        let count = self.audit_log.len();
        self.audit_log.clear();
        tracing::info!(cleared = count, "Audit log cleared");
    }

    /// Logs an access decision to the audit log.
    ///
    /// Automatically prunes old entries if max_audit_entries is exceeded.
    pub(crate) fn log_access(
        &mut self,
        agent_name: &str,
        action: &str,
        resource: &str,
        allowed: bool,
        reason: Option<String>,
    ) {
        let entry = AuditEntry::new(agent_name, action, resource, allowed, reason.clone());

        self.audit_log.push(entry);

        // Prune if needed.
        if self.audit_log.len() > self.max_audit_entries {
            let prune_count = self.audit_log.len() - self.max_audit_entries;
            self.audit_log.drain(0..prune_count);
        }

        // Trace denied actions at warn level.
        if !allowed {
            tracing::warn!(
                agent = %agent_name,
                action = %action,
                resource = %resource,
                reason = ?reason,
                "Access denied"
            );
        }
    }

    /// Validates a permission set for correctness.
    ///
    /// Returns a list of warnings about the permissions.
    pub fn validate_permissions(&self, perms: &AgentPermissions) -> Vec<String> {
        let mut warnings = Vec::new();

        if perms.allowed_tools.is_empty() {
            warnings.push("Agent has no allowed tools".to_string());
        }

        if perms.allowed_paths.is_empty() {
            warnings.push("Agent has no path restrictions (wide open)".to_string());
        }

        if perms.network_access {
            warnings.push("Agent has network access enabled".to_string());
        }

        if perms.can_fork {
            warnings.push("Agent can fork sub-agents".to_string());
        }

        if perms.max_execution_time_secs == 0 {
            warnings.push("Agent has no execution time limit".to_string());
        }

        if perms.max_memory_mb == 0 {
            warnings.push("Agent has no memory limit".to_string());
        }

        warnings
    }
}

impl Default for AccessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- AgentPermissions tests ---

    #[test]
    fn test_default_permissions() {
        let perms = AgentPermissions::default();
        assert!(perms.allowed_tools.contains("bash"));
        assert!(!perms.network_access);
        assert!(!perms.can_fork);
        assert_eq!(perms.max_execution_time_secs, 300);
        assert_eq!(perms.max_memory_mb, 512);
    }

    #[test]
    fn test_for_new_agent() {
        let perms = AgentPermissions::for_new_agent("my-agent");
        assert_eq!(perms.agent_name, "my-agent");
        assert!(perms.allowed_tools.contains("bash"));
    }

    #[test]
    fn test_allow_deny_tool() {
        let mut perms = AgentPermissions::for_new_agent("test");
        assert!(perms.allowed_tools.contains("bash"));

        perms.deny_tool("bash");
        assert!(!perms.allowed_tools.contains("bash"));

        perms.allow_tool("custom");
        assert!(perms.allowed_tools.contains("custom"));
    }

    #[test]
    fn test_allow_deny_path() {
        let mut perms = AgentPermissions::for_new_agent("test");

        perms.allow_path("/workspace/**");
        assert!(perms.allowed_paths.contains(&"/workspace/**".to_string()));

        perms.deny_path("/workspace/.secret/**");
        assert!(perms.denied_paths.contains(&"/workspace/.secret/**".to_string()));
    }

    #[test]
    fn test_enable_network() {
        let mut perms = AgentPermissions::for_new_agent("test");
        assert!(!perms.network_access);

        perms.enable_network();
        assert!(perms.network_access);
    }

    #[test]
    fn test_enable_forking() {
        let mut perms = AgentPermissions::for_new_agent("test");
        assert!(!perms.can_fork);

        perms.enable_forking();
        assert!(perms.can_fork);
    }

    #[test]
    fn test_path_matching_allowed() {
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.allowed_paths = vec!["/workspace/**".to_string(), "/home/*/docs/**".to_string()];
        perms.denied_paths = vec!["/workspace/.oxios/**".to_string()];

        // Matches allowed.
        assert!(perms.is_path_allowed("/workspace/project/file.rs"));
        assert!(perms.is_path_allowed("/home/user/docs/readme.md"));

        // Does not match any allowed pattern.
        assert!(!perms.is_path_allowed("/etc/passwd"));
        assert!(!perms.is_path_allowed("/home/user/secret.txt"));
    }

    #[test]
    fn test_path_matching_denied() {
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.allowed_paths = vec!["/workspace/**".to_string()];
        perms.denied_paths = vec!["/workspace/.oxios/**".to_string()];

        // Denied takes precedence over allowed.
        assert!(perms.is_path_denied("/workspace/.oxios/config.toml"));
        assert!(!perms.is_path_denied("/workspace/project/file.rs"));

        // Even though it matches allowed, denied blocks it.
        let _access = AccessManager::new();
        let mut perms2 = perms.clone();
        perms2.agent_name = "test".to_string();
        // The is_path_denied check happens first in can_access_path.
        // If allowed_paths matches but denied_paths also matches, it's blocked.
        // We test this via the full can_access_path method.
    }

    #[test]
    fn test_path_denied_pattern_matching() {
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.denied_paths = vec!["/etc/**".to_string(), "**/secrets/*".to_string()];

        assert!(perms.is_path_denied("/etc/passwd"));
        assert!(perms.is_path_denied("/etc/shadow"));
        assert!(!perms.is_path_denied("/workspace/file"));
    }

    // --- AccessManager tool access tests ---

    #[test]
    fn test_can_use_tool_allowed() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("code-agent");
        perms.allow_tool("bash");
        perms.allow_tool("read");
        access.set_permissions(perms);

        assert!(access.can_use_tool("code-agent", "bash"));
        assert!(access.can_use_tool("code-agent", "read"));
    }

    #[test]
    fn test_can_use_tool_denied() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("code-agent");
        perms.allow_tool("read");
        perms.deny_tool("bash"); // Explicitly denied.
        access.set_permissions(perms);

        assert!(!access.can_use_tool("code-agent", "bash")); // denied
        assert!(!access.can_use_tool("code-agent", "spawn")); // not in list
        assert!(!access.can_use_tool("unknown-agent", "bash")); // unknown agent
    }

    #[test]
    fn test_unknown_agent_denied_all_tools() {
        let mut access = AccessManager::new();

        // No permissions set for unknown-agent.
        assert!(!access.can_use_tool("unknown-agent", "read"));
        assert!(!access.can_access_path("unknown-agent", "/workspace/test.txt"));
        assert!(!access.can_access_network("unknown-agent"));
        assert!(!access.can_fork("unknown-agent"));
    }

    // --- AccessManager path access tests ---

    #[test]
    fn test_can_access_path_allowed() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("file-agent");
        access.set_permissions(perms);

        assert!(access.can_access_path("file-agent", "/workspace/project/file.rs"));
        assert!(!access.can_access_path("file-agent", "/etc/passwd"));
    }

    #[test]
    fn test_can_access_path_denied_takes_precedence() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("test");
        perms.allowed_paths = vec!["/workspace/**".to_string()];
        perms.denied_paths = vec!["/workspace/.oxios/**".to_string()];
        access.set_permissions(perms);

        // Allowed but also denied → blocked.
        assert!(!access.can_access_path("test", "/workspace/.oxios/config.toml"));

        // Just allowed → allowed.
        assert!(access.can_access_path("test", "/workspace/project/file.rs"));
    }

    // --- AccessManager network access tests ---

    #[test]
    fn test_can_access_network() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("net-agent");
        perms.enable_network();
        access.set_permissions(perms);

        assert!(access.can_access_network("net-agent"));
        assert!(!access.can_access_network("no-net-agent"));
    }

    // --- Execution limits tests ---

    #[test]
    fn test_can_execute_for() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("test");
        perms.max_execution_time_secs = 300;
        access.set_permissions(perms);

        assert!(access.can_execute_for("test", 100));
        assert!(access.can_execute_for("test", 300));
        assert!(!access.can_execute_for("test", 301));
    }

    #[test]
    fn test_unlimited_execution_time() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("test");
        perms.max_execution_time_secs = 0; // unlimited
        access.set_permissions(perms);

        assert!(access.can_execute_for("test", 100_000));
    }

    #[test]
    fn test_can_use_memory() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("test");
        perms.max_memory_mb = 512;
        access.set_permissions(perms);

        assert!(access.can_use_memory("test", 256));
        assert!(access.can_use_memory("test", 512));
        assert!(!access.can_use_memory("test", 513));
    }

    #[test]
    fn test_unlimited_memory() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("test");
        perms.max_memory_mb = 0;
        access.set_permissions(perms);

        assert!(access.can_use_memory("test", 1_000_000));
    }

    // --- Fork tests ---

    #[test]
    fn test_can_fork() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("test");
        perms.enable_forking();
        access.set_permissions(perms);

        assert!(access.can_fork("test"));
        assert!(!access.can_fork("no-fork-agent"));
    }

    // --- Permission management tests ---

    #[test]
    fn test_set_and_get_permissions() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("test-agent");
        access.set_permissions(perms);

        let retrieved = access.get_permissions("test-agent");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().agent_name, "test-agent");
    }

    #[test]
    fn test_get_nonexistent_permissions() {
        let access = AccessManager::new();
        assert!(access.get_permissions("ghost").is_none());
    }

    #[test]
    fn test_get_or_create_permissions() {
        let mut access = AccessManager::new();

        // First access creates default.
        let perms = access.get_or_create_permissions("new-agent");
        assert_eq!(perms.agent_name, "new-agent");

        // Second access returns same instance.
        let perms2 = access.get_or_create_permissions("new-agent");
        assert_eq!(perms2.agent_name, "new-agent");
    }

    #[test]
    fn test_remove_permissions() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("to-remove");
        access.set_permissions(perms);

        assert!(access.get_permissions("to-remove").is_some());

        access.remove_permissions("to-remove");

        assert!(access.get_permissions("to-remove").is_none());
        // All access should now be denied.
        assert!(!access.can_use_tool("to-remove", "bash"));
    }

    #[test]
    fn test_list_agents() {
        let mut access = AccessManager::new();

        access.set_permissions(AgentPermissions::for_new_agent("agent-1"));
        access.set_permissions(AgentPermissions::for_new_agent("agent-2"));

        let agents = access.list_agents();
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&"agent-1".to_string()));
        assert!(agents.contains(&"agent-2".to_string()));
    }

    // --- Audit log tests ---

    #[test]
    fn test_audit_log_records_access() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("test-agent");
        access.set_permissions(perms);

        access.can_use_tool("test-agent", "bash"); // allowed
        access.can_use_tool("test-agent", "network"); // denied

        let log = access.audit_log();
        assert_eq!(log.len(), 2);
        assert!(log[0].allowed);
        assert!(!log[1].allowed);
        assert_eq!(log[0].agent_name, "test-agent");
        assert_eq!(log[0].action, "use_tool");
        assert_eq!(log[0].resource, "bash");
    }

    #[test]
    fn test_audit_log_recent() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("test");
        access.set_permissions(perms);

        for i in 0..10 {
            access.can_use_tool("test", &format!("tool-{}", i));
        }

        let recent = access.audit_log_recent(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_audit_log_for_agent() {
        let mut access = AccessManager::new();

        access.set_permissions(AgentPermissions::for_new_agent("agent-a"));
        access.set_permissions(AgentPermissions::for_new_agent("agent-b"));

        access.can_use_tool("agent-a", "tool1");
        access.can_use_tool("agent-b", "tool2");
        access.can_use_tool("agent-a", "tool3");

        let log_a = access.audit_log_for_agent("agent-a");
        assert_eq!(log_a.len(), 2);
    }

    #[test]
    fn test_denied_actions() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("test");
        access.set_permissions(perms);

        access.can_use_tool("test", "bash"); // allowed
        access.can_use_tool("test", "dangerous"); // denied
        access.can_access_path("test", "/etc/shadow"); // denied

        let denied = access.denied_actions();
        assert_eq!(denied.len(), 2);
    }

    #[test]
    fn test_clear_audit_log() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("test");
        access.set_permissions(perms);

        for _ in 0..5 {
            access.can_use_tool("test", "tool");
        }

        assert_eq!(access.audit_log().len(), 5);

        access.clear_audit_log();

        assert!(access.audit_log().is_empty());
    }

    // --- Max audit entries pruning ---

    #[test]
    fn test_audit_log_prunes_old_entries() {
        let mut access = AccessManager::with_max_audit_entries(5);

        let perms = AgentPermissions::for_new_agent("test");
        access.set_permissions(perms);

        // Add 10 entries.
        for i in 0..10 {
            access.can_use_tool("test", &format!("tool-{}", i));
        }

        // Should be pruned to max_audit_entries.
        assert_eq!(access.audit_log().len(), 5);
    }

    // --- Validate permissions tests ---

    #[test]
    fn test_validate_permissions_no_tools() {
        let mut access = AccessManager::new();
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.allowed_tools.clear();
        access.set_permissions(perms.clone());

        let warnings = access.validate_permissions(&perms);
        assert!(warnings.iter().any(|w| w.contains("no allowed tools")));
    }

    #[test]
    fn test_validate_permissions_no_path_restrictions() {
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.allowed_paths.clear();

        let access = AccessManager::new();
        let warnings = access.validate_permissions(&perms);
        assert!(warnings.iter().any(|w| w.contains("no path restrictions")));
    }

    #[test]
    fn test_validate_permissions_warnings() {
        let mut access = AccessManager::new();
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.network_access = true;
        perms.can_fork = true;
        perms.max_execution_time_secs = 0;
        perms.max_memory_mb = 0;
        access.set_permissions(perms.clone());

        let warnings = access.validate_permissions(&perms);
        assert!(warnings.iter().any(|w| w.contains("network access")));
        assert!(warnings.iter().any(|w| w.contains("fork sub-agents")));
        assert!(warnings.iter().any(|w| w.contains("no execution time limit")));
        assert!(warnings.iter().any(|w| w.contains("no memory limit")));
    }

    // --- AuditEntry timestamp ---

    #[test]
    fn test_audit_entry_has_timestamp() {
        let entry = AuditEntry::new("agent", "action", "resource", true, None);
        // timestamp should be set (not default DateTime).
        assert!(entry.timestamp.timestamp() > 0);
    }
}