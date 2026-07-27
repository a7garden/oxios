//! Security API — authentication, audit trail, RBAC, approvals.

use crate::access_manager::{
    AccessManager, AgentPermissions, ApprovalStatus, PendingApproval, PermissionUpdate,
};
use crate::auth::AuthManager;
use crate::state_store::StateStore;
use oxi_sdk::observability::{AuditAction, AuditTrail, TrailEntry};
use std::collections::HashMap;
use std::sync::Arc;

/// A one-time ticket for WebSocket authentication.
struct WsTicket {
    created_at: std::time::Instant,
}

/// How long a [`WsTicket`] is considered valid during `validate_ws_ticket`.
/// Single-use: the ticket is removed from the map on first validation.
const WS_TICKET_TTL_SECS: u64 = 30;
/// Prune threshold used inside `generate_ws_ticket`. Slightly longer than
/// [`WS_TICKET_TTL_SECS`] so an expired-but-not-yet-consumed ticket is
/// cleared from memory on the next generate, rather than lingering until
/// process exit.
const WS_TICKET_PRUNE_AFTER_SECS: u64 = 60;

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

    /// Generate a one-time WebSocket ticket.
    ///
    /// The ticket is valid for [`WS_TICKET_TTL_SECS`] seconds (single-use).
    /// Pruning removes entries older than [`WS_TICKET_PRUNE_AFTER_SECS`]
    /// seconds — the prune window is intentionally a bit longer than the
    /// validate window so a ticket that has just expired is still cleared
    /// from memory on the next generate.
    pub fn generate_ws_ticket(&self) -> String {
        let bytes: [u8; 16] = *uuid::Uuid::new_v4().as_bytes();
        let ticket = format!("wst_{}", hex::encode(bytes));
        let mut tickets = self.ws_tickets.lock();
        // Prune expired tickets.
        tickets.retain(|_, t| t.created_at.elapsed().as_secs() < WS_TICKET_PRUNE_AFTER_SECS);
        tickets.insert(
            ticket.clone(),
            WsTicket {
                created_at: std::time::Instant::now(),
            },
        );
        ticket
    }

    /// Validate and consume a one-time WebSocket ticket. Returns false if
    /// invalid/expired/already-used.
    pub fn validate_ws_ticket(&self, ticket: &str) -> bool {
        let mut tickets = self.ws_tickets.lock();
        if let Some(t) = tickets.remove(ticket) {
            t.created_at.elapsed().as_secs() < WS_TICKET_TTL_SECS
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
            .map_err(|e| anyhow::anyhow!("audit verify failed: {e:?}"))
    }

    /// Query audit entries by sequence range.
    pub fn query_audit(&self, from_seq: u64, to_seq: u64) -> Vec<TrailEntry> {
        self.audit_trail.entries(from_seq, to_seq)
    }

    /// Query audit entries whose agent/subject matches `agent_id`.
    /// Field access is serde-based so this is robust to `TrailEntry` field
    /// renames in oxi-sdk.
    pub fn query_audit_by_agent(&self, agent_id: &str) -> Vec<TrailEntry> {
        self.audit_trail
            .entries(0, u64::MAX)
            .into_iter()
            .filter(|e| {
                serde_json::to_value(e)
                    .ok()
                    .and_then(|v| {
                        v.get("agent")
                            .or_else(|| v.get("subject"))
                            .or_else(|| v.get("agent_id"))
                            .and_then(|s| s.as_str())
                            .map(|s| s == agent_id)
                    })
                    .unwrap_or(false)
            })
            .collect()
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
        // 1. Persist entries to state store via AuditPersistence trait
        self.audit_trail.flush_to(self.state_store.as_ref())?;
        // 2. Commit to git. Unlike best-effort commits in save_and_commit
        //    (where the on-disk save already succeeded), audit trail commits
        //    are compliance-relevant: surface the failure so operators know
        //    the audit record is not versioned.
        if git.is_enabled() {
            git.commit_file("audit", "audit trail flush")?;
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
