//! Agent permissions types — per-agent permission sets and audit entries.

use chrono::{DateTime, Utc};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
            allowed_tools: ["read", "write", "edit", "bash", "grep", "find", "exec"]
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

/// Update struct for permission changes (partial updates).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionUpdate {
    /// Set of allowed tool names.
    #[serde(default)]
    pub allowed_tools: Option<HashSet<String>>,
    /// Allowed path patterns (glob).
    #[serde(default)]
    pub allowed_paths: Option<Vec<String>>,
    /// Denied path patterns (glob).
    #[serde(default)]
    pub denied_paths: Option<Vec<String>>,
    /// Whether this agent can make network requests.
    #[serde(default)]
    pub network_access: Option<bool>,
    /// Maximum execution time in seconds (0 = unlimited).
    #[serde(default)]
    pub max_execution_time_secs: Option<u64>,
    /// Maximum memory in MB (0 = unlimited).
    #[serde(default)]
    pub max_memory_mb: Option<u64>,
    /// Whether this agent can spawn sub-agents.
    #[serde(default)]
    pub can_fork: Option<bool>,
}

impl PermissionUpdate {
    /// Apply this update to a permission set.
    pub fn apply(&self, perms: &mut AgentPermissions) {
        if let Some(tools) = &self.allowed_tools {
            perms.allowed_tools = tools.clone();
        }
        if let Some(paths) = &self.allowed_paths {
            perms.allowed_paths = paths.clone();
        }
        if let Some(paths) = &self.denied_paths {
            perms.denied_paths = paths.clone();
        }
        if let Some(v) = self.network_access {
            perms.network_access = v;
        }
        if let Some(v) = self.max_execution_time_secs {
            perms.max_execution_time_secs = v;
        }
        if let Some(v) = self.max_memory_mb {
            perms.max_memory_mb = v;
        }
        if let Some(v) = self.can_fork {
            perms.can_fork = v;
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
    pub(crate) fn is_path_denied(&self, path: &str) -> bool {
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
    pub(crate) fn is_path_allowed(&self, path: &str) -> bool {
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
    pub(crate) fn new(
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
