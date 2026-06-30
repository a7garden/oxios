//! Journal bridge — injects calendar events into daily journal files.
//!
//! The bridge reads events from [`CalendarEngine`] for a given date and
//! writes a `"## Today's events"` section into the corresponding journal
//! file managed by `oxios-markdown`. If no events exist, the section is
//! removed. If the journal file doesn't exist, nothing is written.

use chrono::{FixedOffset, NaiveDate};
use oxios_markdown::fs::VirtualFs;
use oxios_markdown::types::DIR_JOURNAL;

use crate::engine::CalendarEngine;

/// Bridge between calendar events and the journal system.
///
/// Injects a `"## Today's events"` section into journal files, listing
/// all calendar events for that day with links back to their .ics files.
pub struct JournalBridge {
    engine: std::sync::Arc<CalendarEngine>,
    fs: VirtualFs,
    timezone: FixedOffset,
}

impl JournalBridge {
    /// Create a new [`JournalBridge`].
    pub fn new(
        engine: std::sync::Arc<CalendarEngine>,
        fs: VirtualFs,
        timezone: FixedOffset,
    ) -> Self {
        Self {
            engine,
            fs,
            timezone,
        }
    }

    /// Inject calendar events for a specific date into the journal.
    ///
    /// Finds or creates a `"## Today's events"` section in the journal file.
    /// If the section already exists, it is replaced with the current events.
    /// If there are no events, the section is removed.
    /// If the journal file doesn't exist, this is a no-op.
    pub async fn inject_events(&self, date: NaiveDate) -> anyhow::Result<()> {
        // 1. Build UTC range for the date in the configured timezone.
        // F12: replace the `unwrap()` panic on `and_local_timezone().earliest()`
        // with an explicit error. FixedOffset has no DST so this can't fail
        // today, but the code accepts any FixedOffset and the defensive
        // check keeps a future timezone-type change from panicking.
        let start_of_day = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid midnight for date {date}"))?
            .and_local_timezone(self.timezone)
            .earliest()
            .ok_or_else(|| {
                anyhow::anyhow!("Non-existent local midnight {date} in the configured timezone")
            })?
            .to_utc();
        let end_of_day = date
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| anyhow::anyhow!("Invalid 23:59:59 for date {date}"))?
            .and_local_timezone(self.timezone)
            .earliest()
            .ok_or_else(|| {
                anyhow::anyhow!("Non-existent local end-of-day {date} in the configured timezone")
            })?
            .to_utc();

        // 2. Query events from calendar engine
        let events = self.engine.list(start_of_day, end_of_day).await?;

        // 3. Get journal filename for the date's month
        let filename = format!("{}.md", date.format("%Y.%m %B"));

        if !self.fs.exists(DIR_JOURNAL, &filename)? {
            return Ok(()); // No journal file — don't create one just for events
        }

        let content = self.fs.read(DIR_JOURNAL, &filename)?;

        // 4. Build the events section
        let section = if events.is_empty() {
            String::new()
        } else {
            self.build_event_section(date, &events)
        };

        // 5. Replace or append the section
        let new_content = replace_or_append_section(&content, "## Today's events", &section);

        // 6. Write back if changed
        if new_content != content {
            self.fs.write(DIR_JOURNAL, &filename, &new_content)?;
        }

        Ok(())
    }

    /// Build the markdown section for calendar events.
    fn build_event_section(&self, _date: NaiveDate, events: &[crate::Event]) -> String {
        let mut lines = vec!["## Today's events".to_string()];
        for e in events {
            let time = e.start.with_timezone(&self.timezone).format("%H:%M");
            let end = e.end.with_timezone(&self.timezone).format("%H:%M");
            let desc = e.description.as_deref().unwrap_or("");
            lines.push(format!(
                "- **{}–{}** [{}]({}) {}",
                time, end, e.title, e.filename, desc
            ));
        }
        lines.join("\n")
    }
}

