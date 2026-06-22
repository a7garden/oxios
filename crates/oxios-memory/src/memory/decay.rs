//! Ebbinghaus-inspired decay engine for memory importance scoring.
//!
//! Implements a forgetting curve: R(t) = e^(-rate × t), where the rate
//! is adjusted by memory type, protection level, and access frequency.

use chrono::{DateTime, Utc};

use crate::memory::types::{MemoryEntry, ProtectionLevel};

// ---------------------------------------------------------------------------
// DecayEngine
// ---------------------------------------------------------------------------

/// Decay engine — computes current retention scores for memory entries.
///
/// Uses an Ebbinghaus-inspired forgetting curve with adjustments for:
/// - Memory type (UserProfile decays slower than Conversation)
/// - Protection level (Higher protection = slower decay)
/// - Access frequency (Frequently accessed = slower decay)
/// - Global multiplier (user-configurable)
#[derive(Debug, Clone)]
pub struct DecayEngine {
    /// Global decay multiplier. 1.0 = default speed.
    pub multiplier: f32,
}

impl DecayEngine {
    /// Create a new decay engine with the given multiplier.
    pub fn new(multiplier: f32) -> Self {
        Self { multiplier }
    }

    /// Create with default multiplier (1.0).
    pub fn default_engine() -> Self {
        Self::new(1.0)
    }

