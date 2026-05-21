//! World clock plugin — generate time reports for multiple timezones.
//!
//! Ported from files.md (`server/plugins/world_clock.go`) by Artem Zakirullin.

use chrono::{TimeZone, Utc};
use chrono_tz::Tz;

/// A timezone entry in the world clock report.
#[derive(Debug, Clone)]
pub struct TimezoneEntry {
    /// Display name (e.g., "MSK").
    pub name: String,
    /// Icon for this timezone.
    pub icon: String,
    /// Formatted current/converted time.
    pub current_time: String,
}

/// Built-in timezone definitions: (name, icon, IANA timezone).
const TIMEZONES: &[(&str, &str, &str)] = &[
    ("UTC", "🕰", "UTC"),
    ("MSK", "🔺", "Europe/Moscow"),
    ("CY", "🏝", "Asia/Nicosia"),
    ("ME", "⛰", "Europe/Podgorica"),
];

/// Default timezone names (in order).
pub fn default_timezone_names() -> Vec<&'static str> {
    TIMEZONES.iter().map(|(name, _, _)| *name).collect()
}

/// Generate world clock report for the current moment.
pub fn world_clock_now() -> Vec<TimezoneEntry> {
    world_clock_for_names(&default_timezone_names())
}

/// Generate world clock report for given timezone names.
pub fn world_clock_for_names(timezone_names: &[&str]) -> Vec<TimezoneEntry> {
    let now = Utc::now();
    timezone_names
        .iter()
        .filter_map(|name| {
            let (icon, tz_str) = find_tz(name)?;
            let time = format_time(&now, tz_str);
            Some(TimezoneEntry {
                name: name.to_string(),
                icon: icon.to_string(),
                current_time: time,
            })
        })
        .collect()
}

/// Try to parse a message as a date (DD.MM.YYYY) and show it in all timezones.
/// Returns None if the message is not a valid date.
pub fn parse_and_show_date(msg: &str) -> Option<Vec<TimezoneEntry>> {
    let date = chrono::NaiveDate::parse_from_str(msg.trim(), "%d.%m.%Y").ok()?;
    let time = date.and_hms_opt(0, 0, 0)?;
    let utc_dt = Utc.from_utc_datetime(&time);
    Some(show_timestamp(&utc_dt))
}

/// Try to parse a message as a datetime (DD.MM.YYYY HH:MM:SS) and show it in all timezones.
/// Returns None if the message is not a valid datetime.
pub fn parse_and_show_time(msg: &str) -> Option<Vec<TimezoneEntry>> {
    let time = chrono::NaiveDateTime::parse_from_str(msg.trim(), "%d.%m.%Y %H:%M:%S").ok()?;
    let utc_dt = Utc.from_utc_datetime(&time);
    Some(show_time(&utc_dt))
}

/// Try to parse a message as a Unix timestamp and show it in all timezones.
/// Handles seconds (10 digits), milliseconds (13 digits), and microseconds (16 digits).
/// Returns None if the message is not a valid timestamp.
pub fn parse_and_show_timestamp(msg: &str) -> Option<Vec<TimezoneEntry>> {
    let ts: i64 = msg.trim().parse().ok()?;
    if ts <= 999_999 {
        return None;
    }
    let utc_dt = if ts > 9_999_999_999_999 {
        // microseconds
        chrono::DateTime::from_timestamp_micros(ts)?
    } else if ts > 9_999_999_999 {
        // milliseconds
        chrono::DateTime::from_timestamp_millis(ts)?
    } else {
        // seconds
        Utc.timestamp_opt(ts, 0).single()?
    };
    Some(show_time(&utc_dt))
}

/// Check if the message can be handled by this plugin.
pub fn can_handle(msg: &str) -> bool {
    parse_and_show_date(msg).is_some()
        || parse_and_show_time(msg).is_some()
        || parse_and_show_timestamp(msg).is_some()
}

