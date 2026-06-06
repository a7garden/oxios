//! Convert simple [`Repeat`] structs to RRULE strings.

use crate::types::Repeat;

/// Convert a [`Repeat`] to an RRULE string (without the `RRULE:` prefix).
///
/// # Example
///
/// ```
/// use oxios_calendar::rrule::simple_repeat_to_rrule;
/// use oxios_calendar::types::Repeat;
///
/// let r = Repeat {
///     frequency: "weekly".into(),
///     days: vec!["mon".into(), "wed".into(), "fri".into()],
///     interval: 1,
///     until: None,
///     count: None,
/// };
/// let s = simple_repeat_to_rrule(&r);
/// assert!(s.contains("FREQ=WEEKLY"));
/// assert!(s.contains("BYDAY=MO,WE,FR"));
/// ```
pub fn simple_repeat_to_rrule(r: &Repeat) -> String {
    let mut parts = Vec::new();

    // Frequency
    let freq: String = match r.frequency.to_lowercase().as_str() {
        "daily" => "DAILY".to_string(),
        "weekly" => "WEEKLY".to_string(),
        "monthly" => "MONTHLY".to_string(),
        "yearly" => "YEARLY".to_string(),
        other => other.to_uppercase(),
    };
    parts.push(format!("FREQ={freq}"));

    // Interval
    if r.interval > 1 {
        parts.push(format!("INTERVAL={}", r.interval));
    }

    // Days (for weekly)
    if !r.days.is_empty() {
        let days: Vec<String> = r.days.iter().map(|d| day_to_ical(d)).collect();
        parts.push(format!("BYDAY={}", days.join(",")));
    }

    // Until
    if let Some(ref until) = r.until {
        // Accept ISO date (YYYY-MM-DD) and convert to YYYYMMDD
        let until_val = if until.contains('-') {
            until.replace('-', "")
        } else {
            until.clone()
        };
        parts.push(format!("UNTIL={until_val}"));
    }

    // Count
    if let Some(count) = r.count {
        parts.push(format!("COUNT={count}"));
    }

    parts.join(";")
}

/// Convert a day abbreviation to iCalendar BYDAY format.
fn day_to_ical(day: &str) -> String {
    match day.to_lowercase().as_str() {
        "mon" | "monday" => "MO".to_string(),
        "tue" | "tuesday" => "TU".to_string(),
        "wed" | "wednesday" => "WE".to_string(),
        "thu" | "thursday" => "TH".to_string(),
        "fri" | "friday" => "FR".to_string(),
        "sat" | "saturday" => "SA".to_string(),
        "sun" | "sunday" => "SU".to_string(),
        other => other.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_default() {
        let r = Repeat {
            frequency: "daily".into(),
            days: vec![],
            interval: 1,
            until: None,
            count: None,
        };
        assert_eq!(simple_repeat_to_rrule(&r), "FREQ=DAILY");
    }

    #[test]
    fn weekly_with_days_and_interval() {
        let r = Repeat {
            frequency: "weekly".into(),
            days: vec!["mon".into(), "wed".into(), "fri".into()],
            interval: 2,
            until: None,
            count: Some(10),
        };
        let s = simple_repeat_to_rrule(&r);
        assert_eq!(s, "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE,FR;COUNT=10");
    }

    #[test]
    fn monthly_with_until() {
        let r = Repeat {
            frequency: "monthly".into(),
            days: vec![],
            interval: 1,
            until: Some("2026-12-31".into()),
            count: None,
        };
        let s = simple_repeat_to_rrule(&r);
        assert_eq!(s, "FREQ=MONTHLY;UNTIL=20261231");
    }
}
