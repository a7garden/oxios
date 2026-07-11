//! Streaming sink registry — per-session lookup table for the WebSocket
//! streaming path.
//!
//! Why this exists
//! ================
//!
//! The agent runtime's `AgentEvent::TextChunk` callback fires per text delta
//! during `run_streaming`. The Web UI needs each delta as a live `token`
//! chunk. Without plumbing `streaming_sink: Option<...>` through every layer
//! between the gateway and the runtime callback (Supervisor trait, AgentApi,
//! AgentTool, AgentLifecycleManager, Orchestrator, AgentRuntime), we use a
//! session-keyed registry:
//!
//! 1. The **gateway** creates `mpsc::unbounded()` + a collector task right
//!    before invoking the orchestrator. The collector holds the **strong**
//!    `Arc<UnboundedSender<StreamDelta>>` for the duration of the turn
//!    (with `target_conn_id = Some(conn_id)` on every partial `OutgoingMessage`,
//!    mirroring `gateway.rs:491`). The gateway also stores a `Weak` under
//!    `session_id` in this registry.
//! 2. The **runtime** callback looks up the registry by `session_id` (which it
//!    already has via `transparency_session`) and sends `StreamDelta::Text`
//!    directly into the channel — no plumbing through intermediate layers.
//! 3. When the collector task completes, the strong `Arc` drops. `Weak::upgrade`
//!    on the next runtime lookup returns `None` and the registry lookup
//!    cleanly misses — **no explicit unregister required, no stale entries**.
//!
//! Design rationale (vs threaded `Option<StreamingSinkTx>` params)
//! =================================================================
//!
//! `run_with_directive` is a `Supervisor` trait method with concrete impls
//! on `BasicSupervisor` and `NoOpSupervisor`, and `dyn Supervisor` is held
//! by `AgentLifecycleManager`, `AgentApi`, and `AgentTool`. Threading the
//! sink through there would touch every impl + every holder. The registry
//! avoids all of that because the runtime callback already has the
//! `session_id` it needs to look up the sink — no new arguments required.
//!
//! Concurrency
//! ===========
//!
//! The registry is `Send + Sync` (it owns a `Mutex<HashMap<...>>` of `Weak`s,
//! neither contains interior mutability beyond the mutex itself). Lookup is
//! O(n) where n = active turns; in practice n ≤ a few during normal use, so
//! a `HashMap` is fine. A `DashMap` would be a one-line swap if load grows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::mpsc::Sender;

/// Bounded capacity for streaming delta channels. Each delta is a small text
/// chunk (~100-500 bytes); 256 caps the buffer at ~128 KB, preventing OOM
/// from slow clients while absorbing normal burst latency.
pub const STREAMING_CHANNEL_CAPACITY: usize = 256;

use crate::agent_runtime::StreamDelta;

/// Strong sender side, wrapped in `Arc` so the runtime callback can clone
/// cheaply on every lookup. Re-exported here so callers don't have to know
/// the runtime's internal type.
pub type StreamingSinkSender = Arc<Sender<StreamDelta>>;

/// Per-session lookup table. Shared via `Arc` between the kernel handle,
/// the agent runtime, and the gateway dispatch layer.
#[derive(Default)]
pub struct StreamingSinkRegistry {
    inner: Mutex<HashMap<String, Weak<Sender<StreamDelta>>>>,
}

impl StreamingSinkRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a strong sender under `session_id`. Stores a `Weak` so the
    /// entry auto-cleans when the gateway's collector drops its strong
    /// reference — no explicit `unregister` needed in the happy path.
    pub fn register(&self, session_id: &str, sender: &StreamingSinkSender) {
        let weak = Arc::downgrade(sender);
        self.inner
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("streaming sink mutex poisoned — recovering");
                e.into_inner()
            })
            .insert(session_id.to_string(), weak);
    }

    /// Look up the sink for `session_id`. Returns `Some(tx)` only if a
    /// strong sender is still alive (i.e., the collector task hasn't
    /// completed and dropped its `Arc`). Misses are silent — the runtime
    /// simply skips emitting the delta.
    pub fn lookup(&self, session_id: &str) -> Option<StreamingSinkSender> {
        let guard = self.inner.lock().unwrap_or_else(|e| {
            tracing::error!("streaming sink mutex poisoned — recovering");
            e.into_inner()
        });
        guard
            .get(session_id)
            .and_then(|w| w.upgrade())
            .map(|a| Arc::clone(&a))
    }

    /// Explicit unregister. Rarely needed — `Weak::upgrade` returning
    /// `None` is the normal cleanup path. Exposed for symmetry / tests
    /// that want to assert pre-cleanup behavior.
    pub fn unregister(&self, session_id: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("streaming sink mutex poisoned — recovering");
                e.into_inner()
            })
            .remove(session_id);
    }

    /// Test/observability helper: number of active sessions.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("streaming sink mutex poisoned — recovering");
                e.into_inner()
            })
            .len()
    }
    /// Returns true if there are no active streaming sessions.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::StreamingSinkTx;

    #[test]
    fn lookup_miss_when_empty() {
        let r = StreamingSinkRegistry::new();
        assert!(r.lookup("missing").is_none());
    }

    #[test]
    fn lookup_returns_strong_when_alive() {
        let r = StreamingSinkRegistry::new();
        let (tx, _rx) = tokio::sync::mpsc::channel::<StreamDelta>(8);
        let sender: StreamingSinkTx = Arc::new(tx);
        r.register("s1", &sender);
        let looked = r.lookup("s1").expect("should find live sink");
        assert!(Arc::ptr_eq(&looked, &sender));
    }

    #[test]
    fn lookup_misses_after_drop() {
        let r = StreamingSinkRegistry::new();
        let (tx, _rx) = tokio::sync::mpsc::channel::<StreamDelta>(8);
        let sender: StreamingSinkTx = Arc::new(tx);
        r.register("s1", &sender);
        drop(sender);
        assert!(r.lookup("s1").is_none(), "stale Weak must not upgrade");
    }

    #[test]
    fn unregister_clears_entry() {
        let r = StreamingSinkRegistry::new();
        let (tx, _rx) = tokio::sync::mpsc::channel::<StreamDelta>(8);
        let sender: StreamingSinkTx = Arc::new(tx);
        r.register("s1", &sender);
        r.unregister("s1");
        assert!(r.lookup("s1").is_none());
        // The strong sender is still alive — caller can keep using it
        // directly; the registry just no longer points at it.
        assert!(Arc::strong_count(&sender) >= 1);
    }
}
