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
use std::path::{Path, PathBuf};

use crate::types::AgentId;

// ─── RBAC Types ───────────────────────────────────────────────────────────────

/// Roles for role-based access control (3-tier model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Basic user — can use agents, limited permissions.
    User,
    /// Superuser — can manage programs, skills, containers.
    Superuser,
    /// Admin — full system access, can modify RBAC.
    Admin,
}

impl Role {
    /// Returns the default policy for this role.
    pub fn default_policy(&self) -> RbacPolicy {
        match self {
            Role::Admin => RbacPolicy {
                role: Role::Admin,
                allowed_actions: vec![
                    Action::UseTool("*".into()),
                    Action::AccessPath("*".into()),
                    Action::ManageAgents,
                    Action::ManagePrograms,
                    Action::ManageGardens,
                    Action::ManageRBAC,
                    Action::ViewAuditLog,
                    Action::SystemConfig,
                ].into_iter().collect(),
                resource_patterns: vec!["*".into()],
                max_concurrent_agents: usize::MAX,
            },
            Role::Superuser => RbacPolicy {
                role: Role::Superuser,
                allowed_actions: vec![
                    Action::UseTool("*".into()),
                    Action::AccessPath("/workspace/**".into()),
                    Action::ManageAgents,
                    Action::ManagePrograms,
                    Action::ManageGardens,
                    Action::ViewAuditLog,
                ].into_iter().collect(),
                resource_patterns: vec!["/workspace/**".into(), "/tmp/**".into()],
                max_concurrent_agents: 10,
            },
            Role::User => RbacPolicy {
                role: Role::User,
                allowed_actions: vec![
                    Action::UseTool("read".into()),
                    Action::UseTool("write".into()),
                    Action::UseTool("edit".into()),
                    Action::UseTool("bash".into()),
                    Action::UseTool("grep".into()),
                    Action::UseTool("find".into()),
                    Action::AccessPath("/workspace/**".into()),
                    Action::ManageAgents,
                ].into_iter().collect(),
                resource_patterns: vec!["/workspace/**".into()],
                max_concurrent_agents: 2,
            },
        }
    }
}

/// Subject — who is accessing the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subject {
    /// A named user.
    User(String),
    /// An agent acting on behalf of a user.
    Agent(AgentId),
    /// System-level operations (bypass RBAC).
    System,
}

impl std::fmt::Display for Subject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Subject::User(name) => write!(f, "user:{}", name),
            Subject::Agent(id) => write!(f, "agent:{}", id),
            Subject::System => write!(f, "system"),
        }
    }
}

/// Actions that can be authorized by RBAC.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Use a specific tool (name or * for all).
    UseTool(String),
    /// Access a specific path pattern (glob).
    AccessPath(String),
    /// Manage agents (fork/exec/kill).
    ManageAgents,
    /// Manage programs (install/uninstall).
    ManagePrograms,
    /// Manage gardens (create/start/stop/remove).
    ManageGardens,
    /// Modify RBAC policies and role assignments.
    ManageRBAC,
    /// View the audit log.
    ViewAuditLog,
    /// Modify system-level configuration.
    SystemConfig,
}

impl Action {
    /// Returns true if this action is considered high-risk and needs HitL approval.
    pub fn requires_approval(&self) -> bool {
        match self {
            Action::ManageRBAC | Action::SystemConfig => true,
            Action::UseTool(t) => t == "*" || t == "osascript" || t == "rm",
            _ => false,
        }
    }
}

/// RBAC policy defining what a role can do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacPolicy {
    /// The role this policy applies to.
    pub role: Role,
    /// Set of actions this role is allowed to perform.
    pub allowed_actions: HashSet<Action>,
    /// Glob patterns for accessible resources.
    pub resource_patterns: Vec<String>,
    /// Maximum number of concurrent agents for this role.
    pub max_concurrent_agents: usize,
}

impl RbacPolicy {
    /// Checks whether this policy allows the given action.
    pub fn allows(&self, action: &Action) -> bool {
        self.allowed_actions.contains(action)
    }
}

/// RBAC audit entry — records authorization decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacAuditEntry {
    /// When the authorization decision was made.
    pub timestamp: DateTime<Utc>,
    /// Who performed the action.
    pub subject: Subject,
    /// What action was attempted.
    pub action: Action,
    /// Which resource was involved.
    pub resource: String,
    /// Whether the action was allowed.
    pub allowed: bool,
    /// Optional reason for the decision.
    pub reason: Option<String>,
}

