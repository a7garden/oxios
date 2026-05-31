# Session Persistence Design

**Status:** Design-only (not implemented)  
**Date:** 2026-05-31  
**Decision:** Option B — Dedicated session file approach

---

## Problem

The orchestrator stores active interview sessions in an in-memory `HashMap<String, InterviewSession>`. On process restart, all multi-turn interview state is lost, breaking the `--session` CLI feature and mid-interview clarifications.

The `StateStore` already persists `Session` objects (messages/responses), but the orchestrator's `InterviewSession` contains richer state that isn't captured.

## Options Considered

### Option A: StateStore Expansion

- Serialize `InterviewSession` into the existing `StateStore.save_json("sessions", ...)` path
- Pro: Uses existing infrastructure, no new files
- Con: StateStore `Session` and orchestrator `InterviewSession` are different types with different schemas. Merging them requires careful migration. StateStore also doesn't support atomic multi-field updates well.

### Option B: Dedicated Session File ✅ Chosen

- Serialize the full `InterviewSession` state to `~/.oxios/workspace/sessions/{id}.json`
- Write on state transition, load on startup
- Pro: Simple, one file per session, easy to debug, no schema conflict
- Con: Yet another persistence mechanism

**Rationale:** Option B avoids schema conflicts between `StateStore.Session` (message-level) and `InterviewSession` (orchestration-level). The orchestrator already has access to `StateStore`, so the write path is straightforward. Loading on startup is a simple directory scan.

---

## Data Model

### What Gets Persisted

```rust
/// Serializable interview session state.
/// Stored at ~/.oxios/workspace/sessions/{id}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedInterviewSession {
    /// Session ID (matches the key in the HashMap).
    id: String,
    /// Full interview state from Ouroboros.
    interview: InterviewResult,
    /// Current phase in the Ouroboros lifecycle.
    phase: Phase,
    /// Active seed ID if past the Seed phase.
    seed_id: Option<Uuid>,
    /// Active agent ID if past the Execute phase.
    agent_id: Option<Uuid>,
    /// Timestamp of last state transition (for staleness detection).
    last_updated: DateTime<Utc>,
    /// Schema version for future migration.
    schema_version: u32,
}
```

### Conversion

```rust
impl From<&InterviewSession> for PersistedInterviewSession {
    fn from(s: &InterviewSession) -> Self {
        Self {
            id: s.id.clone(),
            interview: s.interview.clone(),
            phase: s.phase,
            seed_id: s.seed_id,
            agent_id: s.agent_id,
            last_updated: Utc::now(),
            schema_version: 1,
        }
    }
}
```

### What is NOT Persisted

- In-flight HTTP request state
- Tokio JoinSet task handles
- Circuit breaker state
- Conversation buffer (topic-shift detection cache)

These are acceptable losses.

---

## Write Timing

### When to Save

| Event | Action |
|-------|--------|
| New session created (first message) | Write initial state |
| Interview phase produces questions | Write updated interview |
| Follow-up answer recorded | Write updated history |
| Seed generated | Write phase transition |
| Execution starts | Write phase transition |
| Session completed / removed | **Delete** file |

### Strategy: Write-on-transition (not write-on-every-turn)

Writing on every user message would be excessive. Instead, write only at state transitions:
1. **Session creation** — first time a session enters the HashMap
2. **Phase change** — Interview → Seed → Execute → Evaluate → Evolve
3. **Ambiguity resolution** — when new questions are generated
4. **Session cleanup** — delete the file

This means at most 4-5 writes per session, which is negligible I/O.

### Implementation Hook

```rust
// In orchestrator.rs — add a persist method
fn persist_session(&self, session: &InterviewSession) {
    let persisted = PersistedInterviewSession::from(session);
    let store = self.state_store.clone();
    tokio::spawn(async move {
        if let Err(e) = store.save_json("interview_sessions", &persisted.id, &persisted).await {
            tracing::warn!(session_id = %persisted.id, error = %e, "Failed to persist interview session");
        }
    });
}
```

Call sites:
- After `sessions.insert(...)` in `handle_message()`
- After `sessions.get_mut(...)` when interview state changes
- Instead of `sessions.remove(...)` → delete the file

---

## Recovery Flow

### On Startup

```rust
// In Orchestrator::with_config() or a new restore_sessions() method
async fn restore_sessions(&self) -> Result<()> {
    let names = self.state_store.list_category("interview_sessions").await?;
    
    for name in names {
        if let Ok(Some(persisted)) = self.state_store
            .load_json::<PersistedInterviewSession>("interview_sessions", &name).await 
        {
            // Check staleness — sessions older than 1 hour are discarded
            let age = Utc::now() - persisted.last_updated;
            if age > chrono::Duration::hours(1) {
                tracing::info!(session_id = %persisted.id, "Discarding stale session");
                let _ = self.state_store.delete_file("interview_sessions", &name).await;
                continue;
            }

            let session = InterviewSession {
                id: persisted.id,
                interview: persisted.interview,
                phase: persisted.phase,
                seed_id: persisted.seed_id,
                agent_id: persisted.agent_id,
            };

            self.sessions.write().insert(session.id.clone(), session);
            tracing::info!(session_id = %name, "Restored interview session");
        }
    }
    Ok(())
}
```

### Cleanup

Stale sessions are cleaned up in two places:
1. **On restore** — sessions older than 1 hour are discarded (interview context is stale)
2. **On prune** — the existing `StateStore.prune_sessions()` mechanism can be extended to also prune `interview_sessions/`
3. **On completion** — the orchestrator already removes sessions from the HashMap; the corresponding file is deleted too

---

## Schema Migration

The `schema_version` field allows future evolution:

```rust
fn migrate(persisted: &mut PersistedInterviewSession) -> bool {
    match persisted.schema_version {
        1 => true, // Current version, no migration needed
        _ => {
            tracing::warn!(version = persisted.schema_version, "Unknown session schema");
            false
        }
    }
}
```

Future versions may add:
- Tool call history per session
- Token usage tracking
- Model/provider metadata
- User preferences accumulated during interview

Migration is forward-only: old versions are upgraded, never downgraded.

---

## Failure Modes

| Failure | Handling |
|---------|----------|
| Write fails (disk full, permissions) | Log warning, session continues in memory. Next transition retries. |
| Corrupted JSON file on load | Log warning, skip session, delete file. |
| Very old session restored | Staleness check (1 hour TTL) discards it. |
| Schema mismatch | Migration function handles it; unknown schemas are skipped. |
| Process crash mid-write | Temp-file + atomic rename in StateStore prevents partial writes. |

---

## Impact Assessment

| Metric | Before | After |
|--------|--------|-------|
| Session recovery on restart | ❌ All lost | ✅ Active sessions restored |
| CLI `--session` after restart | ❌ Breaks | ✅ Works within TTL |
| Disk I/O per session | 0 | ~4-5 file writes (JSON, <10KB each) |
| Startup time impact | 0 | +0-50ms (directory scan + JSON parse) |
| New dependencies | 0 | 0 (uses existing StateStore) |

---

## Open Questions

1. **TTL for restored sessions:** 1 hour is conservative. For long-running interview sessions (e.g., complex seeds), this may be too short. Consider making it configurable.
2. **Concurrent access:** If two processes share the same workspace (unlikely but possible), file-based session storage has no locking. Consider using `fs2` file locks if this becomes a concern.
3. **Session metrics:** Should we emit a metric when restoring a session? Useful for monitoring how often restarts affect active sessions.
