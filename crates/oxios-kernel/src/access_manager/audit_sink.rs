//! Unified audit sink — single destination for all security events.
//!
//! Eliminates the three-way split between `AccessManager.audit_log`,
//! `RbacManager.audit_log`, and `AuditTrail`. All security events
//! flow through `AuditSink` into the Merkle chain and JSONL file.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::audit_trail::{AuditAction, AuditTrail};

// ─── Audit Event ────────────────────────────────────────────────────────────

/// Unified security audit event.
///
/// Every security-relevant decision produces one of these variants.
/// Serialized as JSONL for file persistence and ingested into the
/// Merkle-chain `AuditTrail` for tamper-evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AuditEvent {
    /// Tool access decision.
    ToolAccess {
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: DateTime<Utc>,
        agent: String,
        tool: String,
        allowed: bool,
        layer: Option<String>,
        reason: Option<String>,
    },
    /// Path access decision.
    PathAccess {
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: DateTime<Utc>,
        agent: String,
        path: String,
        mode: String,
        allowed: bool,
        layer: Option<String>,
        reason: Option<String>,
    },
    /// Command execution decision.
    ExecAccess {
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: DateTime<Utc>,
        agent: String,
        binary: String,
        allowed: bool,
        layer: Option<String>,
        reason: Option<String>,
    },
    /// RBAC authorization decision.
    RbacDecision {
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: DateTime<Utc>,
        subject: String,
        action: String,
        resource: String,
        allowed: bool,
        reason: Option<String>,
    },
    /// Workspace sandbox violation.
    SandboxViolation {
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: DateTime<Utc>,
        agent: String,
        path: String,
        workspace: String,
    },
    /// Human-in-the-loop approval event.
    Approval {
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: DateTime<Utc>,
        approval_id: String,
        subject: String,
        action: String,
        status: String,
    },
}

impl AuditEvent {
    /// Returns the agent/subject responsible for this event.
    pub fn actor(&self) -> &str {
        match self {
            AuditEvent::ToolAccess { agent, .. } => agent,
            AuditEvent::PathAccess { agent, .. } => agent,
            AuditEvent::ExecAccess { agent, .. } => agent,
            AuditEvent::RbacDecision { subject, .. } => subject,
            AuditEvent::SandboxViolation { agent, .. } => agent,
            AuditEvent::Approval { subject, .. } => subject,
        }
    }

    /// Convert to an AuditAction for the Merkle-chain AuditTrail.
    pub fn to_audit_action(&self) -> AuditAction {
        match self {
            AuditEvent::ToolAccess {
                tool, allowed, ..
            } => AuditAction::Other {
                detail: format!("tool_access:{}:allowed={}", tool, allowed),
            },
            AuditEvent::PathAccess {
                path, mode, allowed, ..
            } => AuditAction::Other {
                detail: format!("path_access:{}:{}:allowed={}", path, mode, allowed),
            },
            AuditEvent::ExecAccess {
                binary, allowed, ..
            } => AuditAction::Other {
                detail: format!("exec_access:{}:allowed={}", binary, allowed),
            },
            AuditEvent::RbacDecision {
                subject,
                action,
                allowed,
                ..
            } => AuditAction::Other {
                detail: format!("rbac:{}:{}:allowed={}", subject, action, allowed),
            },
            AuditEvent::SandboxViolation {
                agent, path, workspace, ..
            } => AuditAction::Other {
                detail: format!("sandbox_violation:{}:{}:ws={}", agent, path, workspace),
            },
            AuditEvent::Approval {
                approval_id,
                status,
                ..
            } => AuditAction::Other {
                detail: format!("approval:{}:{}", approval_id, status),
            },
        }
    }

    #[cfg(test)]
    fn now() -> DateTime<Utc> {
        Utc::now()
    }
}

// ─── Audit Sink Trait ───────────────────────────────────────────────────────

/// Destination for all security audit events.
///
/// Implementations persist events to Merkle chain + file, or are no-ops for tests.
pub trait AuditSink: Send + Sync {
    /// Record a security audit event.
    fn record(&self, event: AuditEvent);
}

// ─── Trail Audit Sink ───────────────────────────────────────────────────────