impl RbacAuditEntry {
    fn new(subject: Subject, action: Action, resource: String, allowed: bool, reason: Option<String>) -> Self {
        Self { timestamp: Utc::now(), subject, action, resource, allowed, reason }
    }
}

/// Human-in-the-loop approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Unique identifier for this approval request.
    pub id: uuid::Uuid,
    /// Who is requesting the action.
    pub subject: Subject,
    /// What action is being requested.
    pub action: Action,
    /// Which resource is involved.
    pub resource: String,
    /// Why the action needs approval.
    pub reason: String,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
}

/// Status of a HitL approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// Awaiting user decision.
    Pending,
    /// User approved the request.
    Approved,
    /// User rejected the request.
    Rejected,
    /// Request timed out.
    Expired,
}

/// RBAC Manager — manages roles, permissions, and HitL approvals.
pub struct RbacManager {
    policies: HashMap<Role, RbacPolicy>,
    subject_roles: HashMap<Subject, Role>,
    audit_log: Vec<RbacAuditEntry>,
    pending_approvals: Vec<(PendingApproval, ApprovalStatus)>,
    max_audit_entries: usize,
}

impl RbacManager {
    /// Creates a new RBAC manager with default policies for all roles.
    pub fn new() -> Self {
        let mut this = Self {
            policies: HashMap::new(),
            subject_roles: HashMap::new(),
            audit_log: Vec::new(),
            pending_approvals: Vec::new(),
            max_audit_entries: 10_000,
        };
        for role in [Role::User, Role::Superuser, Role::Admin] {
            this.policies.insert(role, role.default_policy());
        }
        this
    }

    /// Assigns a role to a subject.
    pub fn assign_role(&mut self, subject: Subject, role: Role) {
        self.subject_roles.insert(subject.clone(), role);
    }

    /// Revokes the role from a subject.
    pub fn revoke_role(&mut self, subject: &Subject) {
        self.subject_roles.remove(subject);
    }

    /// Returns the role assigned to a subject, if any.
    pub fn get_role(&self, subject: &Subject) -> Option<Role> {
        self.subject_roles.get(subject).copied()
    }

    /// Checks whether a subject has permission for the given action on a resource.
    pub fn check_permission(&mut self, subject: &Subject, action: &Action, resource: &str) -> bool {
        if matches!(subject, Subject::System) {
            return true;
        }
        let role = match self.subject_roles.get(subject) {
            Some(r) => *r,
            None => return false,
        };
        let policy = match self.policies.get(&role) {
            Some(p) => p,
            None => return false,
        };
        let allowed = policy.allows(action);
        self.audit_log.push(RbacAuditEntry::new(
            subject.clone(),
            action.clone(),
            resource.to_string(),
            allowed,
            if allowed { None } else { Some(format!("role {:?} does not allow {:?}", role, action)) },
        ));
        if self.audit_log.len() > self.max_audit_entries {
            self.audit_log.drain(0..self.audit_log.len() - self.max_audit_entries);
        }
        allowed
    }

    /// Creates a new approval request for a high-risk action.
    pub fn request_approval(&mut self, subject: Subject, action: Action, resource: String, reason: String) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        self.pending_approvals.push((
            PendingApproval { id, subject, action, resource, reason, created_at: Utc::now() },
            ApprovalStatus::Pending,
        ));
        id
    }

    /// Approves a pending approval request.
    pub fn approve(&mut self, id: uuid::Uuid) -> bool {
        if let Some((_, s)) = self.pending_approvals.iter_mut().find(|(p, _)| p.id == id) {
            *s = ApprovalStatus::Approved;
            return true;
        }
        false
    }

    /// Rejects a pending approval request.
    pub fn reject(&mut self, id: uuid::Uuid) -> bool {
        if let Some((_, s)) = self.pending_approvals.iter_mut().find(|(p, _)| p.id == id) {
            *s = ApprovalStatus::Rejected;
            return true;
        }
        false
    }

    /// Returns all currently pending approval requests.
    pub fn pending_approvals(&self) -> Vec<&PendingApproval> {
        self.pending_approvals.iter().filter(|(_, s)| matches!(s, ApprovalStatus::Pending)).map(|(p, _)| p).collect()
    }

    /// Returns all approval requests (pending + history) with their status.
    pub fn all_approvals(&self) -> &[(PendingApproval, ApprovalStatus)] {
        &self.pending_approvals
    }

    /// Returns the RBAC audit log.
    pub fn audit_log(&self) -> &[RbacAuditEntry] {
        &self.audit_log
    }
}

