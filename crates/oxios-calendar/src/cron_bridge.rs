//! Cron bridge — converts cron job schedules into synthetic calendar events.
//!
//! This module takes a list of [`CronJobDef`] definitions (simplified representations
//! of cron jobs, independent of the kernel's `CronScheduler`) and expands each enabled
//! job's schedule into concrete fire times within a given time window. Each fire time
//! becomes a 5-minute [`CronSyntheticEvent`] with a ⚙️ prefix, suitable for rendering
//! on the calendar UI.

use chrono::{DateTime, Duration, Utc};
use std::str::FromStr;

/// A simplified cron job definition for the bridge.
///
/// This is a decoupled representation of a cron job. The kernel's
/// `CronScheduler` maps its internal `CronJob` structs into these
/// definitions before passing them to the calendar bridge, avoiding
/// a circular dependency between `oxios-kernel` and `oxios-calendar`.
#[derive(Debug, Clone)]
pub struct CronJobDef {
    /// Job identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Cron expression (5-field, e.g. `"0 8 * * *"`).
    pub schedule: String,
    /// Whether the job is active.
    pub enabled: bool,
}

/// A synthetic event produced from a cron job expansion.
///
/// Each event represents a single fire time of a cron job, with a
/// fixed 5-minute duration. These events are not persisted as `.ics`
/// files — they exist only in memory for calendar display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CronSyntheticEvent {
    /// Job ID.
    pub job_id: String,
    /// Display title (e.g., `"⚙️ Morning Digest"`).
    pub title: String,
    /// Fire time (event start).
    pub start: DateTime<Utc>,
    /// Estimated end (fire time + 5 minutes).
    pub end: DateTime<Utc>,
}

/// Normalize a cron expression to 7-field format (sec min hour dom month dow year).
///
/// The `cron` crate requires 7 fields. Users commonly provide 5-field Linux-style
/// expressions (`min hour dom month dow`). This function prepends `"0 "` (run at second 0)
/// and appends `" *"` (any year) to convert 5-field expressions.
///
/// Expressions that already have 6 or 7 fields are returned unchanged.
fn normalize_cron_expr(expr: &str) -> String {
    let field_count = expr.split_whitespace().count();
    if field_count == 5 {
        format!("0 {expr} *")
    } else {
        expr.to_string()
    }
}

