//! Audit trail REST API routes.
//!
//! Provides endpoints for querying and verifying the cryptographic hash-chain
//! audit log maintained by the Oxios kernel.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::AppError;
use crate::server::AppState;

// ─── Request Types ─────────────────────────────────────────────────────────────

/// Query parameters for range-based audit entry queries.
#[derive(Debug, Deserialize)]
pub struct RangeQuery {
    /// Starting sequence number (inclusive, default: 0).
    pub from_seq: Option<u64>,
    /// Ending sequence number (inclusive, default: 100).
    pub to_seq: Option<u64>,
}

/// Request body for exporting audit entries.
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    /// Starting sequence number for export (default: 0).
    pub from_seq: Option<u64>,
}

// ─── Response Types ───────────────────────────────────────────────────────────

/// Response for audit entry queries.
#[derive(Debug, Serialize)]
pub struct AuditEntriesResponse {
    pub entries: Vec<serde_json::Value>,
    pub count: usize,
}

/// Response for audit verification.
#[derive(Debug, Serialize)]
pub struct AuditVerifyResponse {
    pub valid: bool,
    pub entry_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broken_at_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found: Option<String>,
}

/// Response for audit export.
#[derive(Debug, Serialize)]
pub struct AuditExportResponse {
    pub json: String,
    pub entry_count: usize,
}

/// Response for audit flush operation.
#[derive(Debug, Serialize)]
pub struct AuditFlushResponse {
    pub flushed: usize,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/audit/entries
///
/// Query audit entries within a sequence range.
/// Defaults: from_seq=0, to_seq=100
pub(crate) async fn handle_audit_entries(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RangeQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let from_seq = params.from_seq.unwrap_or(0);
    let to_seq = params.to_seq.unwrap_or(100);

    let entries = state.kernel.query_audit(from_seq, to_seq);
    let count = entries.len();

    let entries_json: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|e| serde_json::to_value(&e).unwrap_or(serde_json::Value::Null))
        .collect();

    Ok(Json(json!({
        "entries": entries_json,
        "count": count,
    })))
}

/// GET /api/audit/verify
///
/// Verify the cryptographic hash chain integrity of the audit trail.
pub(crate) async fn handle_audit_verify(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entry_count = state.kernel.audit_count();

    match state.kernel.verify_audit() {
        Ok(valid) => Ok(Json(json!({
            "valid": valid,
            "entry_count": entry_count,
        }))),
        Err(e) => {
            // Parse the error message to extract details
            let msg = e.to_string();
            if let Some(seq) = msg.strip_prefix("chain broken at seq ") {
                let parts: Vec<&str> = seq.split(" expected ").collect();
                if parts.len() >= 2 {
                    let seq_num: u64 = parts[0].parse().unwrap_or(0);
                    let exp_found: Vec<&str> = parts[1].split(" found ").collect();
                    if exp_found.len() >= 2 {
                        return Ok(Json(json!({
                            "valid": false,
                            "entry_count": entry_count,
                            "broken_at_seq": seq_num,
                            "expected": exp_found[0],
                            "found": exp_found[1],
                        })));
                    }
                }
            }
            if msg.contains("timestamp in the future") {
                let seq = msg.lines()
                    .find(|l| l.contains("seq"))
                    .and_then(|l| l.split(":").nth(1))
                    .map(|s| s.trim().parse::<u64>().unwrap_or(0))
                    .unwrap_or(0);
                return Ok(Json(json!({
                    "valid": false,
                    "entry_count": entry_count,
                    "broken_at_seq": seq,
                    "expected": "valid timestamp",
                    "found": "timestamp in the future",
                })));
            }
            Err(AppError::Internal(format!("audit verify failed: {}", msg)))
        }
    }
}

/// GET /api/audit/agent/{agent_id}
///
/// Query all audit entries for a specific agent.
pub(crate) async fn handle_audit_by_agent(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entries = state.kernel.query_audit_by_agent(&agent_id);
    let count = entries.len();

    let entries_json: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|e| serde_json::to_value(&e).unwrap_or(serde_json::Value::Null))
        .collect();

    Ok(Json(json!({
        "entries": entries_json,
        "count": count,
    })))
}

/// POST /api/audit/export
///
/// Export audit entries from a sequence number as JSON.
pub(crate) async fn handle_audit_export(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ExportRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let from_seq = body.from_seq.unwrap_or(0);

    let entries = state.kernel.query_audit(from_seq, u64::MAX);
    let entry_count = entries.len();

    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| AppError::Internal(format!("failed to serialize entries: {}", e)))?;

    Ok(Json(json!({
        "json": json,
        "entry_count": entry_count,
    })))
}

/// POST /api/audit/flush
///
/// Flush audit entries to the StateStore for persistence.
pub(crate) async fn handle_audit_flush(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let flushed = state.kernel.audit_count();

    state.kernel.flush_audit();

    Ok(Json(json!({
        "flushed": flushed,
    })))
}