impl Default for RbacManager {
    fn default() -> Self { Self::new() }
}

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
/// // Assign agent to a garden workspace
/// access.assign_garden("code-agent", "project-alpha");
///
/// // Check permissions with sandbox enforcement
/// if access.can_access_path_in_garden("code-agent", "/workspace/file.rs", "project-alpha") {
///     // allow file access within garden
/// }
///
/// // Check if agent can access a specific garden
/// if access.can_access_garden("code-agent", "project-alpha") {
///     // allow garden access
/// }
/// ```
pub struct AccessManager {
    /// Permissions for each agent.
    permissions: HashMap<String, AgentPermissions>,
    /// Audit log of all access decisions.
    audit_log: Vec<AuditEntry>,
    /// Optional path for audit log file persistence.
    #[allow(dead_code)]
    audit_log_path: Option<std::path::PathBuf>,
    /// Maximum audit log entries to retain.
    max_audit_entries: usize,
    /// RBAC manager for HitL approvals.
    pub(crate) rbac: RbacManager,
    /// Garden workspace paths: container_name -> workspace_path.
    container_workspaces: HashMap<String, PathBuf>,
    /// Agent-to-garden assignments: agent_name -> container_name.
    agent_containers: HashMap<String, String>,
    /// Garden-to-agents mapping: container_name -> set of agent_names.
    garden_agents: HashMap<String, HashSet<String>>,
}

