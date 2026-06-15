//! Conflict detection between calendar events.

use chrono::{DateTime, Utc};

use crate::types::{Conflict, Event};

/// Detect events that overlap with a given time range `[new_start, new_end)`.
///
/// `exclude_uid` is the UID of the event being checked (to avoid self-conflict
/// during updates).
///
/// Returns a list of conflicts sorted by overlap duration (largest first).
pub fn detect_conflicts(
    events: &[&Event],
    new_start: DateTime<Utc>,
    new_end: DateTime<Utc>,
    exclude_uid: Option<&str>,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for event in events {
        // Skip self
        if let Some(uid) = exclude_uid
            && event.uid == uid
        {
            continue;
        }

        // Skip cancelled events
        if event.status == "CANCELLED" {
            continue;
        }

        // Check overlap: two ranges [a, b) and [c, d) overlap iff a < d && c < b
        if new_start < event.end && event.start < new_end {
            let overlap_start = new_start.max(event.start);
            let overlap_end = new_end.min(event.end);
            let overlap_minutes = (overlap_end - overlap_start).num_minutes() as u32;

            conflicts.push(Conflict {
                uid: event.uid.clone(),
                title: event.title.clone(),
                overlap_minutes,
            });
        }
    }

    // Sort by overlap duration descending
    conflicts.sort_by_key(|b| std::cmp::Reverse(b.overlap_minutes));
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventSource;
    use chrono::TimeZone;

    fn make_event(uid: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Event {
        Event {
            uid: uid.to_string(),
            title: format!("Event {}", uid),
            start,
            end,
            all_day: false,
            description: None,
            location: None,
            rrule: None,
            status: "CONFIRMED".to_string(),
            source: EventSource::Agent,
            filename: format!("{}.ics", uid),
        }
    }

    #[test]
    fn no_conflicts() {
        let e1 = make_event(
            "e1",
            Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
        );
        let new_start = Utc.with_ymd_and_hms(2026, 6, 10, 11, 0, 0).unwrap();
        let new_end = Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap();

        let conflicts = detect_conflicts(&[&e1], new_start, new_end, None);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn overlapping_conflict() {
        let e1 = make_event(
            "e1",
            Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
        );
        let new_start = Utc.with_ymd_and_hms(2026, 6, 10, 9, 30, 0).unwrap();
        let new_end = Utc.with_ymd_and_hms(2026, 6, 10, 10, 30, 0).unwrap();

        let conflicts = detect_conflicts(&[&e1], new_start, new_end, None);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].uid, "e1");
        assert_eq!(conflicts[0].overlap_minutes, 30);
    }

    #[test]
    fn exclude_self() {
        let e1 = make_event(
            "e1",
            Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
        );
        let new_start = Utc.with_ymd_and_hms(2026, 6, 10, 9, 30, 0).unwrap();
        let new_end = Utc.with_ymd_and_hms(2026, 6, 10, 10, 30, 0).unwrap();

        let conflicts = detect_conflicts(&[&e1], new_start, new_end, Some("e1"));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn cancelled_not_conflict() {
        let mut e1 = make_event(
            "e1",
            Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
        );
        e1.status = "CANCELLED".to_string();

        let new_start = Utc.with_ymd_and_hms(2026, 6, 10, 9, 30, 0).unwrap();
        let new_end = Utc.with_ymd_and_hms(2026, 6, 10, 10, 30, 0).unwrap();

        let conflicts = detect_conflicts(&[&e1], new_start, new_end, None);
        assert!(conflicts.is_empty());
    }
}
