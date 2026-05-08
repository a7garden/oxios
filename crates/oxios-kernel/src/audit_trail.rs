//! Tamper-evident audit trail with cryptographic hash chain.
//!
//! Provides a Merkle-chain style audit log for all kernel events.
//! Each entry is cryptographically linked to the previous entry,
//! making tampering detectable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state_store::StateStore;

/// Type alias for hash digest (blake3 hex output).
pub type HashDigest = String;

/// Unique identifier for an agent (String for flexibility).
pub type AgentId = String;

// ─── Error Types ─────────────────────────────────────────────────────────────

/// Errors that can occur during audit trail operations.
#[derive(Debug, Clone)]
pub enum AuditError {
    /// Chain link broken at given sequence number.
    ChainBroken {
        seq: u64,
        expected: String,
        found: String,
    },
    /// Invalid timestamp detected.
    InvalidTimestamp { seq: u64 },
    /// Failed to export audit log.
    ExportFailed(String),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::ChainBroken {
                seq,
                expected,
                found,
            } => {
                write!(
                    f,
                    "chain broken at seq {}: expected hash '{}', found '{}'",
                    seq, expected, found
                )
            }
            AuditError::InvalidTimestamp { seq } => {
                write!(f, "invalid timestamp at seq {}", seq)
            }
            AuditError::ExportFailed(msg) => {
                write!(f, "export failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for AuditError {}

// ─── Audit Action ─────────────────────────────────────────────────────────────

/// Types of actions that can be audited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum AuditAction {
    /// Agent spawned with task type.
    AgentSpawn { task_type: String },
    /// Agent exited with reason.
    AgentExit { reason: String },
    /// Tool was called.
    ToolCall { tool: String, args_json: String },
    /// Tool returned a result.
    ToolResult { tool: String, success: bool },
    /// Memory entry written.
    MemoryWrite { entry_id: String },
    /// Memory entry read.
    MemoryRead { entry_id: String },
    /// Configuration changed.
    ConfigChange { key: String },
    /// Container started.
    ContainerStart { container_id: String },
    /// Container stopped.
    ContainerStop { container_id: String },
    /// Program installed.
    ProgramInstall { program: String, version: String },
    /// Cron job triggered.
    CronTrigger { job_id: String },
    /// Git commit created.
    GitCommit { message: String },
    /// Access was denied.
    AccessDenied { permission: String },
    /// Other/unclassified action.
    Other { detail: String },
}

// ─── Audit Entry ─────────────────────────────────────────────────────────────

/// A single entry in the audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Sequential entry number.
    pub seq: u64,
    /// Timestamp of the entry.
    pub timestamp: DateTime<Utc>,
    /// Agent ID that performed the action.
    pub actor: AgentId,
    /// The action that was performed.
    pub action: AuditAction,
    /// Resource affected by the action.
    pub resource: String,
    /// Hash of the previous entry (empty string for genesis).
    pub prev_hash: HashDigest,
    /// Hash of this entry.
    pub hash: HashDigest,
    /// Optional arbitrary metadata.
    pub metadata: Option<serde_json::Value>,
}

// ─── Hash Computation ──────────────────────────────────────────────────────────

/// Compute the hash for an audit entry.
/// Uses blake3 to hash all entry fields in a deterministic way.
fn compute_entry_hash(
    seq: u64,
    ts: &DateTime<Utc>,
    actor: &str,
    action: &AuditAction,
    resource: &str,
    prev: &str,
) -> HashDigest {
    use blake3::Hasher;

    let mut h = Hasher::new();
    h.update(b"oxios-audit-v1");
    h.update(&seq.to_be_bytes());
    h.update(ts.to_rfc3339().as_bytes());
    h.update(actor.as_bytes());

    // Serialize action to bytes for hashing
    let action_bytes = serde_json::to_vec(action).unwrap_or_default();
    h.update(&action_bytes);
    h.update(prev.as_bytes());
    h.update(resource.as_bytes());

    h.finalize().to_hex().to_string()
}

// ─── Audit Trail ─────────────────────────────────────────────────────────────

/// A tamper-evident audit trail with cryptographic hash chain.
/// 
/// Each entry is cryptographically linked to the previous entry using
/// blake3 hashing. This makes it possible to detect any tampering with
/// historical entries.
pub struct AuditTrail {
    /// All audit entries in order.
    entries: parking_lot::RwLock<Vec<AuditEntry>>,
    /// Sequence number counter for next entry.
    seq_counter: AtomicU64,
    /// Chain hasher for computing hashes (mutex for interior mutability).
    chain_hasher: parking_lot::Mutex<blake3::Hasher>,
    /// Maximum number of entries before auto-pruning.
    max_entries: usize,
}

impl AuditTrail {
    /// Create a new audit trail.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: parking_lot::RwLock::new(Vec::new()),
            seq_counter: AtomicU64::new(1), // Start at 1, 0 is genesis marker
            chain_hasher: parking_lot::Mutex::new(blake3::Hasher::new()),
            max_entries,
        }
    }

    /// Get the current number of entries.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the trail is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the last hash in the chain.
    fn last_hash(&self) -> HashDigest {
        let entries = self.entries.read();
        entries
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| "genesis".to_string())
    }

    /// Append an audit entry. Computes hash chain automatically.
    pub fn append(
        &self,
        actor: AgentId,
        action: AuditAction,
        resource: String,
    ) -> HashDigest {
        self.append_with_meta(actor, action, resource, None)
    }

    /// Append an audit entry with optional metadata.
    pub fn append_with_meta(
        &self,
        actor: AgentId,
        action: AuditAction,
        resource: String,
        metadata: Option<serde_json::Value>,
    ) -> HashDigest {
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        let timestamp = Utc::now();
        let prev_hash = self.last_hash();
        let hash = compute_entry_hash(seq, &timestamp, &actor, &action, &resource, &prev_hash);

        let entry = AuditEntry {
            seq,
            timestamp,
            actor,
            action,
            resource,
            prev_hash,
            hash,
            metadata,
        };

        let entry_hash = entry.hash.clone();

        {
            let mut entries = self.entries.write();
            entries.push(entry);
            
            // Auto-prune if over limit
            if entries.len() > self.max_entries {
                let excess = entries.len() - self.max_entries;
                entries.drain(0..excess);
            }
        }

        entry_hash
    }

    /// Verify the integrity of the hash chain.
    pub fn verify(&self) -> Result<bool, AuditError> {
        let entries = self.entries.read();
        let mut prev_hash = "genesis".to_string();

        for entry in entries.iter() {
            // Check sequence is correct
            if entry.seq == 0 {
                return Err(AuditError::ChainBroken {
                    seq: 0,
                    expected: "non-zero sequence".to_string(),
                    found: "0".to_string(),
                });
            }

            // Check prev_hash matches
            if entry.prev_hash != prev_hash {
                return Err(AuditError::ChainBroken {
                    seq: entry.seq,
                    expected: prev_hash,
                    found: entry.prev_hash.clone(),
                });
            }

            // Verify timestamp is not in the future
            let now = Utc::now();
            if entry.timestamp > now {
                return Err(AuditError::InvalidTimestamp { seq: entry.seq });
            }

            // Recompute hash and verify
            let computed = compute_entry_hash(
                entry.seq,
                &entry.timestamp,
                &entry.actor,
                &entry.action,
                &entry.resource,
                &entry.prev_hash,
            );

            if computed != entry.hash {
                return Err(AuditError::ChainBroken {
                    seq: entry.seq,
                    expected: computed,
                    found: entry.hash.clone(),
                });
            }

            prev_hash = entry.hash.clone();
        }

        Ok(true)
    }

    /// Get entries within a sequence range (inclusive).
    pub fn entries(&self, from_seq: u64, to_seq: u64) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| e.seq >= from_seq && e.seq <= to_seq)
            .cloned()
            .collect()
    }

    /// Get all entries.
    pub fn all_entries(&self) -> Vec<AuditEntry> {
        self.entries.read().clone()
    }

    /// Query entries by agent ID.
    pub fn by_agent(&self, agent_id: &str) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| e.actor == agent_id)
            .cloned()
            .collect()
    }

    /// Query entries by action type.
    pub fn by_action(&self, action: &AuditAction) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| &e.action == action)
            .cloned()
            .collect()
    }

    /// Query entries by action discriminant (for faster lookup).
    pub fn by_action_type(&self, type_name: &str) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| {
                let action_name = match &e.action {
                    AuditAction::AgentSpawn { .. } => "AgentSpawn",
                    AuditAction::AgentExit { .. } => "AgentExit",
                    AuditAction::ToolCall { .. } => "ToolCall",
                    AuditAction::ToolResult { .. } => "ToolResult",
                    AuditAction::MemoryWrite { .. } => "MemoryWrite",
                    AuditAction::MemoryRead { .. } => "MemoryRead",
                    AuditAction::ConfigChange { .. } => "ConfigChange",
                    AuditAction::ContainerStart { .. } => "ContainerStart",
                    AuditAction::ContainerStop { .. } => "ContainerStop",
                    AuditAction::ProgramInstall { .. } => "ProgramInstall",
                    AuditAction::CronTrigger { .. } => "CronTrigger",
                    AuditAction::GitCommit { .. } => "GitCommit",
                    AuditAction::AccessDenied { .. } => "AccessDenied",
                    AuditAction::Other { .. } => "Other",
                };
                action_name == type_name
            })
            .cloned()
            .collect()
    }

    /// Export entries from a sequence number as JSON.
    pub fn export_json(&self, from_seq: u64) -> Result<String, AuditError> {
        let entries = self.entries.read();
        let filtered: Vec<&AuditEntry> = entries
            .iter()
            .filter(|e| e.seq >= from_seq)
            .collect();

        serde_json::to_string_pretty(&filtered).map_err(|e| AuditError::ExportFailed(e.to_string()))
    }

    /// Export all entries as JSON.
    pub fn export_all_json(&self) -> Result<String, AuditError> {
        let entries = self.entries.read();
        serde_json::to_string_pretty(&*entries).map_err(|e| AuditError::ExportFailed(e.to_string()))
    }

    /// Flush entries to the state store for persistence.
    pub fn flush(&self, state_store: &StateStore) -> Result<(), AuditError> {
        let entries = self.entries.read();
        state_store
            .save_audit_entries(&entries)
            .map_err(|e| AuditError::ExportFailed(e.to_string()))
    }
}