/// Expand cron jobs into synthetic events within a time range.
///
/// Each enabled cron job's schedule is parsed and all fire times
/// within `[from, to)` are collected. Each fire time becomes a
/// 5-minute synthetic event with a ⚙️ prefix.
///
/// Disabled jobs are skipped. Jobs with invalid cron expressions
/// are logged as warnings and skipped.
///
/// # Example
///
/// ```
/// use oxios_calendar::cron_bridge::{CronJobDef, expand_cron_events};
/// use chrono::{TimeZone, Utc, Duration};
///
/// let jobs = vec![CronJobDef {
///     id: "morning-digest".into(),
///     name: "Morning Digest".into(),
///     schedule: "0 8 * * *".into(),
///     enabled: true,
/// }];
///
/// let from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
/// let to = from + Duration::days(3);
///
/// let events = expand_cron_events(&jobs, from, to);
/// assert_eq!(events.len(), 3); // one per day
/// ```
pub fn expand_cron_events(
    jobs: &[CronJobDef],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<CronSyntheticEvent> {
    let mut events = Vec::new();

    for job in jobs {
        if !job.enabled {
            continue;
        }

        // Normalize 5-field (Linux-style) cron to 7-field (sec min hour dom month dow year)
        let normalized = normalize_cron_expr(&job.schedule);

        let schedule = match cron::Schedule::from_str(&normalized) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(job_id = %job.id, error = %e, "Invalid cron expression, skipping");
                continue;
            }
        };

        for fire in schedule.after(&from) {
            if fire >= to {
                break;
            }
            events.push(CronSyntheticEvent {
                job_id: job.id.clone(),
                title: format!("⚙️ {}", job.name),
                start: fire,
                end: fire + Duration::minutes(5),
            });
        }
    }

    events.sort_by_key(|e| e.start);
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn job(id: &str, name: &str, schedule: &str, enabled: bool) -> CronJobDef {
        CronJobDef {
            id: id.to_string(),
            name: name.to_string(),
            schedule: schedule.to_string(),
            enabled,
        }
    }

    #[test]
    fn normalize_five_field() {
        assert_eq!(super::normalize_cron_expr("0 8 * * *"), "0 0 8 * * * *");
    }

    #[test]
    fn normalize_seven_field_unchanged() {
        assert_eq!(super::normalize_cron_expr("0 0 8 * * * *"), "0 0 8 * * * *");
    }

    #[test]
    fn daily_cron_produces_one_event_per_day() {
        let jobs = vec![job("morning", "Morning Digest", "0 8 * * *", true)];
        let from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let to = from + Duration::days(3);

        let events = expand_cron_events(&jobs, from, to);

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].title, "⚙️ Morning Digest");
        assert_eq!(events[0].job_id, "morning");
        assert_eq!(events[0].end, events[0].start + Duration::minutes(5));
    }

    #[test]
    fn hourly_cron_spanning_two_days() {
        let jobs = vec![job("hourly", "Hourly Check", "0 * * * *", true)];
        let from = Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap();
        let to = from + Duration::days(2);

        let events = expand_cron_events(&jobs, from, to);

        // `after(&from)` returns fire times strictly after `from`.
        // With from=00:00 and hourly cron at minute 0, first fire is 01:00.
        // Over 2 days: 01:00 day1 .. 23:00 day2 = 47 events.
        assert_eq!(events.len(), 47);
        // Verify sorted
        for window in events.windows(2) {
            assert!(window[0].start < window[1].start);
        }
    }

    #[test]
    fn disabled_jobs_are_skipped() {
        let jobs = vec![job("disabled", "Disabled Job", "0 8 * * *", false)];
        let from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let to = from + Duration::days(1);

        let events = expand_cron_events(&jobs, from, to);

        assert!(events.is_empty());
    }

    #[test]
    fn invalid_cron_expression_is_skipped() {
        let jobs = vec![job("bad", "Bad Job", "not a cron", true)];
        let from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let to = from + Duration::days(1);

        let events = expand_cron_events(&jobs, from, to);

        assert!(events.is_empty());
    }

    #[test]
    fn multiple_jobs_are_interleaved_and_sorted() {
        let jobs = vec![
            job("evening", "Evening Report", "0 18 * * *", true),
            job("morning", "Morning Digest", "0 8 * * *", true),
        ];
        let from = Utc.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap();
        let to = from + Duration::days(2);

        let events = expand_cron_events(&jobs, from, to);

        // 2 jobs × 2 days = 4 events
        assert_eq!(events.len(), 4);
        // Morning (8:00) comes before evening (18:00) on each day
        assert!(events[0].start < events[1].start);
        assert!(events[1].start < events[2].start);
        assert!(events[2].start < events[3].start);
    }

    #[test]
    fn empty_jobs_list_produces_no_events() {
        let from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let to = from + Duration::days(1);

        let events = expand_cron_events(&[], from, to);

        assert!(events.is_empty());
    }

    #[test]
    fn range_with_no_fire_times_produces_no_events() {
        // Cron fires at 08:00, but our window is 09:00–10:00
        let jobs = vec![job("morning", "Morning", "0 8 * * *", true)];
        let from = Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap();

        let events = expand_cron_events(&jobs, from, to);

        assert!(events.is_empty());
    }

    #[test]
    fn event_duration_is_five_minutes() {
        let jobs = vec![job("once", "One Shot", "0 12 15 6 *", true)]; // June 15 at 12:00
        let from = Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap();

        let events = expand_cron_events(&jobs, from, to);

        assert_eq!(events.len(), 1);
        let evt = &events[0];
        assert_eq!(evt.end - evt.start, Duration::minutes(5));
        assert_eq!(
            evt.start,
            Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()
        );
    }
}
