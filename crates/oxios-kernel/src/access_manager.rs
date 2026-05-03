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
    fn log_access(
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

    #[test]
    fn test_default_permissions() {
        let perms = AgentPermissions::default();
        assert!(perms.allowed_tools.contains("bash"));
        assert!(!perms.network_access);
    }

    #[test]
    fn test_path_matching() {
        let mut perms = AgentPermissions::for_new_agent("test");
        perms.allowed_paths = vec!["/workspace/**".to_string(), "/home/*/docs/**".to_string()];
        perms.denied_paths = vec!["/workspace/.oxios/**".to_string()];

        assert!(perms.is_path_allowed("/workspace/project/file.rs"));
        assert!(perms.is_path_allowed("/home/user/docs/readme.md"));
        assert!(!perms.is_path_allowed("/etc/passwd"));
        assert!(!perms.is_path_allowed("/home/user/secret.txt"));

        // Denied takes precedence.
        assert!(perms.is_path_denied("/workspace/.oxios/config.toml"));
    }

    #[test]
    fn test_access_manager_tool_check() {
        let mut access = AccessManager::new();

        let mut perms = AgentPermissions::for_new_agent("code-agent");
        perms.allow_tool("bash");
        perms.allow_tool("read");
        perms.deny_tool("network");
        access.set_permissions(perms);

        assert!(access.can_use_tool("code-agent", "bash"));
        assert!(access.can_use_tool("code-agent", "read"));
        assert!(!access.can_use_tool("code-agent", "network")); // denied by deny_tool()
        assert!(!access.can_use_tool("code-agent", "spawn")); // not in default set
        assert!(!access.can_use_tool("unknown-agent", "bash")); // unknown agent
    }

    #[test]
    fn test_access_manager_path_check() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("file-agent");
        access.set_permissions(perms);

        assert!(access.can_access_path("file-agent", "/workspace/project/file.rs"));
        assert!(!access.can_access_path("file-agent", "/etc/passwd"));
    }

    #[test]
    fn test_audit_log() {
        let mut access = AccessManager::new();

        let perms = AgentPermissions::for_new_agent("test-agent");
        access.set_permissions(perms);

        access.can_use_tool("test-agent", "bash");
        access.can_use_tool("test-agent", "network");

        let log = access.audit_log();
        assert_eq!(log.len(), 2);
        assert!(log[0].allowed);
        assert!(!log[1].allowed);
    }

    #[test]
    fn test_deny_tool_removes_from_set() {
        let mut perms = AgentPermissions::for_new_agent("test");
        assert!(perms.allowed_tools.contains("bash"));

        perms.deny_tool("bash");
        assert!(!perms.allowed_tools.contains("bash"));
    }
}