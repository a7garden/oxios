//! Security API — authentication, audit trail, RBAC, approvals.

use crate::access_manager::{
    AccessManager, AgentPermissions, ApprovalStatus, PendingApproval, PermissionUpdate,
};
use crate::audit_trail::{AuditAction, AuditEntry, AuditTrail};
use crate::auth::AuthManager;
use crate::state_store::StateStore;
use std::collections::HashMap;
use std::sync::Arc;

/// A one-time ticket for WebSocket authentication.
/// Valid for 30 seconds after creation, single-use.
struct WsTicket {
    created_at: std::time::Instant,
}

/// Security system calls.
pub struct SecurityApi {
    pub(crate) auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
    pub(crate) audit_trail: Arc<AuditTrail>,
    pub(crate) access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    pub(crate) state_store: Arc<StateStore>,
    ws_tickets: Arc<parking_lot::Mutex<HashMap<String, WsTicket>>>,
}

impl SecurityApi {
    /// Create a new SecurityApi.
    pub fn new(
        auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
        audit_trail: Arc<AuditTrail>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        state_store: Arc<StateStore>,
    ) -> Self {
        Self {
            auth_manager,
            audit_trail,
            access_manager,
            state_store,
            ws_tickets: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    /// Generate a one-time WebSocket ticket. Valid for 30 seconds, single-use.
    pub fn generate_ws_ticket(&self) -> String {
        let bytes: [u8; 16] = *uuid::Uuid::new_v4().as_bytes();
        let ticket = format!("wst_{}", hex::encode(bytes));
        let mut tickets = self.ws_tickets.lock();
        // Prune expired tickets (older than 60s)
        tickets.retain(|_, t| t.created_at.elapsed().as_secs() < 60);
        tickets.insert(
            ticket.clone(),
            WsTicket {
                created_at: std::time::Instant::now(),
            },
        );
        ticket
    }

    /// Validate and consume a one-time WebSocket ticket. Returns false if invalid/expired/already used.
    pub fn validate_ws_ticket(&self, ticket: &str) -> bool {
        let mut tickets = self.ws_tickets.lock();
        if let Some(t) = tickets.remove(ticket) {
            t.created_at.elapsed().as_secs() < 30
        } else {
            false
        }
    }
    /// Audit an action.
    pub fn audit(&self, actor: &str, action: AuditAction, resource: &str) -> String {
        self.audit_trail
            .append(actor.to_string(), action, resource.to_string())
    }

    /// Verify audit chain integrity.
    pub fn verify_chain(&self) -> anyhow::Result<bool> {
        self.audit_trail
            .verify()
            .map_err(|e| anyhow::anyhow!("audit verify failed: {:?}", e))
    }

    /// Query audit entries by sequence range.
    pub fn query_audit(&self, from_seq: u64, to_seq: u64) -> Vec<AuditEntry> {
        self.audit_trail.entries(from_seq, to_seq)
    }

    /// Query audit by agent.
    pub fn query_audit_by_agent(&self, agent_id: &str) -> Vec<AuditEntry> {
        self.audit_trail.by_agent(agent_id)
    }

    /// Get audit entry count.
    pub fn audit_count(&self) -> usize {
        self.audit_trail.len()
    }

    /// Flush audit trail to disk and commit to git.
    ///
    /// Persists all in-memory audit entries to the state store,
    /// then commits the audit file to git for versioning.
    pub fn flush(&self, git: &crate::git_layer::GitLayer) -> anyhow::Result<()> {
        // 1. Persist entries to state store
        self.audit_trail.flush(&self.state_store)?;
        // 2. Commit to git
        if git.is_enabled() {
            let _ = git.commit_file("audit", "audit trail flush");
        }
        Ok(())
    }

    /// Validate a bearer token.
    pub fn validate_token(&self, token: &str) -> bool {
        self.auth_manager.lock().validate(token)
    }

    /// Get audit log entries from access manager.
    pub fn get_audit_log(&self) -> Vec<crate::access_manager::AuditEntry> {
        self.access_manager.lock().audit_log().to_vec()
    }

    /// Get permissions for an agent.
    pub fn get_permissions(&self, agent: &str) -> Option<AgentPermissions> {
        self.access_manager.lock().get_permissions(agent).cloned()
    }

    /// Ensure permissions exist for an agent (get or create).
    pub fn ensure_permissions(&self, agent: &str) -> AgentPermissions {
        self.access_manager
            .lock()
            .get_or_create_permissions(agent)
            .clone()
    }

    /// Update permissions for an agent.
    pub fn update_permissions(&self, agent: &str, update: PermissionUpdate) -> anyhow::Result<()> {
        self.access_manager.lock().update_permissions(agent, update)
    }

    /// Log an audit action.
    pub fn log_action(&self, agent_name: &str, action: &str, resource: &str) {
        let mut am = self.access_manager.lock();
        am.log_access(agent_name, action, resource, true, None);
    }

    /// List all pending approvals.
    pub fn list_approvals(&self) -> Vec<(PendingApproval, ApprovalStatus)> {
        self.access_manager
            .lock()
            .rbac_manager()
            .all_approvals()
            .to_vec()
    }

    /// Approve a pending request.
    pub fn approve(&self, id: uuid::Uuid) -> bool {
        self.access_manager.lock().rbac_manager_mut().approve(id)
    }

    /// Reject a pending request.
    pub fn reject(&self, id: uuid::Uuid) -> bool {
        self.access_manager.lock().rbac_manager_mut().reject(id)
    }
}