/// Replace a section identified by its header, or append it if not found.
///
/// A section spans from its header line to the next `## ` header or end of
/// file. If `new_section` is empty the section is removed entirely.
fn replace_or_append_section(content: &str, section_header: &str, new_section: &str) -> String {
    let mut lines: Vec<&str> = content.lines().collect();
    let mut section_start: Option<usize> = None;
    let mut section_end: Option<usize> = None;

    // Find the section
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == section_header {
            section_start = Some(i);
        } else if section_start.is_some()
            && line.starts_with("## ")
            && line.trim() != section_header
        {
            section_end = Some(i);
            break;
        }
    }

    if let Some(start) = section_start {
        let end = section_end.unwrap_or(lines.len());
        if new_section.is_empty() {
            // Remove the section entirely
            lines.drain(start..end);
            // Also remove preceding blank line if any
            if start > 0
                && start <= lines.len()
                && lines
                    .get(start - 1)
                    .map(|l| l.trim().is_empty())
                    .unwrap_or(false)
            {
                lines.remove(start - 1);
            }
        } else {
            // Replace the section
            let new_lines: Vec<&str> = new_section.lines().collect();
            lines.splice(start..end, new_lines);
        }
    } else if !new_section.is_empty() {
        // Append at the end
        lines.push("");
        for line in new_section.lines() {
            lines.push(line);
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── replace_or_append_section tests ─────────────────────

    #[test]
    fn append_section_to_empty_content() {
        let content = "## 7 June, Saturday\n\nSome notes.";
        let section = "## Today's events\n- **09:00–10:00** [Meeting](abc.ics) ";
        let result = replace_or_append_section(content, "## Today's events", section);
        assert!(result.contains("## Today's events"));
        assert!(result.contains("## 7 June, Saturday"));
        assert!(result.contains("Meeting"));
    }

    #[test]
    fn replace_existing_section() {
        let content = "\
## 7 June, Saturday

Some notes.

## Today's events
- **09:00–10:00** [Old Meeting](old.ics) 

## Another section
More text.";

        let section = "## Today's events\n- **11:00–12:00** [New Meeting](new.ics) ";
        let result = replace_or_append_section(content, "## Today's events", section);

        assert!(result.contains("New Meeting"));
        assert!(!result.contains("Old Meeting"));
        assert!(result.contains("## Another section"));
    }

    #[test]
    fn remove_existing_section() {
        let content = "\
## 7 June, Saturday

Some notes.

## Today's events
- **09:00–10:00** [Meeting](abc.ics) 

## Another section
More text.";

        let result = replace_or_append_section(content, "## Today's events", "");

        assert!(!result.contains("Today's events"));
        assert!(!result.contains("Meeting"));
        assert!(result.contains("## Another section"));
        assert!(result.contains("## 7 June, Saturday"));
    }

    #[test]
    fn no_change_when_section_empty_and_not_found() {
        let content = "## 7 June, Saturday\n\nSome notes.";
        let result = replace_or_append_section(content, "## Today's events", "");
        assert_eq!(result, content);
    }

    #[test]
    fn section_at_end_of_file() {
        let content = "## 7 June, Saturday\n\nSome notes.\n\n## Today's events\n- **09:00–10:00** [Meeting](abc.ics) ";
        let section = "## Today's events\n- **14:00–15:00** [Standup](def.ics) ";
        let result = replace_or_append_section(content, "## Today's events", section);

        assert!(result.contains("Standup"));
        assert!(!result.contains("Meeting"));
        assert!(result.contains("## 7 June, Saturday"));
    }

    #[test]
    fn remove_section_at_end_of_file() {
        let content = "## 7 June, Saturday\n\nSome notes.\n\n## Today's events\n- **09:00–10:00** [Meeting](abc.ics) ";
        let result = replace_or_append_section(content, "## Today's events", "");

        assert!(!result.contains("Today's events"));
        assert!(result.contains("Some notes."));
    }

    // ── build_event_section tests ───────────────────────────

    #[test]
    fn build_event_section_formats_correctly() {
        use crate::types::EventSource;
        use chrono::{TimeZone, Utc};

        let fs = VirtualFs::new(tempfile::tempdir().unwrap().path().to_path_buf()).unwrap();
        let tz = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let engine =
            std::sync::Arc::new(CalendarEngine::new_blocking(dir.path().to_path_buf()).unwrap());

        let bridge = JournalBridge::new(engine, fs, tz);

        let date = NaiveDate::from_ymd_opt(2026, 6, 7).unwrap();
        let events = vec![crate::Event {
            uid: "test-uid".to_string(),
            title: "Team Standup".to_string(),
            start: Utc.with_ymd_and_hms(2026, 6, 7, 0, 0, 0).unwrap(), // 09:00 KST
            end: Utc.with_ymd_and_hms(2026, 6, 7, 0, 30, 0).unwrap(),  // 09:30 KST
            all_day: false,
            description: Some("Daily sync".to_string()),
            location: None,
            rrule: None,
            status: "CONFIRMED".to_string(),
            source: EventSource::Agent,
            filename: "abc123.ics".to_string(),
            note_path: None,
        }];

        let section = bridge.build_event_section(date, &events);

        assert!(section.starts_with("## Today's events"));
        assert!(section.contains("09:00"));
        assert!(section.contains("09:30"));
        assert!(section.contains("Team Standup"));
        assert!(section.contains("abc123.ics"));
        assert!(section.contains("Daily sync"));
    }

    #[test]
    fn build_event_section_empty_description() {
        use crate::types::EventSource;
        use chrono::{TimeZone, Utc};

        let fs = VirtualFs::new(tempfile::tempdir().unwrap().path().to_path_buf()).unwrap();
        let tz = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let engine =
            std::sync::Arc::new(CalendarEngine::new_blocking(dir.path().to_path_buf()).unwrap());

        let bridge = JournalBridge::new(engine, fs, tz);

        let date = NaiveDate::from_ymd_opt(2026, 6, 7).unwrap();
        let events = vec![crate::Event {
            uid: "test-uid".to_string(),
            title: "Lunch".to_string(),
            start: Utc.with_ymd_and_hms(2026, 6, 7, 3, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 7, 4, 0, 0).unwrap(),
            all_day: false,
            description: None,
            location: None,
            rrule: None,
            status: "CONFIRMED".to_string(),
            source: EventSource::User,
            filename: "lunch.ics".to_string(),
            note_path: None,
        }];

        let section = bridge.build_event_section(date, &events);

        // Empty description should produce empty string after filename
        let line = section.lines().nth(1).unwrap();
        assert!(line.contains("Lunch"));
        assert!(line.contains("lunch.ics"));
        // The description part should be empty (just trailing space)
        assert!(line.ends_with(' '));
    }
}
