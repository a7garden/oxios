//! Calendar engine — the primary API for calendar operations.

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use tokio::fs;
use uuid::Uuid;

use crate::conflict;
use crate::ical;
use crate::index::CalendarIndex;
use crate::types::{
    AlarmEvent, CreateResult, Event, EventDraft, EventPatch, FreeBusySlot, IndexEntry, UpdateResult,
};

/// The main calendar engine.
///
/// Manages .ics files in a directory, maintains an in-memory index for fast
/// lookups, and provides conflict detection, free/busy calculation, and alarm
/// enumeration.
pub struct CalendarEngine {
    /// Root directory for .ics files.
    dir: PathBuf,
    /// In-memory index, protected by a read-write lock.
    index: RwLock<CalendarIndex>,
}

impl CalendarEngine {
    /// Create a new engine, loading or creating the index at `dir`.
    pub async fn new(dir: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&dir).await?;
        let index = CalendarIndex::load(&dir)?;
        Ok(Self {
            dir,
            index: RwLock::new(index),
        })
    }

    /// Create a new engine synchronously (for use outside async contexts).
    pub fn new_blocking(dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let index = CalendarIndex::load(&dir)?;
        Ok(Self {
            dir,
            index: RwLock::new(index),
        })
    }

    /// Get the calendar directory path.
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }

    /// Create a new event from an [`EventDraft`].
    ///
    /// Writes the .ics file, updates the index, and checks for conflicts.
    pub async fn create(&self, draft: EventDraft) -> anyhow::Result<CreateResult> {
        let uid = Uuid::new_v4().to_string();
        let filename = format!("{uid}.ics");

        // Build .ics content
        let ics_content = ical::build_ics(&uid, &draft)?;

        // Write to disk
        let path = self.dir.join(&filename);
        fs::write(&path, ics_content).await?;

        // Check conflicts against existing events. F15: collect filenames
        // under the lock, release, then read asynchronously.
        let existing_filenames: Vec<String> = {
            let index = self.index.read();
            index
                .all_entries()
                .into_iter()
                .map(|e| e.file.clone())
                .collect()
        };
        let existing = self.load_events_from_filenames(&existing_filenames).await;
        let conflicts = conflict::detect_conflicts(
            &existing.iter().collect::<Vec<_>>(),
            draft.start,
            draft.end,
            None,
        );

        // Update index
        let entry = IndexEntry {
            file: filename.clone(),
            summary: draft.title.clone(),
            dtstart: draft.start,
            dtend: draft.end,
            rrule: draft
                .repeat
                .as_ref()
                .map(crate::rrule::simple_repeat_to_rrule),
            status: "CONFIRMED".to_string(),
            source: draft.source,
        };
        self.index.write().insert(uid.clone(), entry);
        self.index.read().save()?;

        Ok(CreateResult {
            uid,
            status: "CONFIRMED".to_string(),
            conflicts,
            file: filename,
        })
    }

    /// Update an existing event with an [`EventPatch`].
    ///
    /// Re-reads the .ics, applies the patch, rewrites the file, and checks
    /// for conflicts.
    pub async fn update(&self, uid: &str, patch: EventPatch) -> anyhow::Result<UpdateResult> {
        let mut event = self.get(uid).await?;

        // Apply patch
        if let Some(ref title) = patch.title {
            event.title = title.clone();
        }
        if let Some(start) = patch.start {
            event.start = start;
        }
        if let Some(end) = patch.end {
            event.end = end;
        }
        if let Some(all_day) = patch.all_day {
            event.all_day = all_day;
        }
        if let Some(ref desc) = patch.description {
            event.description = desc.clone();
        }
        if let Some(ref loc) = patch.location {
            event.location = loc.clone();
        }
        if let Some(ref repeat) = patch.repeat {
            event.rrule = repeat.as_ref().map(crate::rrule::simple_repeat_to_rrule);
        }

        // Rebuild .ics
        let draft = EventDraft {
            title: event.title.clone(),
            start: event.start,
            end: event.end,
            all_day: event.all_day,
            description: event.description.clone(),
            location: event.location.clone(),
            repeat: None, // RRULE is already a string; preserve it
            reminder_minutes: patch.reminder_minutes.unwrap_or_default(),
            source: event.source,
        };

        let mut ics_content = ical::build_ics(&event.uid, &draft)?;

        // F16: append the original RRULE if it exists and the patch didn't
        // change it. `build_ics` doesn't add RRULE since `repeat` is None, so
        // we splice it in before the *structural* END:VEVENT. We locate the
        // last occurrence of the line marker "\nEND:VEVENT" (reverse-find)
        // rather than using `str::replace`, which would corrupt the file by
        // also matching "END:VEVENT" embedded in a DESCRIPTION/SUMMARY value.
        // iCalendar escapes newlines inside TEXT values as literal "\n", so
        // a real newline before "END:VEVENT" can only be the block terminator.
        if let Some(ref rrule_str) = event.rrule {
            const MARKER: &str = "\nEND:VEVENT";
            if let Some(idx) = ics_content.rfind(MARKER) {
                ics_content.insert_str(idx + 1, &format!("RRULE:{rrule_str}\n"));
            } else {
                tracing::warn!(
                    file = %event.filename,
                    "Could not locate END:VEVENT marker to splice RRULE; skipping"
                );
            }
        }

        let path = self.dir.join(&event.filename);
        fs::write(&path, ics_content).await?;

        // Check conflicts. F15: collect filenames under the lock, release,
        // then read asynchronously.
        let existing_filenames: Vec<String> = {
            let index = self.index.read();
            index
                .all_entries()
                .into_iter()
                .map(|e| e.file.clone())
                .collect()
        };
        let existing = self.load_events_from_filenames(&existing_filenames).await;
        let conflicts = conflict::detect_conflicts(
            &existing.iter().collect::<Vec<_>>(),
            event.start,
            event.end,
            Some(uid),
        );

        // Update index
        let entry = IndexEntry {
            file: event.filename.clone(),
            summary: event.title.clone(),
            dtstart: event.start,
            dtend: event.end,
            rrule: event.rrule.clone(),
            status: event.status.clone(),
            source: event.source,
        };
        self.index.write().insert(uid.to_string(), entry);
        self.index.read().save()?;

        Ok(UpdateResult {
            uid: uid.to_string(),
            status: event.status.clone(),
            conflicts,
        })
    }

    /// Delete an event by UID.
    pub async fn delete(&self, uid: &str) -> anyhow::Result<()> {
        let entry = self.index.write().remove(uid);
        if let Some(entry) = entry {
            let path = self.dir.join(&entry.file);
            if path.exists() {
                fs::remove_file(path).await?;
            }
            self.index.read().save()?;
            tracing::info!("Deleted event {}", uid);
        } else {
            return Err(anyhow::anyhow!("Event not found: {uid}"));
        }
        Ok(())
    }

    /// Get a single event by UID.
    ///
    /// F15: reads the .ics file asynchronously (`tokio::fs`) instead of
    /// blocking the runtime worker on `std::fs`.
    pub async fn get(&self, uid: &str) -> anyhow::Result<Event> {
        // Collect the needed owned data (path, filename) under the lock,
        // then release before the async read — parking_lot guards are not
        // `Send` and must not cross `.await`.
        let (path, filename) = {
            let index = self.index.read();
            let entry = index
                .get(uid)
                .ok_or_else(|| anyhow::anyhow!("Event not found: {uid}"))?;
            (self.dir.join(&entry.file), entry.file.clone())
        };
        let content = fs::read_to_string(&path).await?;
        ical::parse_ics(&content, &filename)
    }

    /// Read and parse multiple `.ics` files by name. Lock-free async I/O —
    /// callers collect filenames under the index lock, then release it before
    /// invoking this (parking_lot guards are not `Send` across `.await`).
    async fn load_events_from_filenames(&self, filenames: &[String]) -> Vec<Event> {
        let mut events = Vec::with_capacity(filenames.len());
        for filename in filenames {
            let path = self.dir.join(filename);
            match fs::read_to_string(&path).await {
                Ok(content) => match ical::parse_ics(&content, filename) {
                    Ok(event) => events.push(event),
                    Err(e) => tracing::warn!(
                        file = filename,
                        error = %e,
                        "failed to parse ics file"
                    ),
                },
                Err(e) => tracing::warn!(
                    file = filename,
                    error = %e,
                    "failed to read ics file"
                ),
            }
        }
        events
    }

    /// List all events in a time range `[from, to)`.
    ///
    /// For recurring events, the base event is included if it falls within
    /// range or its RRULE produces occurrences within range.
    ///
    /// F15: reads files asynchronously and releases the index lock before
    /// any `.await`. Files that fail to read/parse are logged and skipped
    /// rather than dropped silently (F11 secondary).
    pub async fn list(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> anyhow::Result<Vec<Event>> {
        // Snapshot the filenames we need to read, then release the index lock.
        let files: Vec<String> = {
            let index = self.index.read();
            let base_entries = index.list_range(from, to);

            // Also include recurring events whose base start is before `to`
            // and whose RRULE might produce occurrences in range
            let recurring_entries: Vec<&IndexEntry> = index
                .all_entries()
                .into_iter()
                .filter(|e| e.rrule.is_some() && e.dtstart < to)
                .filter(|e| !base_entries.iter().any(|b| b.file == e.file))
                .collect();

            let all_entries: Vec<&IndexEntry> =
                base_entries.into_iter().chain(recurring_entries).collect();

            all_entries.into_iter().map(|e| e.file.clone()).collect()
        };

        let mut events = Vec::new();
        for filename in &files {
            let path = self.dir.join(filename);
            let content = match fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(file = %filename, error = %e, "Failed to read calendar file");
                    continue;
                }
            };
            let event = match ical::parse_ics(&content, filename) {
                Ok(ev) => ev,
                Err(e) => {
                    tracing::warn!(
                        file = %filename,
                        error = %e,
                        "Failed to parse calendar file; skipping"
                    );
                    continue;
                }
            };
            // For recurring events, expand occurrences in range
            if let Some(ref rrule_str) = event.rrule {
                match self.expand_rrule_in_range(&event, rrule_str, from, to) {
                    Ok(expanded) => {
                        events.extend(expanded);
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(
                            file = %filename,
                            error = %e,
                            "RRULE expansion failed; falling back to base event"
                        );
                    }
                }
            }
            events.push(event);
        }

        // Sort by start time
        events.sort_by_key(|e| e.start);
        Ok(events)
    }

    /// Search events by title (case-insensitive substring match).
    ///
    /// F15: async file read + lock released before `.await`. Failures are
    /// logged instead of silently dropped.
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<Event>> {
        let files: Vec<String> = {
            let index = self.index.read();
            index
                .search(query)
                .into_iter()
                .map(|e| e.file.clone())
                .collect()
        };

        let mut events = Vec::new();
        for filename in &files {
            let path = self.dir.join(filename);
            match fs::read_to_string(&path).await {
                Ok(content) => match ical::parse_ics(&content, filename) {
                    Ok(event) => events.push(event),
                    Err(e) => tracing::warn!(
                        file = %filename,
                        error = %e,
                        "Failed to parse calendar file during search"
                    ),
                },
                Err(e) => tracing::warn!(
                    file = %filename,
                    error = %e,
                    "Failed to read calendar file during search"
                ),
            }
        }
        events.sort_by_key(|e| e.start);
        Ok(events)
    }

    /// Compute free/busy slots in a time range `[from, to)`.
    ///
    /// Returns a list of slots alternating between free and busy, starting
    /// with a free slot from `from`.
    pub async fn freebusy(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> anyhow::Result<Vec<FreeBusySlot>> {
        let events = self.list(from, to).await?;
        let mut slots = Vec::new();

        // Collect busy intervals
        let mut intervals: Vec<(DateTime<Utc>, DateTime<Utc>)> = events
            .iter()
            .filter(|e| e.status != "CANCELLED")
            .map(|e| (e.start, e.end))
            .collect();

        // Sort by start time
        intervals.sort_by_key(|(s, _)| *s);

        // Merge overlapping intervals
        let mut merged: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
        for (start, end) in intervals {
            if let Some(last) = merged.last_mut()
                && start <= last.1
            {
                last.1 = last.1.max(end);
                continue;
            }
            merged.push((start, end));
        }

        // Build free/busy slots
        let mut cursor = from;
        for (busy_start, busy_end) in &merged {
            if cursor < *busy_start {
                slots.push(FreeBusySlot {
                    start: cursor,
                    end: *busy_start,
                    busy: false,
                });
            }
            if cursor < *busy_end {
                slots.push(FreeBusySlot {
                    start: cursor.max(*busy_start),
                    end: *busy_end,
                    busy: true,
                });
            }
            cursor = cursor.max(*busy_end);
        }

        // Trailing free slot
        if cursor < to {
            slots.push(FreeBusySlot {
                start: cursor,
                end: to,
                busy: false,
            });
        }

        Ok(slots)
    }

    /// Find pending alarms in a time range `[from, to)`.
    ///
    /// This is a lightweight version that only returns alarms for non-recurring
    /// events from the index. Uses a default 15-minute reminder.
    pub fn find_pending_alarms(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<AlarmEvent> {
        let index = self.index.read();
        let entries = index.list_range(
            from - Duration::hours(24), // Look back for events that might have alarms
            to + Duration::hours(24),
        );

        let mut alarms = Vec::new();
        for entry in entries {
            // Default reminder: 15 minutes before
            let trigger_at = entry.dtstart - Duration::minutes(15);
            if trigger_at >= from && trigger_at <= to {
                alarms.push(AlarmEvent {
                    event_uid: entry.file.replace(".ics", ""),
                    event_title: entry.summary.clone(),
                    trigger_at,
                    minutes_before: 15,
                });
            }
        }
        alarms
    }

    /// Expand an RRULE into synthetic event occurrences within a time range.
    ///
    /// F9: the `rrule` crate's `all(limit)` silently truncates at `limit`;
    /// we surface that by logging a warning when `result.limited` is set so
    /// a partial result is never mistaken for a complete one.
    ///
    /// F10: an RRULE with a sub-hourly frequency and no COUNT/UNTIL over a
    /// wide range can produce a huge number of occurrences. The cap below
    /// bounds the work; a warning is also emitted for dangerous rule shapes
    /// so misconfigured/imported rules are visible.
    fn expand_rrule_in_range(
        &self,
        event: &Event,
        rrule_str: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Event>> {
        // F10: warn on high-frequency, unbounded rules — they're almost
        // always a misconfiguration or a malicious .ics and will hit the cap.
        let upper = rrule_str.to_ascii_uppercase();
        let has_bound = upper.contains("COUNT=") || upper.contains("UNTIL=");
        let high_freq = upper.contains("FREQ=SECONDLY")
            || upper.contains("FREQ=MINUTELY")
            || upper.contains("FREQ=HOURLY");
        if high_freq && !has_bound {
            tracing::warn!(
                event_uid = %event.uid,
                rrule = %rrule_str,
                "RRULE has a sub-daily frequency with no COUNT/UNTIL; \
                 expansion will be capped"
            );
        }

        let rrule_set_str = format!(
            "DTSTART:{}\nRRULE:{}",
            event.start.format("%Y%m%dT%H%M%SZ"),
            rrule_str
        );

        let rrule_set: rrule::RRuleSet = rrule_set_str
            .parse()
            .map_err(|e: rrule::RRuleError| anyhow::anyhow!("RRULE parse error: {e}"))?;

        // Convert DateTime<Utc> to rrule::Tz
        let rrule_tz = rrule::Tz::UTC;
        let from_tz = from.with_timezone(&rrule_tz);
        let to_tz = to.with_timezone(&rrule_tz);

        let after = rrule_set.after(from_tz);
        let bounded = after.before(to_tz);
        let result = bounded.all(MAX_RRULE_OCCURRENCES);

        // F9: don't swallow the truncation flag.
        if result.limited {
            tracing::warn!(
                event_uid = %event.uid,
                rrule = %rrule_str,
                cap = MAX_RRULE_OCCURRENCES,
                from = %from,
                to = %to,
                "RRULE expansion hit the occurrence cap; \
                 results are truncated — narrow the range or add COUNT/UNTIL"
            );
        }

        let duration = event.end - event.start;
        let mut expanded = Vec::with_capacity(result.dates.len());

        for dt in &result.dates {
            let start = dt.with_timezone(&Utc);
            let end = start + duration;
            expanded.push(Event {
                uid: event.uid.clone(),
                title: event.title.clone(),
                start,
                end,
                all_day: event.all_day,
                description: event.description.clone(),
                location: event.location.clone(),
                rrule: event.rrule.clone(),
                status: event.status.clone(),
                source: event.source,
                filename: event.filename.clone(),
            });
        }

        Ok(expanded)
    }

    /// Remove an entry from the index (used by archive).
    pub(crate) async fn remove_from_index(&self, uid: &str) -> anyhow::Result<()> {
        self.index.write().remove(uid);
        Ok(())
    }

    /// Save the index to disk (used by archive).
    pub(crate) async fn save_index(&self) -> anyhow::Result<()> {
        self.index.read().save()?;
        Ok(())
    }
}

/// Upper bound on RRULE occurrences generated for a single range query (F9/F10).
/// 10_000 covers ~27 years of daily events while preventing unbounded CPU/memory
/// use when an RRULE has no COUNT/UNTIL and the query range is wide.
const MAX_RRULE_OCCURRENCES: u16 = 10_000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventSource;
    use chrono::TimeZone;

    async fn make_engine() -> (tempfile::TempDir, CalendarEngine) {
        let dir = tempfile::tempdir().unwrap();
        let engine = CalendarEngine::new(dir.path().to_path_buf()).await.unwrap();
        (dir, engine)
    }

    #[tokio::test]
    async fn create_and_get() {
        let (_dir, engine) = make_engine().await;
        let draft = EventDraft {
            title: "Test Event".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
            all_day: false,
            description: Some("A test".into()),
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::User,
        };

        let result = engine.create(draft).await.unwrap();
        assert_eq!(result.status, "CONFIRMED");
        assert!(result.conflicts.is_empty());

        let event = engine.get(&result.uid).await.unwrap();
        assert_eq!(event.title, "Test Event");
        assert_eq!(event.source, EventSource::User);
    }

    #[tokio::test]
    async fn update_event() {
        let (_dir, engine) = make_engine().await;

        let draft = EventDraft {
            title: "Original".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };

        let result = engine.create(draft).await.unwrap();

        let patch = EventPatch {
            title: Some("Updated".into()),
            ..Default::default()
        };

        engine.update(&result.uid, patch).await.unwrap();
        let event = engine.get(&result.uid).await.unwrap();
        assert_eq!(event.title, "Updated");
    }

    #[tokio::test]
    async fn delete_event() {
        let (_dir, engine) = make_engine().await;

        let draft = EventDraft {
            title: "To Delete".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };

        let result = engine.create(draft).await.unwrap();
        engine.delete(&result.uid).await.unwrap();
        assert!(engine.get(&result.uid).await.is_err());
    }

    #[tokio::test]
    async fn list_events() {
        let (_dir, engine) = make_engine().await;

        for i in 0..3 {
            let draft = EventDraft {
                title: format!("Event {}", i),
                start: Utc
                    .with_ymd_and_hms(2026, 6, 10 + i as u32, 9, 0, 0)
                    .unwrap(),
                end: Utc
                    .with_ymd_and_hms(2026, 6, 10 + i as u32, 10, 0, 0)
                    .unwrap(),
                all_day: false,
                description: None,
                location: None,
                repeat: None,
                reminder_minutes: vec![],
                source: EventSource::Agent,
            };
            engine.create(draft).await.unwrap();
        }

        let from = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 6, 13, 0, 0, 0).unwrap();
        let events = engine.list(from, to).await.unwrap();
        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn search_events() {
        let (_dir, engine) = make_engine().await;

        let draft = EventDraft {
            title: "Weekly Standup".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 9, 30, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };
        engine.create(draft).await.unwrap();

        let results = engine.search("standup").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Weekly Standup");
    }

    #[tokio::test]
    async fn freebusy_slots() {
        let (_dir, engine) = make_engine().await;

        let draft = EventDraft {
            title: "Busy Time".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 11, 0, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };
        engine.create(draft).await.unwrap();

        let from = Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap();
        let slots = engine.freebusy(from, to).await.unwrap();

        assert_eq!(slots.len(), 3); // free, busy, free
        assert!(!slots[0].busy); // 9-10 free
        assert!(slots[1].busy); // 10-11 busy
        assert!(!slots[2].busy); // 11-12 free
    }

    #[tokio::test]
    async fn conflict_detection_on_create() {
        let (_dir, engine) = make_engine().await;

        let draft1 = EventDraft {
            title: "First".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };
        engine.create(draft1).await.unwrap();

        let draft2 = EventDraft {
            title: "Overlapping".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 30, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 10, 30, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };
        let result = engine.create(draft2).await.unwrap();
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].overlap_minutes, 30);
    }
}