impl AccessManager {
    /// Creates a new access manager.
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
            audit_log: Vec::new(),
            audit_log_path: None,
            max_audit_entries: 10_000,
            rbac: RbacManager::new(),
            container_workspaces: HashMap::new(),
            agent_containers: HashMap::new(),
            garden_agents: HashMap::new(),
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
            audit_log_path: None,
            max_audit_entries,
            rbac: RbacManager::new(),
            container_workspaces: HashMap::new(),
            agent_containers: HashMap::new(),
            garden_agents: HashMap::new(),
        }
    }

    /// Configure an audit log file path for persistence.
    pub fn with_audit_log_path(mut self, path: std::path::PathBuf) -> Self {
        self.audit_log_path = Some(path);
        self
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

    /// Updates permissions for an agent using a partial update.
    ///
    /// Creates default permissions if the agent doesn't exist.
    /// Only updates fields that are Some() in the update.
    pub fn update_permissions(&mut self, agent_name: &str, update: PermissionUpdate) -> anyhow::Result<()> {
        let perms = self.permissions
            .entry(agent_name.to_string())
            .or_insert_with(|| AgentPermissions::for_new_agent(agent_name));
        update.apply(perms);
        Ok(())
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

    /// Returns a reference to the RBAC manager (for HitL approvals).
    pub fn rbac_manager(&self) -> &RbacManager {
        &self.rbac
    }

    /// Returns a mutable reference to the RBAC manager (for HitL approvals).
    pub fn rbac_manager_mut(&mut self) -> &mut RbacManager {
        &mut self.rbac
    }

    // ─── Garden Sandbox Integration ───────────────────────────────────────

    /// Registers a garden workspace path.
    ///
    /// This is used by ContainerBackend to report which paths belong to each garden.
    ///
    /// # Arguments
    /// * `container_name` - Name of the garden
    /// * `workspace_path` - Absolute path to the garden's workspace directory
    pub fn register_container_workspace(&mut self, container_name: &str, workspace_path: PathBuf) {
        self.container_workspaces.insert(container_name.to_string(), workspace_path);
        tracing::debug!(garden = %container_name, "Garden workspace registered");
    }

    /// Assigns an agent to a specific garden.
    ///
    /// After assignment, the agent is sandboxed to that garden's workspace.
    ///
    /// # Arguments
    /// * `agent_name` - Name of the agent to assign
    /// * `container_name` - Name of the garden to assign the agent to
    ///
    /// # Returns
    /// `true` if assignment succeeded, `false` if the garden doesn't exist
    pub fn assign_garden(&mut self, agent_name: &str, container_name: &str) -> bool {
        if !self.container_workspaces.contains_key(container_name) {
            tracing::warn!(agent = %agent_name, garden = %container_name, "Cannot assign agent to non-existent garden");
            return false;
        }

        // Remove from previous garden if any
        if let Some(prev_container) = self.agent_containers.get(agent_name) {
            if let Some(agents) = self.garden_agents.get_mut(prev_container) {
                agents.remove(agent_name);
            }
        }

        // Assign to new garden
        self.agent_containers.insert(agent_name.to_string(), container_name.to_string());
        self.garden_agents
            .entry(container_name.to_string())
            .or_default()
            .insert(agent_name.to_string());

        tracing::info!(agent = %agent_name, garden = %container_name, "Agent assigned to garden");
        true
    }

    /// Gets the garden name that an agent is assigned to.
    ///
    /// # Returns
    /// `Some(container_name)` if assigned, `None` if the agent is not assigned to any garden
    pub fn get_garden_for_agent(&self, agent_name: &str) -> Option<String> {
        self.agent_containers.get(agent_name).cloned()
    }

    /// Gets the workspace path for a specific garden.
    ///
    /// # Returns
    /// `Some(path)` if the garden exists, `None` otherwise
    pub fn get_container_workspace(&self, container_name: &str) -> Option<&PathBuf> {
        self.container_workspaces.get(container_name)
    }

    /// Lists all registered containers.
    pub fn list_containers(&self) -> Vec<String> {
        self.container_workspaces.keys().cloned().collect()
    }

    /// Lists all agents assigned to a specific garden.
    pub fn list_agents_in_garden(&self, container_name: &str) -> Vec<String> {
        self.garden_agents
            .get(container_name)
            .map(|agents| agents.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Checks if an agent can access a specific garden.
    ///
    /// An agent can access a garden if it is assigned to it.
    ///
    /// # Arguments
    /// * `agent_name` - Name of the agent
    /// * `container_name` - Name of the garden to check
    ///
    /// # Returns
    /// `true` if the agent is assigned to the garden, `false` otherwise
    pub fn can_access_garden(&self, agent_name: &str, container_name: &str) -> bool {
        self.agent_containers
            .get(agent_name)
            .map(|g| g == container_name)
            .unwrap_or(false)
    }

    /// Checks if a path is within a garden's workspace.
    ///
    /// This performs a canonical path comparison to ensure the path is
    /// a descendant of the garden's workspace directory.
    ///
    /// # Arguments
    /// * `container_name` - Name of the garden
    /// * `path` - Path to check (absolute or relative)
    ///
    /// # Returns
    /// `true` if the path is within the garden's workspace, `false` otherwise
    pub fn is_path_in_container_workspace(&self, container_name: &str, path: &str) -> bool {
        let workspace = match self.container_workspaces.get(container_name) {
            Some(w) => w,
            None => return false,
        };

        // Resolve the path to an absolute canonical path
        let requested_path = match Path::new(path).canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // If we can't canonicalize, try as relative to workspace
                let candidate = workspace.join(path);
                match candidate.canonicalize() {
                    Ok(p) => p,
                    Err(_) => return false,
                }
            }
        };

        // Check if the canonical path starts with the workspace
        let workspace_canonical = match workspace.canonicalize() {
            Ok(w) => w,
            Err(_) => return false,
        };

        requested_path.starts_with(&workspace_canonical)
    }

    /// Full sandbox check: RBAC → path allowed? → within garden workspace?
    ///
    /// This is the main method for enforcing sandbox boundaries. It checks:
    /// 1. RBAC - does the agent's role allow the action?
    /// 2. Path permissions - is the path in the agent's allowed_paths?
    /// 3. Garden boundary - is the path within the assigned garden's workspace?
    ///
    /// If any check fails, the access is denied and logged as "sandbox violation"
    /// if the path would be valid but outside the garden boundary.
    ///
    /// # Arguments
    /// * `agent_name` - Name of the agent
    /// * `path` - Path to access
    /// * `garden` - Garden context (if agent is assigned to one)
    ///
    /// # Returns
    /// `true` if all checks pass, `false` otherwise
    pub fn can_access_path_in_garden(
        &mut self,
        agent_name: &str,
        path: &str,
        garden: Option<&str>,
    ) -> bool {
        // First check RBAC via the agent's role
        let subject = Subject::Agent(AgentId::new_v4());
        let action = Action::AccessPath(path.to_string());
        let rbac_allowed = self.rbac.check_permission(&subject, &action, path);

        // Check path permissions (allowed_paths vs denied_paths)
        let path_allowed = self.can_access_path(agent_name, path);

        // Check garden workspace boundary
        let garden_allowed = if let Some(container_name) = garden {
            let is_in_workspace = self.is_path_in_container_workspace(container_name, path);

            if !is_in_workspace {
                // Log as sandbox violation
                self.log_access(
                    agent_name,
                    "sandbox_violation",
                    path,
                    false,
                    Some(format!(
                        "Path '{}' is outside garden '{}' workspace boundary",
                        path, container_name
                    )),
                );
            }

            is_in_workspace
        } else {
            // No garden context - check if agent has any garden assignment
            if let Some(assigned_container) = self.agent_containers.get(agent_name) {
                let is_in_workspace = self.is_path_in_container_workspace(assigned_container, path);

                if !is_in_workspace {
                    self.log_access(
                        agent_name,
                        "sandbox_violation",
                        path,
                        false,
                        Some(format!(
                            "Path '{}' is outside assigned garden '{}' workspace boundary",
                            path, assigned_container
                        )),
                    );
                }

                is_in_workspace
            } else {
                // Agent has no garden assignment - default to allowing path check only
                true
            }
        };

        // All three checks must pass
        rbac_allowed && path_allowed && garden_allowed
    }

    /// Unassigns an agent from its garden (if any).
    ///
    /// The agent will no longer be sandboxed to any garden workspace.
    pub fn unassign_garden(&mut self, agent_name: &str) -> Option<String> {
        if let Some(container_name) = self.agent_containers.remove(agent_name) {
            if let Some(agents) = self.garden_agents.get_mut(&container_name) {
                agents.remove(agent_name);
            }
            tracing::info!(agent = %agent_name, garden = %container_name, "Agent unassigned from garden");
            Some(container_name)
        } else {
            None
        }
    }

    /// Removes a garden and unassigns all agents from it.
    ///
    /// All agents assigned to this garden will have their garden assignments cleared.
    pub fn remove_container(&mut self, container_name: &str) {
        // Unassign all agents from this garden
        if let Some(agents) = self.garden_agents.remove(container_name) {
            for agent_name in agents {
                self.agent_containers.remove(&agent_name);
            }
        }

        // Remove the workspace path
        self.container_workspaces.remove(container_name);

        tracing::info!(garden = %container_name, "Garden removed from access manager");
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
    /// Persists to file if audit_log_path is configured.
    pub(crate) fn log_access(
        &mut self,
        agent_name: &str,
        action: &str,
        resource: &str,
        allowed: bool,
        reason: Option<String>,
    ) {
        let entry = AuditEntry::new(agent_name, action, resource, allowed, reason.clone());

        self.audit_log.push(entry.clone());

        // Prune if needed.
        if self.audit_log.len() > self.max_audit_entries {
            let prune_count = self.audit_log.len() - self.max_audit_entries;
            self.audit_log.drain(0..prune_count);
        }

        // Persist to file (fire-and-forget).
        self.persist_audit_entry(&entry);

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

    /// Persists an audit entry to the configured log file.
    /// Runs in a background thread to avoid blocking the main path.
    fn persist_audit_entry(&self, entry: &AuditEntry) {
        let path = match &self.audit_log_path {
            Some(p) => p.clone(),
            None => return,
        };
        let line = match serde_json::to_string(entry) {
            Ok(s) => s,
            Err(_) => return,
        };
        let path_for_thread = path;
        std::thread::spawn(move || {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path_for_thread)
            {
                let _ = writeln!(f, "{}", line);
            }
        });
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

    // --- Garden Sandbox tests ---

    #[test]
    fn test_register_container_workspace() {
        let mut access = AccessManager::new();
        access.register_container_workspace("my-container", PathBuf::from("/workspace/my-container"));

        assert_eq!(access.list_containers(), vec!["my-container"]);
        assert_eq!(access.get_container_workspace("my-container"), Some(&PathBuf::from("/workspace/my-container")));
    }

    #[test]
    fn test_assign_agent_to_garden() {
        let mut access = AccessManager::new();
        access.register_container_workspace("project-alpha", PathBuf::from("/workspace/alpha"));

        // Assign agent to garden
        assert!(access.assign_garden("agent-1", "project-alpha"));

        // Check agent is assigned
        assert_eq!(access.get_garden_for_agent("agent-1"), Some("project-alpha".to_string()));
        assert!(access.can_access_garden("agent-1", "project-alpha"));
        assert!(!access.can_access_garden("agent-1", "other-garden"));
    }

    #[test]
    fn test_assign_agent_to_nonexistent_garden_fails() {
        let mut access = AccessManager::new();

        // Cannot assign to non-existent garden
        assert!(!access.assign_garden("agent-1", "nonexistent"));
        assert_eq!(access.get_garden_for_agent("agent-1"), None);
    }

    #[test]
    fn test_reassign_agent_to_different_garden() {
        let mut access = AccessManager::new();
        access.register_container_workspace("container-a", PathBuf::from("/workspace/a"));
        access.register_container_workspace("container-b", PathBuf::from("/workspace/b"));

        // Assign to first garden
        access.assign_garden("agent-1", "container-a");
        assert_eq!(access.get_garden_for_agent("agent-1"), Some("container-a".to_string()));

        // Reassign to second garden
        access.assign_garden("agent-1", "container-b");
        assert_eq!(access.get_garden_for_agent("agent-1"), Some("container-b".to_string()));

        // Agent should not be in first garden anymore
        assert!(!access.can_access_garden("agent-1", "container-a"));
    }

    #[test]
    fn test_unassign_agent_from_garden() {
        let mut access = AccessManager::new();
        access.register_container_workspace("my-container", PathBuf::from("/workspace/my"));

        access.assign_garden("agent-1", "my-container");
        assert!(access.get_garden_for_agent("agent-1").is_some());

        let removed = access.unassign_garden("agent-1");
        assert_eq!(removed, Some("my-container".to_string()));
        assert!(access.get_garden_for_agent("agent-1").is_none());
    }

    #[test]
    fn test_list_agents_in_garden() {
        let mut access = AccessManager::new();
        access.register_container_workspace("my-container", PathBuf::from("/workspace/my"));

        access.assign_garden("agent-1", "my-container");
        access.assign_garden("agent-2", "my-container");
        access.assign_garden("agent-3", "other-garden");

        let agents = access.list_agents_in_garden("my-container");
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&"agent-1".to_string()));
        assert!(agents.contains(&"agent-2".to_string()));
        assert!(!agents.contains(&"agent-3".to_string()));
    }

    #[test]
    fn test_remove_container_unassigns_all_agents() {
        let mut access = AccessManager::new();
        access.register_container_workspace("my-container", PathBuf::from("/workspace/my"));

        access.assign_garden("agent-1", "my-container");
        access.assign_garden("agent-2", "my-container");

        access.remove_container("my-container");

        assert!(access.list_containers().is_empty());
        assert!(access.get_garden_for_agent("agent-1").is_none());
        assert!(access.get_garden_for_agent("agent-2").is_none());
    }

    #[test]
    fn test_is_path_in_container_workspace() {
        let mut access = AccessManager::new();

        // Use /tmp for testing - it should exist on most systems
        let workspace = PathBuf::from("/tmp/oxios-test-workspace");

        // Create temp directories BEFORE registering (so canonicalize works)
        std::fs::create_dir_all(&workspace).ok();
        std::fs::create_dir_all(workspace.join("subdir")).ok();

        // Now register the garden workspace
        access.register_container_workspace("my-container", workspace.clone());

        // Path inside workspace
        let inside_path = workspace.join("file.txt");
        std::fs::write(&inside_path, "test").ok(); // Create the file too

        assert!(
            access.is_path_in_container_workspace("my-container", inside_path.to_str().unwrap()),
            "Path {:?} should be inside workspace",
            inside_path
        );

        let nested_path = workspace.join("subdir/nested.txt");
        std::fs::write(&nested_path, "test").ok();
        assert!(
            access.is_path_in_container_workspace("my-container", nested_path.to_str().unwrap()),
            "Path {:?} should be inside workspace",
            nested_path
        );

        // Path outside workspace (use /tmp directly without our subdirectory)
        assert!(!access.is_path_in_container_workspace("my-container", "/tmp/other-workspace/file.txt"));

        // Non-existent garden
        assert!(!access.is_path_in_container_workspace("nonexistent", "/tmp/test"));

        // Cleanup
        std::fs::remove_dir_all(workspace).ok();
    }
}