//! iCalendar (.ics) build and parse utilities.

use chrono::{Duration, Utc};
use icalendar::{
    Calendar, CalendarDateTime, Component, DatePerhapsTime, Event as ICalEvent, EventLike,
};

use crate::types::{Event, EventDraft, EventSource};

/// Custom X-SOURCE property key.
const X_SOURCE: &str = "X-SOURCE";

/// Build a complete .ics file string from an [`EventDraft`].
///
/// The generated .ics contains a single `VCALENDAR` with one `VEVENT`.
/// Reminders are encoded as `VALARM` components with `DISPLAY` action.
pub fn build_ics(uid: &str, draft: &EventDraft) -> anyhow::Result<String> {
    let mut event = ICalEvent::new();
    event.uid(uid);
    event.summary(&draft.title);
    event.timestamp(Utc::now());

    if draft.all_day {
        // For all-day events, use date-only values.
        let start_date = draft.start.date_naive();
        // All-day events: end date is exclusive in iCalendar. If end == start,
        // set end = start + 1 day.
        let end_date = if draft.end.date_naive() == start_date {
            start_date + chrono::Duration::days(1)
        } else {
            draft.end.date_naive()
        };
        event.all_day(start_date);
        // Set DTEND explicitly for multi-day all-day events.
        if end_date != start_date + chrono::Duration::days(1) {
            event.append_property(icalendar::Property::new(
                "DTEND",
                format_date_only(end_date),
            ));
        }
    } else {
        event.starts(draft.start);
        event.ends(draft.end);
    }

    if let Some(ref desc) = draft.description {
        event.description(desc);
    }

    if let Some(ref loc) = draft.location {
        event.location(loc);
    }

    // RRULE
    if let Some(ref repeat) = draft.repeat {
        let rrule_str = crate::rrule::simple_repeat_to_rrule(repeat);
        event.append_property(icalendar::Property::new("RRULE", &rrule_str));
    }

    // Reminders as VALARM
    for &minutes in &draft.reminder_minutes {
        let trigger = -Duration::minutes(minutes as i64);
        event.alarm(icalendar::Alarm::display(
            &format!("Reminder: {}", draft.title),
            trigger,
        ));
    }

    // Store source as custom property
    let source_str = match draft.source {
        EventSource::Agent => "agent",
        EventSource::User => "user",
        EventSource::Cron => "cron",
    };
    event.append_property(icalendar::Property::new(X_SOURCE, source_str));

    // Default status: CONFIRMED
    event.status(icalendar::EventStatus::Confirmed);

    let mut calendar = Calendar::new();
    calendar.push(event.done());

    Ok(calendar.to_string())
}

/// Parse an .ics file content into an [`Event`].
///
/// Extracts the first `VEVENT` component found in the calendar.
/// The `filename` parameter is stored verbatim in the resulting [`Event`].
pub fn parse_ics(content: &str, filename: &str) -> anyhow::Result<Event> {
    // read_calendar returns a Calendar struct with borrowed data.
    // We convert to owned CalendarComponent immediately.
    let parsed = icalendar::parser::read_calendar(content)
        .map_err(|e| anyhow::anyhow!("ICS parse error: {e}"))?;

    // Find the first VEVENT component
    let mut found_event: Option<ICalEvent> = None;
    for cal_component in &parsed.components {
        if cal_component.name.as_ref() == "VEVENT" {
            let cc: icalendar::CalendarComponent = (*cal_component).clone().into();
            if let icalendar::CalendarComponent::Event(evt) = cc {
                found_event = Some(evt);
                break;
            }
        }
    }

    let event = found_event.ok_or_else(|| anyhow::anyhow!("No VEVENT found in .ics file"))?;

    // UID
    let uid = event.get_uid().unwrap_or_default().to_string();

    // Summary / title
    let title = event.get_summary().unwrap_or("(untitled)").to_string();

    // Description
    let description = event.get_description().map(|s| s.to_string());

    // Location
    let location = event.get_location().map(|s| s.to_string());

    // Status
    let status = match event.get_status() {
        Some(icalendar::EventStatus::Confirmed) => "CONFIRMED",
        Some(icalendar::EventStatus::Tentative) => "TENTATIVE",
        Some(icalendar::EventStatus::Cancelled) => "CANCELLED",
        None => "CONFIRMED",
    }
    .to_string();

    // Start / End
    let (start, end, all_day) = extract_datetimes(&event)?;

    // RRULE
    let rrule = event.property_value("RRULE").map(|s| s.to_string());

    // Source (custom X-SOURCE)
    let source = event
        .property_value(X_SOURCE)
        .and_then(|s| match s {
            "agent" => Some(EventSource::Agent),
            "user" => Some(EventSource::User),
            "cron" => Some(EventSource::Cron),
            _ => None,
        })
        .unwrap_or(EventSource::Agent);

    Ok(Event {
        uid,
        title,
        start,
        end,
        all_day,
        description,
        location,
        rrule,
        status,
        source,
        filename: filename.to_string(),
    })
}