/// Handle a message: try date, time, and timestamp parsing.
pub fn handle(msg: &str) -> Option<Vec<TimezoneEntry>> {
    if let Some(entries) = parse_and_show_date(msg) {
        return Some(entries);
    }
    if let Some(entries) = parse_and_show_time(msg) {
        return Some(entries);
    }
    if let Some(entries) = parse_and_show_timestamp(msg) {
        return Some(entries);
    }
    None
}

/// Format the world clock report as a string.
pub fn format_report(entries: &[TimezoneEntry]) -> String {
    entries
        .iter()
        .map(|e| format!("{} {} {}", e.icon, e.current_time, e.name))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Internal helpers ────────────────────────────────────────

fn find_tz(name: &str) -> Option<(&'static str, &'static str)> {
    TIMEZONES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, icon, tz)| (*icon, *tz))
}

fn format_time(utc_dt: &chrono::DateTime<Utc>, tz_str: &str) -> String {
    if tz_str == "UTC" {
        utc_dt.format("%d.%m.%Y %H:%M:%S").to_string()
    } else if let Ok(tz) = tz_str.parse::<Tz>() {
        let local = utc_dt.with_timezone(&tz);
        local.format("%d.%m.%Y %H:%M:%S").to_string()
    } else {
        // Fallback: try FixedOffset
        utc_dt.format("%d.%m.%Y %H:%M:%S").to_string()
    }
}

fn show_timestamp(utc_dt: &chrono::DateTime<Utc>) -> Vec<TimezoneEntry> {
    show_impl(utc_dt, format_time)
}

fn show_time(utc_dt: &chrono::DateTime<Utc>) -> Vec<TimezoneEntry> {
    show_impl(utc_dt, format_time)
}

fn show_impl<F>(utc_dt: &chrono::DateTime<Utc>, formatter: F) -> Vec<TimezoneEntry>
where
    F: Fn(&chrono::DateTime<Utc>, &str) -> String,
{
    TIMEZONES
        .iter()
        .map(|(name, icon, tz_str)| TimezoneEntry {
            name: name.to_string(),
            icon: icon.to_string(),
            current_time: formatter(utc_dt, tz_str),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_clock_now() {
        let entries = world_clock_now();
        assert!(entries.len() >= 4);
        assert!(entries.iter().any(|e| e.name == "UTC"));
        assert!(entries.iter().any(|e| e.name == "MSK"));
    }

    #[test]
    fn test_parse_date() {
        let result = parse_and_show_date("01.06.2024");
        assert!(result.is_some());
        let entries = result.unwrap();
        assert!(entries[0].current_time.contains("01.06.2024"));
    }

    #[test]
    fn test_parse_date_invalid() {
        assert!(parse_and_show_date("not a date").is_none());
    }

    #[test]
    fn test_parse_time() {
        let result = parse_and_show_time("01.06.2024 12:30:45");
        assert!(result.is_some());
        let entries = result.unwrap();
        assert!(entries[0].current_time.contains("12:30:45"));
    }

    #[test]
    fn test_parse_timestamp_seconds() {
        let result = parse_and_show_timestamp("1717237200");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_timestamp_millis() {
        let result = parse_and_show_timestamp("1717237200000");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_timestamp_micros() {
        let result = parse_and_show_timestamp("1717237200000000");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_and_show_timestamp("123").is_none());
        assert!(parse_and_show_timestamp("abc").is_none());
    }

    #[test]
    fn test_can_handle() {
        assert!(can_handle("01.06.2024"));
        assert!(can_handle("01.06.2024 12:30:00"));
        assert!(can_handle("1717237200"));
        assert!(!can_handle("hello world"));
    }

    #[test]
    fn test_format_report() {
        let entries = vec![TimezoneEntry {
            name: "UTC".into(),
            icon: "🕰".into(),
            current_time: "01.06.2024 12:00:00".into(),
        }];
        let report = format_report(&entries);
        assert!(report.contains("🕰"));
        assert!(report.contains("UTC"));
    }
}
