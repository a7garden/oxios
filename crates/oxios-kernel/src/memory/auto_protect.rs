//! Automatic memory protection based on access patterns.
//!
//! Computes protection levels from behavior signals (access frequency,
//! session appearances, user corrections) without user intervention.

use chrono::Utc;

use super::{MemoryEntry, ProtectionLevel};

// ---------------------------------------------------------------------------
// AutoProtector
// ---------------------------------------------------------------------------

/// Automatic protection calculator.
///
/// Protection levels are computed from access patterns:
/// - None → Low: 2+ accesses
/// - Low → Medium: 3+ accesses OR 2+ session appearances
/// - Medium → High: 5+ accesses OR 3+ session appearances OR user correction
/// - High → Permanent: UserProfile/Preference type OR explicit pin
///
/// Protection can also be demoted when entries become stale.
#[derive(Debug, Clone)]
pub struct AutoProtector {
    /// Minimum access count for Low protection.
    pub protection_low_access: u32,
    /// Minimum access count for Medium protection.
    pub protection_medium_access: u32,
    /// Minimum access count for High protection.
    pub protection_high_access: u32,
    /// Minimum session appearances for Medium protection.
    pub protection_medium_sessions: u32,
    /// Minimum session appearances for High protection.
    pub protection_high_sessions: u32,
    /// Days without access before considering demotion.
    pub demotion_stale_days: u32,
}

impl AutoProtector {
    /// Create a new protector with the given thresholds.
    pub fn new(
        low_access: u32,
        medium_access: u32,
        high_access: u32,
        medium_sessions: u32,
        high_sessions: u32,
        demotion_stale_days: u32,
    ) -> Self {
        Self {
            protection_low_access: low_access,
            protection_medium_access: medium_access,
            protection_high_access: high_access,
            protection_medium_sessions: medium_sessions,
            protection_high_sessions: high_sessions,
            demotion_stale_days,
        }
    }

    /// Create with default thresholds from RFC-008.
    pub fn default_protector() -> Self {
        Self::new(2, 3, 5, 2, 3, 30)
    }

    /// Compute protection level for a memory entry based on access patterns.
    ///
    /// This is the core auto-protection logic. Dream calls this every run.
    pub fn compute_protection(&self, entry: &MemoryEntry) -> ProtectionLevel {
        // 1. Type-based default protection
        if entry.memory_type.is_auto_protected() {
            return ProtectionLevel::Permanent;
        }

        // 2. Explicit pin
        if entry.pinned {
            return ProtectionLevel::Permanent;
        }

        // 3. User correction → High
        if entry.user_corrected {
            return ProtectionLevel::High;
        }

        // 4. Access pattern-based promotion
        let access_count = entry.access_count;
        let session_span = entry.session_appearances;

        // 5+ accesses OR 3+ sessions → High
        if access_count >= self.protection_high_access
            || session_span >= self.protection_high_sessions
        {
            return ProtectionLevel::High;
        }

        // 3+ accesses OR 2+ sessions → Medium
        if access_count >= self.protection_medium_access
            || session_span >= self.protection_medium_sessions
        {
            return ProtectionLevel::Medium;
        }

        // 2+ accesses → Low
        if access_count >= self.protection_low_access {
            return ProtectionLevel::Low;
        }

        // Default: no protection
        ProtectionLevel::None
    }

    /// Evaluate whether a protection level should be demoted.
    ///
    /// Only demotes by one step at a time (High → Medium, Medium → Low, etc.).
    /// Returns `None` if no demotion is warranted.
    pub fn should_demote_protection(
        &self,
        entry: &MemoryEntry,
        current: ProtectionLevel,
    ) -> Option<ProtectionLevel> {
        // Permanent and explicit pins are never demoted
        if entry.pinned || current == ProtectionLevel::Permanent {
            return None;
        }

        let days_since_access = (Utc::now() - entry.accessed_at).num_days() as u32;
        let stale = self.demotion_stale_days;

        // High → Medium: stale_days + current criteria no longer met
        if current == ProtectionLevel::High
            && days_since_access > stale
            && entry.access_count < self.protection_medium_access
        {
            return Some(ProtectionLevel::Medium);
        }

        // Medium → Low: stale_days × 2
        if current == ProtectionLevel::Medium && days_since_access > stale * 2 {
            return Some(ProtectionLevel::Low);
        }

        // Low → None: stale_days × 3
        if current == ProtectionLevel::Low && days_since_access > stale * 3 {
            return Some(ProtectionLevel::None);
        }

        None
    }

    /// Record access to a memory entry (updates tracking fields).
    ///
    /// Call this whenever a memory is recalled or searched.
    pub fn record_access(entry: &mut MemoryEntry, current_session_id: &str) {
        entry.access_count += 1;
        entry.accessed_at = Utc::now();

        // Update session_appearances with dedup
        if !entry
            .seen_in_sessions
            .contains(&current_session_id.to_string())
        {
            entry.session_appearances += 1;
            entry.seen_in_sessions.push(current_session_id.to_string());
            // Cap at 100 entries
            if entry.seen_in_sessions.len() > 100 {
                entry.seen_in_sessions.remove(0);
            }
        }

        // Partial decay recovery on access
        let boosted = 0.5 + 0.5 * entry.decay_score;
        entry.decay_score = entry.decay_score.max(boosted);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryEntry, MemoryTier, MemoryType};
    use chrono::Duration;

