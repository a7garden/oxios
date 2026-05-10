//! RBAC types and manager — role-based access control with HitL approvals.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::types::AgentId;

// ─── RBAC Types ───────────────────────────────────────────────────────────────

/// Roles for role-based access control (3-tier model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Basic user — can use agents, limited permissions.
    User,
    /// Superuser — can manage programs, skills, workspaces.
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
                    Action::ManageWorkspaces,
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
                    Action::ManageWorkspaces,
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
    /// Manage workspaces (create/start/stop/remove).
    ManageWorkspaces,
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
    /// Creates a new RBAC audit entry.
    pub(crate) fn new(subject: Subject, action: Action, resource: String, allowed: bool, reason: Option<String>) -> Self {
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
#[derive(Debug, Clone)]
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