impl Default for AuditTrail {
    fn default() -> Self {
        Self::new(100_000)
    }
}

impl std::fmt::Debug for AuditTrail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditTrail")
            .field("entries", &self.len())
            .field("seq_counter", &self.seq_counter)
            .field("max_entries", &self.max_entries)
            .finish()
    }
}

// ─── StateStore Extension ─────────────────────────────────────────────────────

use anyhow::Result;

impl StateStore {
    /// Save audit entries to the state store.
    pub fn save_audit_entries(&self, entries: &[AuditEntry]) -> Result<()> {
        let path = self.audit_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entries)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load audit entries from the state store.
    pub fn load_audit_entries(&self) -> Result<Vec<AuditEntry>> {
        let path = self.audit_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let json = std::fs::read_to_string(&path)?;
        let entries: Vec<AuditEntry> = serde_json::from_str(&json)?;
        Ok(entries)
    }

    /// Get the path to the audit trail file.
    fn audit_path(&self) -> std::path::PathBuf {
        self.base_path.join("audit").join("trail.json")
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_trail() -> AuditTrail {
        AuditTrail::new(1000)
    }

    #[test]
    fn test_append_generates_hash() {
        let trail = create_test_trail();
        let hash = trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // blake3 hex is 64 chars
    }

    #[test]
    fn test_append_increments_seq() {
        let trail = create_test_trail();

        let h1 = trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        let h2 = trail.append(
            "agent-002".to_string(),
            AuditAction::ToolCall {
                tool: "bash".to_string(),
                args_json: "{}".to_string(),
            },
            "/test/resource2".to_string(),
        );

        assert_ne!(h1, h2);

        let entries = trail.all_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].seq, 1);
        assert_eq!(entries[1].seq, 2);
    }

    #[test]
    fn test_hash_chain_linked() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentExit {
                reason: "done".to_string(),
            },
            "/test/resource".to_string(),
        );

        let entries = trail.all_entries();
        assert_eq!(entries[0].prev_hash, "genesis");
        assert_eq!(entries[1].prev_hash, entries[0].hash);
    }

    #[test]
    fn test_verify_passes_clean_chain() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::ToolCall {
                tool: "bash".to_string(),
                args_json: "{}".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::ToolResult {
                tool: "bash".to_string(),
                success: true,
            },
            "/test/resource".to_string(),
        );

        assert!(trail.verify().is_ok());
    }

    #[test]
    fn test_verify_detects_tampering() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::ToolCall {
                tool: "bash".to_string(),
                args_json: "{}".to_string(),
            },
            "/test/resource".to_string(),
        );

        // Tamper with an entry (change actor, which changes its hash)
        {
            let mut entries = trail.entries.write();
            entries[0].actor = "hacker-001".to_string();
        }

        // Verification should fail - entry 1's stored hash no longer matches recomputed hash
        let result = trail.verify();
        assert!(result.is_err());
        match result {
            Err(AuditError::ChainBroken { seq, .. }) => {
                // First entry's stored hash doesn't match its recomputed hash after tampering
                assert_eq!(seq, 1);
            }
            _ => panic!("expected ChainBroken error"),
        }
    }

    #[test]
    fn test_verify_detects_prev_hash_tampering() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::ToolCall {
                tool: "bash".to_string(),
                args_json: "{}".to_string(),
            },
            "/test/resource".to_string(),
        );

        // Tamper with prev_hash
        {
            let mut entries = trail.entries.write();
            entries[1].prev_hash = "fake-hash".to_string();
        }

        let result = trail.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_export_json_format() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        let json = trail.export_json(0).unwrap();
        
        // Should be valid JSON
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);

        // Should have expected fields
        let entry = &parsed[0];
        assert!(entry.get("seq").is_some());
        assert!(entry.get("timestamp").is_some());
        assert!(entry.get("actor").is_some());
        assert!(entry.get("action").is_some());
        assert!(entry.get("resource").is_some());
        assert!(entry.get("prev_hash").is_some());
        assert!(entry.get("hash").is_some());
    }

    #[test]
    fn test_by_agent_query() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-002".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentExit {
                reason: "done".to_string(),
            },
            "/test/resource".to_string(),
        );

        let agent_001_entries = trail.by_agent("agent-001");
        assert_eq!(agent_001_entries.len(), 2);

        let agent_002_entries = trail.by_agent("agent-002");
        assert_eq!(agent_002_entries.len(), 1);
    }

    #[test]
    fn test_by_action_query() {
        let trail = create_test_trail();

        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::ToolCall {
                tool: "bash".to_string(),
                args_json: "{}".to_string(),
            },
            "/test/resource".to_string(),
        );

        trail.append(
            "agent-001".to_string(),
            AuditAction::ToolCall {
                tool: "grep".to_string(),
                args_json: "{}".to_string(),
            },
            "/test/resource".to_string(),
        );

        let spawn_entries = trail.by_action(&AuditAction::AgentSpawn {
            task_type: "test".to_string(),
        });
        assert_eq!(spawn_entries.len(), 1);

        let tool_calls = trail.by_action_type("ToolCall");
        assert_eq!(tool_calls.len(), 2);
    }

    #[test]
    fn test_entries_range() {
        let trail = create_test_trail();

        for i in 0..10 {
            trail.append(
                "agent-001".to_string(),
                AuditAction::Other {
                    detail: format!("action-{}", i),
                },
                "/test/resource".to_string(),
            );
        }

        let range = trail.entries(3, 7);
        assert_eq!(range.len(), 5);
        assert_eq!(range[0].seq, 3);
        assert_eq!(range[4].seq, 7);
    }

    #[test]
    fn test_auto_prune() {
        let trail = AuditTrail::new(5);

        for i in 0..10 {
            trail.append(
                "agent-001".to_string(),
                AuditAction::Other {
                    detail: format!("action-{}", i),
                },
                "/test/resource".to_string(),
            );
        }

        // Should only have 5 entries (oldest pruned)
        assert_eq!(trail.len(), 5);

        let entries = trail.all_entries();
        // First entry should be seq 6 (after pruning 1-5)
        assert_eq!(entries[0].seq, 6);
        assert_eq!(entries[4].seq, 10);
    }

    #[test]
    fn test_append_with_metadata() {
        let trail = create_test_trail();
        let metadata = serde_json::json!({
            "duration_ms": 150,
            "memory_mb": 32
        });

        let hash = trail.append_with_meta(
            "agent-001".to_string(),
            AuditAction::MemoryWrite {
                entry_id: "mem-001".to_string(),
            },
            "/memory/entries".to_string(),
            Some(metadata.clone()),
        );

        assert!(!hash.is_empty());

        let entries = trail.all_entries();
        assert!(entries[0].metadata.is_some());
        assert_eq!(entries[0].metadata.as_ref().unwrap(), &metadata);
    }

    #[test]
    fn test_genesis_hash() {
        let trail = create_test_trail();
        
        // First entry should have prev_hash = "genesis"
        trail.append(
            "agent-001".to_string(),
            AuditAction::AgentSpawn {
                task_type: "test".to_string(),
            },
            "/test/resource".to_string(),
        );

        let entries = trail.all_entries();
        assert_eq!(entries[0].prev_hash, "genesis");
    }

    #[test]
    fn test_deterministic_hash() {
        let trail1 = create_test_trail();
        let trail2 = create_test_trail();

        let action = AuditAction::AgentSpawn {
            task_type: "test".to_string(),
        };

        trail1.append(
            "agent-001".to_string(),
            action.clone(),
            "/test/resource".to_string(),
        );

        // Same input should produce same hash
        let hash = compute_entry_hash(
            1,
            &trail1.all_entries()[0].timestamp,
            "agent-001",
            &action,
            "/test/resource",
            "genesis",
        );

        assert_eq!(hash, trail1.all_entries()[0].hash);
    }

    #[test]
    fn test_empty_trail_verify() {
        let trail = create_test_trail();
        assert!(trail.verify().is_ok());
    }

    #[test]
    fn test_all_action_types() {
        let trail = create_test_trail();

        let actions = vec![
            AuditAction::AgentSpawn { task_type: "test".to_string() },
            AuditAction::AgentExit { reason: "done".to_string() },
            AuditAction::ToolCall { tool: "bash".to_string(), args_json: "{}".to_string() },
            AuditAction::ToolResult { tool: "bash".to_string(), success: true },
            AuditAction::MemoryWrite { entry_id: "mem-001".to_string() },
            AuditAction::MemoryRead { entry_id: "mem-001".to_string() },
            AuditAction::ConfigChange { key: "max_agents".to_string() },
            AuditAction::ContainerStart { container_id: "ctr-001".to_string() },
            AuditAction::ContainerStop { container_id: "ctr-001".to_string() },
            AuditAction::ProgramInstall { program: "test-program".to_string(), version: "1.0.0".to_string() },
            AuditAction::CronTrigger { job_id: "job-001".to_string() },
            AuditAction::GitCommit { message: "test commit".to_string() },
            AuditAction::AccessDenied { permission: "write".to_string() },
            AuditAction::Other { detail: "misc".to_string() },
        ];

        for (i, action) in actions.into_iter().enumerate() {
            trail.append(
                "agent-001".to_string(),
                action,
                format!("/resource/{}", i),
            );
        }

        assert_eq!(trail.len(), 14);
        assert!(trail.verify().is_ok());
    }

    #[test]
    fn test_hash_different_for_different_inputs() {
        let ts = Utc::now();
        
        let hash1 = compute_entry_hash(
            1,
            &ts,
            "agent-001",
            &AuditAction::AgentSpawn { task_type: "test".to_string() },
            "/resource",
            "genesis",
        );

        let hash2 = compute_entry_hash(
            2,
            &ts,
            "agent-001",
            &AuditAction::AgentSpawn { task_type: "test".to_string() },
            "/resource",
            "genesis",
        );

        assert_ne!(hash1, hash2);

        let hash3 = compute_entry_hash(
            1,
            &ts,
            "agent-002",
            &AuditAction::AgentSpawn { task_type: "test".to_string() },
            "/resource",
            "genesis",
        );

        assert_ne!(hash1, hash3);
    }
}