    fn make_entry_with_access(access_count: u32, sessions: u32) -> MemoryEntry {
        let mut entry = make_base_entry();
        entry.access_count = access_count;
        entry.session_appearances = sessions;
        entry.seen_in_sessions = (0..sessions).map(|i| format!("session-{}", i)).collect();
        entry
    }

    fn make_base_entry() -> MemoryEntry {
        MemoryEntry {
            id: "test".to_string(),
            memory_type: MemoryType::Fact,
            tier: MemoryTier::Warm,
            content: "test".to_string(),
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
            accessed_at: Utc::now(),
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
    fn test_protection_none_default() {
        let protector = AutoProtector::default_protector();
        let entry = make_entry_with_access(0, 0);
        assert_eq!(protector.compute_protection(&entry), ProtectionLevel::None);
    }

    #[test]
    fn test_protection_low() {
        let protector = AutoProtector::default_protector();
        let entry = make_entry_with_access(2, 0);
        assert_eq!(protector.compute_protection(&entry), ProtectionLevel::Low);
    }

    #[test]
    fn test_protection_medium_access() {
        let protector = AutoProtector::default_protector();
        let entry = make_entry_with_access(3, 0);
        assert_eq!(
            protector.compute_protection(&entry),
            ProtectionLevel::Medium
        );
    }

    #[test]
    fn test_protection_medium_sessions() {
        let protector = AutoProtector::default_protector();
        let entry = make_entry_with_access(0, 2);
        assert_eq!(
            protector.compute_protection(&entry),
            ProtectionLevel::Medium
        );
    }

    #[test]
    fn test_protection_high_access() {
        let protector = AutoProtector::default_protector();
        let entry = make_entry_with_access(5, 0);
        assert_eq!(protector.compute_protection(&entry), ProtectionLevel::High);
    }

    #[test]
    fn test_protection_high_sessions() {
        let protector = AutoProtector::default_protector();
        let entry = make_entry_with_access(0, 3);
        assert_eq!(protector.compute_protection(&entry), ProtectionLevel::High);
    }

    #[test]
    fn test_protection_permanent_for_profile() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_base_entry();
        entry.memory_type = MemoryType::UserProfile;
        assert_eq!(
            protector.compute_protection(&entry),
            ProtectionLevel::Permanent
        );
    }

    #[test]
    fn test_protection_permanent_for_preference() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_base_entry();
        entry.memory_type = MemoryType::Preference;
        assert_eq!(
            protector.compute_protection(&entry),
            ProtectionLevel::Permanent
        );
    }

    #[test]
    fn test_protection_user_correction() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_base_entry();
        entry.user_corrected = true;
        assert_eq!(protector.compute_protection(&entry), ProtectionLevel::High);
    }

    #[test]
    fn test_protection_pinned() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_base_entry();
        entry.pinned = true;
        assert_eq!(
            protector.compute_protection(&entry),
            ProtectionLevel::Permanent
        );
    }

    #[test]
    fn test_demote_high_to_medium() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_entry_with_access(2, 0); // Below medium threshold
        entry.accessed_at = Utc::now() - Duration::days(35); // > 30 days stale
        let result = protector.should_demote_protection(&entry, ProtectionLevel::High);
        assert_eq!(result, Some(ProtectionLevel::Medium));
    }

    #[test]
    fn test_demote_medium_to_low() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_entry_with_access(3, 1);
        entry.accessed_at = Utc::now() - Duration::days(65); // > 60 days stale
        let result = protector.should_demote_protection(&entry, ProtectionLevel::Medium);
        assert_eq!(result, Some(ProtectionLevel::Low));
    }

    #[test]
    fn test_demote_low_to_none() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_entry_with_access(2, 0);
        entry.accessed_at = Utc::now() - Duration::days(95); // > 90 days stale
        let result = protector.should_demote_protection(&entry, ProtectionLevel::Low);
        assert_eq!(result, Some(ProtectionLevel::None));
    }

    #[test]
    fn test_no_demote_permanent() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_base_entry();
        entry.accessed_at = Utc::now() - Duration::days(365);
        let result = protector.should_demote_protection(&entry, ProtectionLevel::Permanent);
        assert_eq!(result, None);
    }

    #[test]
    fn test_no_demote_pinned() {
        let protector = AutoProtector::default_protector();
        let mut entry = make_base_entry();
        entry.pinned = true;
        entry.accessed_at = Utc::now() - Duration::days(365);
        let result = protector.should_demote_protection(&entry, ProtectionLevel::High);
        assert_eq!(result, None);
    }

    #[test]
    fn test_record_access() {
        let mut entry = make_base_entry();
        entry.decay_score = 0.2;
        AutoProtector::record_access(&mut entry, "session-1");

        assert_eq!(entry.access_count, 1);
        assert_eq!(entry.session_appearances, 1);
        assert!(entry.seen_in_sessions.contains(&"session-1".to_string()));
        assert!(entry.decay_score > 0.2, "Should recover decay on access");
    }

    #[test]
    fn test_record_access_dedup_session() {
        let mut entry = make_base_entry();
        AutoProtector::record_access(&mut entry, "session-1");
        AutoProtector::record_access(&mut entry, "session-1");
        assert_eq!(entry.access_count, 2);
        assert_eq!(
            entry.session_appearances, 1,
            "Same session should not increment appearances"
        );
    }
}
