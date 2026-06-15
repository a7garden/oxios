//! Delivery reliability layer (RFC-024 SP1).
//!
//! Sits between the gateway and every [`Channel`] implementation. The layer
//! assigns a monotonic `seq` to each outgoing message and keeps a bounded
//! ring buffer of recent messages so a reconnecting client can ask for
//! messages it missed by `last_seq` (C2).
//!
//! **Hybrid replay policy** (RFC-024 §2):
//! - If `last_seq + 1` falls within the live buffer, the client gets the
//!   missing slice via `Replay(msgs)` — gapless.
//! - If the cursor is older than the buffer's oldest message (TTL expired
//!   or eviction pushed it out), the client gets `Resync` and is expected
//!   to pull the full state via the regular HTTP API.
//!
//! At-least-once + idempotent dedup at the client = effectively-once.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use crate::message::OutgoingMessage;

/// Tunable replay policy.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// How many of the most recent messages to keep in the ring buffer.
    pub buffer_size: usize,
    /// How long a message stays in the buffer before being purged.
    pub ttl: Duration,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            buffer_size: 512,
            ttl: Duration::from_secs(60),
        }
    }
}

/// Outcome of a replay request.
#[derive(Debug, Clone)]
pub enum ReplayResult {
    /// All messages after `last_seq` (exclusive) that the buffer still holds.
    /// May be empty if the caller is fully caught up.
    Replay(Vec<OutgoingMessage>),
    /// The cursor is older than the buffer's oldest surviving message; the
    /// client should re-pull the full state via the regular HTTP API.
    Resync,
}

struct BufferEntry {
    msg: OutgoingMessage,
    inserted_at: Instant,
}

/// Per-gateway delivery layer. Cheap to clone via internal `Arc`-free state
/// (each field is already a `Sync` primitive or a `RwLock`).
pub struct ReliabilityLayer {
    seq: AtomicU64,
    buffer: RwLock<VecDeque<BufferEntry>>,
    config: ReplayConfig,
}

impl ReliabilityLayer {
    /// Create a new layer with the given policy.
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            seq: AtomicU64::new(0),
            buffer: RwLock::new(VecDeque::with_capacity(config.buffer_size)),
            config,
        }
    }

    /// Assign the next `seq` to `msg`, push it into the replay buffer, and
    /// return the modified message. Idempotency keys (`msg.id`) and the
    /// sequence number (`msg.seq`) are independent — the seq establishes
    /// *order*, the id establishes *uniqueness* for client-side dedup.
    pub fn assign_seq(&self, mut msg: OutgoingMessage) -> OutgoingMessage {
        let s = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        msg.seq = Some(s);

        let mut buf = self.buffer.write();
        let now = Instant::now();

        // Purge TTL-expired entries from the front.
        while let Some(front) = buf.front() {
            if now.duration_since(front.inserted_at) > self.config.ttl {
                buf.pop_front();
            } else {
                break;
            }
        }

        // Enforce capacity by evicting the oldest until we have a free slot.
        while buf.len() >= self.config.buffer_size {
            buf.pop_front();
        }

        buf.push_back(BufferEntry {
            msg: msg.clone(),
            inserted_at: now,
        });
        msg
    }

    /// Look up messages that the caller missed.
    ///
    /// - `last_seq = 0` returns every buffered message (full replay).
    /// - `last_seq` within the buffer returns the gapless slice.
    /// - `last_seq` older than the buffer's oldest surviving message returns
    ///   `Resync` so the client can pull fresh state via HTTP.
    pub fn replay(&self, last_seq: u64) -> ReplayResult {
        let buf = self.buffer.read();
        let oldest_seq = buf.front().and_then(|e| e.msg.seq);

        if let Some(oldest) = oldest_seq
            && last_seq + 1 < oldest
        {
            return ReplayResult::Resync;
        }

        let msgs: Vec<OutgoingMessage> = buf
            .iter()
            .filter(|e| e.msg.seq.is_some_and(|s| s > last_seq))
            .map(|e| e.msg.clone())
            .collect();
        ReplayResult::Replay(msgs)
    }

    /// Current maximum assigned seq (0 if no message has been assigned yet).
    /// Diagnostic / test helper.
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    /// How many messages are currently buffered.
    pub fn buffer_len(&self) -> usize {
        self.buffer.read().len()
    }
}

impl std::fmt::Debug for ReliabilityLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReliabilityLayer")
            .field("current_seq", &self.current_seq())
            .field("buffer_len", &self.buffer_len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn msg() -> OutgoingMessage {
        OutgoingMessage::with_id(Uuid::new_v4(), "test", "user", "hi")
    }

    #[test]
    fn assign_seq_is_monotonic() {
        let l = ReliabilityLayer::new(ReplayConfig::default());
        let a = l.assign_seq(msg());
        let b = l.assign_seq(msg());
        let c = l.assign_seq(msg());
        assert_eq!(a.seq, Some(1));
        assert_eq!(b.seq, Some(2));
        assert_eq!(c.seq, Some(3));
    }

    #[test]
    fn replay_returns_messages_after_cursor() {
        let l = ReliabilityLayer::new(ReplayConfig::default());
        l.assign_seq(msg());
        l.assign_seq(msg());
        l.assign_seq(msg());
        match l.replay(1) {
            ReplayResult::Replay(msgs) => {
                assert_eq!(msgs.len(), 2);
                assert_eq!(msgs[0].seq, Some(2));
                assert_eq!(msgs[1].seq, Some(3));
            }
            _ => panic!("expected Replay"),
        }
    }

    #[test]
    fn replay_zero_returns_everything() {
        let l = ReliabilityLayer::new(ReplayConfig::default());
        l.assign_seq(msg());
        l.assign_seq(msg());
        match l.replay(0) {
            ReplayResult::Replay(msgs) => assert_eq!(msgs.len(), 2),
            _ => panic!("expected Replay"),
        }
    }

    #[test]
    fn replay_beyond_buffer_returns_resync() {
        let l = ReliabilityLayer::new(ReplayConfig {
            buffer_size: 2,
            ttl: Duration::from_secs(60),
        });
        l.assign_seq(msg());
        l.assign_seq(msg());
        l.assign_seq(msg()); // evicts seq 1
        // Cursor at 0 is older than the oldest surviving (2) → Resync.
        assert!(matches!(l.replay(0), ReplayResult::Resync));
    }

    #[test]
    fn replay_at_exact_boundary_returns_empty() {
        let l = ReliabilityLayer::new(ReplayConfig::default());
        l.assign_seq(msg()); // seq 1
        l.assign_seq(msg()); // seq 2
        // Cursor == current seq → nothing to replay.
        match l.replay(2) {
            ReplayResult::Replay(msgs) => assert!(msgs.is_empty()),
            _ => panic!("expected Replay(empty)"),
        }
    }

    #[test]
    fn capacity_evicts_oldest() {
        let l = ReliabilityLayer::new(ReplayConfig {
            buffer_size: 2,
            ttl: Duration::from_secs(60),
        });
        l.assign_seq(msg());
        l.assign_seq(msg());
        l.assign_seq(msg());
        assert_eq!(l.buffer_len(), 2);
        assert_eq!(l.current_seq(), 3);
    }

    #[test]
    fn ttl_purges_expired() {
        let l = ReliabilityLayer::new(ReplayConfig {
            buffer_size: 100,
            ttl: Duration::from_millis(20),
        });
        l.assign_seq(msg());
        std::thread::sleep(Duration::from_millis(40));
        l.assign_seq(msg()); // triggers purge
        assert_eq!(l.buffer_len(), 1);
    }
}