/// Production audit sink: Merkle chain + async JSONL file writer.
///
/// Events are:
/// 1. Appended to the `AuditTrail` (Merkle chain, tamper-evident)
/// 2. Sent to a background file writer via bounded channel (JSONL)
///
/// If the channel is full, a warning is logged and the event is still
/// recorded in the Merkle chain (just not persisted to file immediately).
pub struct TrailAuditSink {
    /// Merkle-chain audit trail — always succeeds (in-memory).
    trail: Arc<AuditTrail>,
    /// Bounded channel to background file writer.
    file_tx: tokio::sync::mpsc::Sender<String>,
}

impl TrailAuditSink {
    /// Create a new `TrailAuditSink`.
    ///
    /// Spawns a background tokio task that reads from the bounded channel
    /// and appends JSONL entries to `audit_path`.
    pub fn new(trail: Arc<AuditTrail>, audit_path: PathBuf) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1000);

        let path = audit_path.clone();
        tokio::spawn(async move {
            if let Ok(mut file) = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await
            {
                use tokio::io::AsyncWriteExt;
                while let Some(line) = rx.recv().await {
                    let _ = file.write_all(line.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            }
        });

        Self {
            trail,
            file_tx: tx,
        }
    }
}

impl AuditSink for TrailAuditSink {
    fn record(&self, event: AuditEvent) {
        // 1. Merkle chain (always succeeds)
        let actor = event.actor().to_string();
        let action = event.to_audit_action();
        self.trail.append(actor, action, "access_gate".into());

        // 2. JSONL file (fire-and-forget, may drop if channel full)
        if let Ok(line) = serde_json::to_string(&event) {
            match self.file_tx.try_send(line) {
                Ok(()) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!("Audit sink channel full — event still in Merkle chain");
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    tracing::warn!("Audit sink channel closed");
                }
            }
        }
    }
}

// ─── No-op Sink (tests) ────────────────────────────────────────────────────

/// No-op audit sink for tests — discards all events.
#[cfg(test)]
pub struct NoOpAuditSink;

#[cfg(test)]
impl AuditSink for NoOpAuditSink {
    fn record(&self, _event: AuditEvent) {}
}

/// Minimal audit sink that logs to tracing — used as default when no file sink is configured.
pub struct TracingAuditSink;

impl AuditSink for TracingAuditSink {
    fn record(&self, event: AuditEvent) {
        if let AuditEvent::ToolAccess { agent, tool, allowed: false, layer, .. } = &event {
            tracing::warn!(
                agent = %agent,
                tool = %tool,
                layer = ?layer,
                "Access denied (no persistent audit sink configured)"
            );
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_access_event() {
        let event = AuditEvent::ToolAccess {
            timestamp: AuditEvent::now(),
            agent: "test-agent".into(),
            tool: "exec".into(),
            allowed: true,
            layer: None,
            reason: None,
        };
        assert_eq!(event.actor(), "test-agent");
        let action = event.to_audit_action();
        assert!(matches!(action, AuditAction::Other { .. }));
    }

    #[test]
    fn test_rbac_decision_event() {
        let event = AuditEvent::RbacDecision {
            timestamp: AuditEvent::now(),
            subject: "user:alice".into(),
            action: "UseTool(exec)".into(),
            resource: "exec".into(),
            allowed: false,
            reason: Some("role User does not allow".into()),
        };
        assert_eq!(event.actor(), "user:alice");
    }

    #[test]
    fn test_sandbox_violation_event() {
        let event = AuditEvent::SandboxViolation {
            timestamp: AuditEvent::now(),
            agent: "rogue-agent".into(),
            path: "/etc/passwd".into(),
            workspace: "project-alpha".into(),
        };
        assert_eq!(event.actor(), "rogue-agent");
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let event = AuditEvent::ExecAccess {
            timestamp: AuditEvent::now(),
            agent: "test".into(),
            binary: "git".into(),
            allowed: true,
            layer: None,
            reason: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ExecAccess"));
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, AuditEvent::ExecAccess { .. }));
    }

    #[test]
    fn test_noop_sink() {
        let sink = NoOpAuditSink;
        sink.record(AuditEvent::ToolAccess {
            timestamp: AuditEvent::now(),
            agent: "test".into(),
            tool: "exec".into(),
            allowed: true,
            layer: None,
            reason: None,
        });
        // No panic = success
    }
}