    /// Compute current decay score for an entry.
    ///
    /// Returns a value between 0.0 (fully decayed) and 1.0 (fresh).
    /// Permanent protection always returns 1.0.
    pub fn compute_decay(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f32 {
        // Permanent protection = always 1.0
        if entry.pinned || entry.protection == ProtectionLevel::Permanent {
            return 1.0;
        }

        // Use fractional hours (not num_hours, which truncates to whole hours
        // and would treat a 59-minute-old access the same as a fresh one).
        let hours_since_access = ((now - entry.accessed_at).num_seconds().max(0) as f32) / 3600.0;
        let base_rate = entry.memory_type.base_decay_rate();

        // Access boost: frequently read memories decay slower
        let access_boost = 1.0 + (1.0_f32 + entry.access_count as f32).ln();

        // Protection multiplier: higher protection = slower decay
        let protection_mult = entry.protection.decay_multiplier();

        let effective_rate = base_rate * self.multiplier * protection_mult / access_boost;
        let retention = (-effective_rate * hours_since_access).exp();
        retention.clamp(0.0, 1.0)
    }

    /// Compute effective importance of a memory entry.
    ///
    /// Effective importance = base_importance × (1 + ln(1 + access_count)) × decay_score.
    pub fn effective_importance(entry: &MemoryEntry) -> f32 {
        let access_boost = 1.0 + (1.0_f32 + entry.access_count as f32).ln();
        entry.importance * access_boost * entry.decay_score
    }

    /// Check if an entry should be considered for pruning.
    ///
    /// An entry is a pruning candidate when:
    /// - its *current* decay score (recomputed at `now`) < threshold
    /// - protection is None or Low
    /// - not pinned
    /// - not auto-protected type
    ///
    /// The decay is recomputed against `now` rather than reading the stale
    /// persisted `entry.decay_score`, so an entry that has crossed the
    /// threshold since the last Dream run is still detected.
    pub fn is_prunable(&self, entry: &MemoryEntry, threshold: f32, now: DateTime<Utc>) -> bool {
        if entry.pinned {
            return false;
        }
        if entry.protection >= super::ProtectionLevel::Medium {
            return false;
        }
        if entry.memory_type.is_auto_protected() {
            return false;
        }
        self.compute_decay(entry, now) < threshold
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryEntry, MemoryType, ProtectionLevel};
    use chrono::Duration;

    fn make_entry(hours_ago: i64) -> MemoryEntry {
        MemoryEntry {
            id: "test".to_string(),
            memory_type: MemoryType::Fact,
            tier: crate::memory::MemoryTier::Warm,
            content: "test content".to_string(),
            content_hash: 0,
            tags: vec![],
            source: "test".to_string(),
            session_id: None,
            importance: 0.5,
            pinned: false,
            protection: ProtectionLevel::None,
            auto_classified: false,
            session_appearances: 0,
            user_corrected: false,
            seen_in_sessions: vec![],
            created_at: Utc::now(),
            accessed_at: Utc::now() - Duration::hours(hours_ago),
            modified_at: Utc::now(),
            access_count: 0,
            decay_score: 1.0,
            compaction_level: 0,
            compacted_from: vec![],
            related_ids: vec![],
            contradicts: None,
        }
    }

    #[test]
    fn test_decay_fresh() {
        let engine = DecayEngine::new(1.0);
        let entry = make_entry(0); // Just accessed
        let score = engine.compute_decay(&entry, Utc::now());
        assert!(
            score > 0.99,
            "Fresh entry should have decay ~1.0, got {}",
            score
        );
    }

    #[test]
    fn test_decay_old() {
        let engine = DecayEngine::new(1.0);
        let entry = make_entry(720); // 30 days ago
        let score = engine.compute_decay(&entry, Utc::now());
        assert!(
            score < 0.5,
            "Old entry should have significant decay, got {}",
            score
        );
    }

    #[test]
    fn test_decay_permanent_protection() {
        let engine = DecayEngine::new(1.0);
        let mut entry = make_entry(720);
        entry.protection = ProtectionLevel::Permanent;
        let score = engine.compute_decay(&entry, Utc::now());
        assert_eq!(score, 1.0, "Permanent protection should always be 1.0");
    }

    #[test]
    fn test_decay_pinned() {
        let engine = DecayEngine::new(1.0);
        let mut entry = make_entry(720);
        entry.pinned = true;
        let score = engine.compute_decay(&entry, Utc::now());
        assert_eq!(score, 1.0, "Pinned entry should always be 1.0");
    }

    #[test]
    fn test_decay_high_protection_slower() {
        let engine = DecayEngine::new(1.0);
        let mut entry_none = make_entry(168); // 7 days
        entry_none.protection = ProtectionLevel::None;

        let mut entry_high = make_entry(168);
        entry_high.protection = ProtectionLevel::High;

        let score_none = engine.compute_decay(&entry_none, Utc::now());
        let score_high = engine.compute_decay(&entry_high, Utc::now());
        assert!(
            score_high > score_none,
            "High protection should decay slower (high={}, none={})",
            score_high,
            score_none
        );
    }

    #[test]
    fn test_decay_access_boost() {
        let engine = DecayEngine::new(1.0);
        let mut entry_low = make_entry(168);
        entry_low.access_count = 0;

        let mut entry_high = make_entry(168);
        entry_high.access_count = 10;

        let score_low = engine.compute_decay(&entry_low, Utc::now());
        let score_high = engine.compute_decay(&entry_high, Utc::now());
        assert!(
            score_high > score_low,
            "Frequently accessed should decay slower (high={}, low={})",
            score_high,
            score_low
        );
    }

    #[test]
    fn test_effective_importance() {
        let mut entry = make_entry(0);
        entry.importance = 0.6;
        entry.access_count = 5;
        entry.decay_score = 0.8;
        let eff = DecayEngine::effective_importance(&entry);
        assert!(
            eff > 0.6,
            "Effective importance should be boosted, got {}",
            eff
        );
    }

    #[test]
    fn test_prunable() {
        let engine = DecayEngine::new(1.0);
        // 30 days old → recomputed decay well below the 0.05 threshold.
        let now = Utc::now();
        let mut entry = make_entry(720);
        assert!(
            engine.is_prunable(&entry, 0.05, now),
            "old, unprotected, unpinned entry should be prunable"
        );

        entry.pinned = true;
        assert!(!engine.is_prunable(&entry, 0.05, now));

        entry.pinned = false;
        entry.protection = ProtectionLevel::Medium;
        assert!(!engine.is_prunable(&entry, 0.05, now));
    }
}