/// Extract start/end datetimes and all-day flag from an icalendar Event.
fn extract_datetimes(
    event: &ICalEvent,
) -> anyhow::Result<(chrono::DateTime<Utc>, chrono::DateTime<Utc>, bool)> {
    let start_opt = event.get_start();
    let end_opt = event.get_end();

    match (start_opt, end_opt) {
        // Both DATE (all-day)
        (Some(DatePerhapsTime::Date(ds)), Some(DatePerhapsTime::Date(de))) => {
            let start = ds.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = de.and_hms_opt(0, 0, 0).unwrap().and_utc();
            Ok((start, end, true))
        }
        // Start DATE, end missing (single all-day)
        (Some(DatePerhapsTime::Date(ds)), None) => {
            let start = ds.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = (ds + chrono::Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            Ok((start, end, true))
        }
        // DATETIME variants
        (
            Some(DatePerhapsTime::DateTime(CalendarDateTime::Utc(ds))),
            Some(DatePerhapsTime::DateTime(CalendarDateTime::Utc(de))),
        ) => Ok((ds, de, false)),
        (Some(DatePerhapsTime::DateTime(CalendarDateTime::Utc(ds))), None) => {
            // Default duration: 1 hour
            Ok((ds, ds + chrono::Duration::hours(1), false))
        }
        // Floating datetime: treat as UTC
        (
            Some(DatePerhapsTime::DateTime(CalendarDateTime::Floating(ds))),
            Some(DatePerhapsTime::DateTime(CalendarDateTime::Floating(de))),
        ) => Ok((ds.and_utc(), de.and_utc(), false)),
        (Some(DatePerhapsTime::DateTime(CalendarDateTime::Floating(ds))), None) => Ok((
            ds.and_utc(),
            ds.and_utc() + chrono::Duration::hours(1),
            false,
        )),
        // With timezone: convert to UTC
        (
            Some(DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
                date_time: ds,
                tzid,
            })),
            Some(DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
                date_time: de,
                tzid: _,
            })),
        ) => {
            let start = convert_tz_to_utc(ds, &tzid)?;
            let end = convert_tz_to_utc(de, &tzid)?;
            Ok((start, end, false))
        }
        (
            Some(DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
                date_time: ds,
                tzid,
            })),
            None,
        ) => {
            let start = convert_tz_to_utc(ds, &tzid)?;
            Ok((start, start + chrono::Duration::hours(1), false))
        }
        // Mixed datetime types: best-effort conversion (catch-all for Some, Some)
        (Some(start_dpt), Some(end_dpt)) => {
            let start = date_perhaps_time_to_utc(&start_dpt)?;
            let end = date_perhaps_time_to_utc(&end_dpt)?;
            let all_day = matches!(start_dpt, DatePerhapsTime::Date(_));
            Ok((start, end, all_day))
        }
        (None, _) => Err(anyhow::anyhow!("No DTSTART found in event")),
    }
}

/// Convert a timezone-aware naive datetime to UTC.
///
/// F11: a recurring event whose local time lands in a fall-back overlap
/// (ambiguous — occurs twice) is resolved to the *earliest* interpretation
/// with a warning, rather than being rejected. Spring-forward gaps (no valid
/// local time) are still surfaced as an explicit error since any choice would
/// be a guess.
fn convert_tz_to_utc(
    dt: chrono::NaiveDateTime,
    tzid: &str,
) -> anyhow::Result<chrono::DateTime<Utc>> {
    let tz: chrono_tz::Tz = tzid
        .parse()
        .map_err(|_| anyhow::anyhow!("Unknown timezone: {tzid}"))?;
    match dt.and_local_timezone(tz) {
        chrono::LocalResult::Single(local) => Ok(local.to_utc()),
        chrono::LocalResult::Ambiguous(earliest, _latest) => {
            tracing::warn!(
                tzid,
                naive = %dt,
                "Ambiguous local time (fall-back DST overlap); using earliest interpretation"
            );
            Ok(earliest.to_utc())
        }
        chrono::LocalResult::None => Err(anyhow::anyhow!(
            "Non-existent local time {dt} in timezone {tzid} (spring-forward gap)"
        )),
    }
}

/// Convert a [`DatePerhapsTime`] to UTC, treating dates as midnight.
fn date_perhaps_time_to_utc(dpt: &DatePerhapsTime) -> anyhow::Result<chrono::DateTime<Utc>> {
    match dpt {
        DatePerhapsTime::Date(d) => Ok(d.and_hms_opt(0, 0, 0).unwrap().and_utc()),
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(dt)) => Ok(*dt),
        DatePerhapsTime::DateTime(CalendarDateTime::Floating(dt)) => Ok(dt.and_utc()),
        DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { date_time, tzid }) => {
            convert_tz_to_utc(*date_time, tzid)
        }
    }
}

/// Format a [`chrono::NaiveDate`] as a date-only iCalendar value (YYYYMMDD).
fn format_date_only(date: chrono::NaiveDate) -> String {
    format!("{}", date.format("%Y%m%d"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn roundtrip_timed_event() {
        let draft = EventDraft {
            title: "Test Meeting".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 10, 10, 0, 0).unwrap(),
            all_day: false,
            description: Some("A test meeting".into()),
            location: Some("Room 42".into()),
            repeat: None,
            reminder_minutes: vec![15],
            source: EventSource::User,
        };

        let ics = build_ics("test-uid-123", &draft).unwrap();
        assert!(ics.contains("BEGIN:VCALENDAR"));
        assert!(ics.contains("SUMMARY:Test Meeting"));
        assert!(ics.contains("UID:test-uid-123"));

        let event = parse_ics(&ics, "test-uid-123.ics").unwrap();
        assert_eq!(event.uid, "test-uid-123");
        assert_eq!(event.title, "Test Meeting");
        assert_eq!(event.description, Some("A test meeting".into()));
        assert_eq!(event.location, Some("Room 42".into()));
        assert!(!event.all_day);
        assert_eq!(event.source, EventSource::User);
    }

    #[test]
    fn roundtrip_allday_event() {
        let draft = EventDraft {
            title: "Birthday".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap(),
            all_day: true,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: EventSource::Agent,
        };

        let ics = build_ics("bday-uid", &draft).unwrap();
        let event = parse_ics(&ics, "bday-uid.ics").unwrap();
        assert!(event.all_day);
        assert_eq!(event.title, "Birthday");
    }
}
