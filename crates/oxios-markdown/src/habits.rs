//! Habit tracking.
//!
//! Ported from files.md (`server/habits/mod.rs`) by Artem Zakirullin.
//! Reads/writes habit data from the insights directory.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{Datelike, TimeZone};
use unicode_segmentation::UnicodeSegmentation;

use crate::fs::VirtualFs;
use crate::parser::norm_new_lines;
use crate::types::{
    FsError, Habits, YearHabits, DIR_HABITS, DIR_INSIGHTS, HABIT_COMPLETED,
    HABIT_COMPLETED_AT_WEEKEND, HABIT_SKIPPED, MD_EXT, MOOD_EMOJIS, MOOD_HABIT,
};

/// Habits-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum HabitsError {
    /// Malformed month line in habits file.
    #[error("malformed month line")]
    MalformedMonthLine,
    /// Other error.
    #[error("{0}")]
    Other(String),
}

impl From<FsError> for HabitsError {
    fn from(e: FsError) -> Self {
        HabitsError::Other(e.to_string())
    }
}

/// Read habits for a given year.
pub fn habits(fs: &VirtualFs, year: i32) -> Result<Habits, HabitsError> {
    let existing = fs.files_and_dirs(DIR_HABITS)?;
    let mut habits: Habits = HashMap::new();
    for entry in &existing {
        habits.insert(entry.display_name.clone(), HashMap::new());
    }

    let filename = format!("{} Habits.md", year);
    if !fs.exists(DIR_INSIGHTS, &filename)? {
        return Ok(habits);
    }

    let content = fs.read(DIR_INSIGHTS, &filename)?;
    let normalized = norm_new_lines(&content);
    let mut month = chrono::Month::January;

    for line in normalized.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("###") {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.len() >= 2 {
                if let Ok(m) = chrono::Month::from_str(parts[1]) {
                    month = m;
                }
            }
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() < 2 {
            continue;
        }

        let days = parts[0];
        let habit = parts[1];
        let first_day =
            chrono::NaiveDate::from_ymd_opt(year, month.number_from_month(), 1).unwrap();
        let mut day_of_year = first_day.ordinal() as i32;

        if habit.contains(MOOD_HABIT) {
            let moods = habits.entry(MOOD_HABIT.to_string()).or_default();
            for gr in days.graphemes(true) {
                let power = MOOD_EMOJIS.iter().position(|&e| e == gr).unwrap_or(0) as i32;
                moods.insert(day_of_year, power);
                day_of_year += 1;
            }
            continue;
        }

        let marker = format!(
            "{}{}{}",
            HABIT_SKIPPED, HABIT_COMPLETED_AT_WEEKEND, HABIT_COMPLETED
        );
        if !days.contains(marker.chars().next().unwrap().to_string().as_str()) {
            continue;
        }

        let name = habit.trim();
        let year_habits = habits.entry(name.to_string()).or_default();
        for gr in days.graphemes(true) {
            year_habits.insert(day_of_year, if gr != HABIT_SKIPPED { 1 } else { 0 });
            day_of_year += 1;
        }
    }
    Ok(habits)
}

/// Get emoji for a habit status.
pub fn emoji_for_status(
    habit_name: &str,
    day: &chrono::DateTime<chrono::FixedOffset>,
    status: i32,
) -> &'static str {
    if habit_name == MOOD_HABIT {
        return MOOD_EMOJIS.get(status as usize).unwrap_or(&HABIT_SKIPPED);
    }
    if status == 1 {
        if day.weekday().num_days_from_sunday() >= 5 {
            HABIT_COMPLETED_AT_WEEKEND
        } else {
            HABIT_COMPLETED
        }
    } else {
        HABIT_SKIPPED
    }
}

/// Get emoji for a habit from its definition file.
pub fn habit_emoji(fs: &VirtualFs, habit_name: &str) -> String {
    if let Ok(content) = fs.read(DIR_HABITS, &format!("{}{}", habit_name, MD_EXT)) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    weekday_emoji(habit_name).to_string()
}

/// Get emoji for a weekday or month name.
pub fn weekday_emoji(key: &str) -> &'static str {
    match key.to_lowercase().as_str() {
        "monday" => "🌑",
        "tuesday" => "🌒",
        "wednesday" => "🌓",
        "thursday" => "🌔",
        "friday" => "🌕",
        "saturday" => "🌝",
        "sunday" => "🌛",
        _ => "⚡️",
    }
}

/// Get last week's habits data.
///
/// Returns habit name → {day_of_year → status} for the current week
/// (Monday through Sunday). Includes habits from the `habits/` directory
/// plus the default Mood habit.
///
/// Ported from Go `LastWeekHabits`.
pub fn last_week_habits(fs: &VirtualFs, tz: chrono::FixedOffset) -> Result<Habits, HabitsError> {
    let now = chrono::Utc::now().with_timezone(&tz);
    let year = now.year();

    let habits_for_year = habits(fs, year)?;

    // Walk back to Monday of the current week
    let mut monday = now.date_naive();
    while monday.weekday() != chrono::Weekday::Mon {
        monday -= chrono::Duration::days(1);
    }

    // Collect existing habit names (from habits/ directory)
    let existing = fs.files_and_dirs(DIR_HABITS)?;
    let mut habit_names: Vec<String> = existing.iter().map(|e| e.display_name.clone()).collect();
    // Add default Mood habit (not in habits/ directory)
    if !habit_names.contains(&MOOD_HABIT.to_string()) {
        habit_names.push(MOOD_HABIT.to_string());
    }

    let mut result: Habits = HashMap::new();
    for name in &habit_names {
        let mut week: YearHabits = HashMap::new();
        for offset in 0..7i64 {
            let day = monday + chrono::Duration::days(offset);
            let year_day = day.ordinal() as i32;
            let status = habits_for_year
                .get(name)
                .and_then(|y| y.get(&year_day))
                .copied()
                .unwrap_or(0);
            week.insert(year_day, status);
        }
        result.insert(name.clone(), week);
    }

    Ok(result)
}

/// Write habits data for a year back to the insights file.
///
/// Generates `insights/{year} Habits.md` with month-by-month habit status.
/// Only months with at least one completed item are included.
///
/// Ported from Go `Write`.
pub fn write_habits(fs: &VirtualFs, year: i32, habits: &Habits) -> Result<(), HabitsError> {
    // Sort habit names alphabetically, Mood last
    let mut habit_keys: Vec<String> = habits
        .keys()
        .filter(|k| *k != MOOD_HABIT)
        .cloned()
        .collect();
    habit_keys.sort();
    if habits.contains_key(MOOD_HABIT) {
        habit_keys.push(MOOD_HABIT.to_string());
    }

    let mut content = String::new();
    let mut day = chrono::NaiveDate::from_ymd_opt(year, 1, 1).unwrap();

    while day.year() < year + 1 {
        let mut habits_for_month = String::new();

        for habit_name in &habit_keys {
            let mut statuses = String::new();
            let mut day_of_month = day;
            let mut at_least_one_completion = false;

            while day_of_month.month() == day.month() {
                let year_day = day_of_month.ordinal() as i32;
                let emoji = if let Some(status_map) = habits.get(habit_name) {
                    if let Some(&status) = status_map.get(&year_day) {
                        let dt = chrono::FixedOffset::east_opt(0)
                            .unwrap()
                            .from_utc_datetime(&day_of_month.and_hms_opt(12, 0, 0).unwrap());
                        let e = emoji_for_status(habit_name, &dt, status);
                        if e != HABIT_SKIPPED {
                            at_least_one_completion = true;
                        }
                        e
                    } else {
                        HABIT_SKIPPED
                    }
                } else {
                    HABIT_SKIPPED
                };
                statuses.push_str(emoji);
                day_of_month += chrono::Duration::days(1);
            }

            if at_least_one_completion {
                habits_for_month.push_str(&format!("{} {}\n", statuses, habit_name));
            }
        }

        if !habits_for_month.is_empty() {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&format!(
                "### {}\n{}",
                month_name(day.month()),
                habits_for_month
            ));
        }

        // Advance to the 1st of the next month
        day = chrono::NaiveDate::from_ymd_opt(
            if day.month() == 12 { year + 1 } else { year },
            if day.month() == 12 {
                1
            } else {
                day.month() + 1
            },
            1,
        )
        .unwrap();
    }

    let filename = format!("{} Habits.md", year);
    fs.write(DIR_INSIGHTS, &filename, &content)?;
    Ok(())
}

/// Get the English month name for a month number (1–12).
fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn test_fs() -> (VirtualFs, TempDir) {
        let dir = TempDir::new().unwrap();
        let fs = VirtualFs::new(dir.path().to_path_buf()).unwrap();
        (fs, dir)
    }

    #[test]
    fn test_emoji_for_status() {
        let saturday = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 6, 12, 0, 0)
            .unwrap();
        assert_eq!(
            emoji_for_status("Exercise", &saturday, 1),
            HABIT_COMPLETED_AT_WEEKEND
        );
        assert_eq!(emoji_for_status("Exercise", &saturday, 0), HABIT_SKIPPED);
    }

    #[test]
    fn test_mood_emoji() {
        let day = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 12, 0, 0)
            .unwrap();
        assert_eq!(emoji_for_status(MOOD_HABIT, &day, 0), HABIT_SKIPPED);
        assert_eq!(emoji_for_status(MOOD_HABIT, &day, 5), "😊");
    }

    #[test]
    fn test_weekday_emoji() {
        assert_eq!(weekday_emoji("monday"), "🌑");
        assert_eq!(weekday_emoji("unknown"), "⚡️");
    }

    #[test]
    fn test_last_week_habits_basic() {
        let (fs, _t) = test_fs();
        let tz = FixedOffset::east_opt(0).unwrap();

        // Create a habit file so it appears in the listing
        fs.make_dir(DIR_HABITS).unwrap();
        fs.write(DIR_HABITS, "Exercise.md", "\u{1F3CB}").unwrap();

        // Write some habit data for the current year
        let now = chrono::Utc::now().with_timezone(&tz);
        let year = now.year();
        let mut habits_data: Habits = HashMap::new();
        let mut year_map: YearHabits = HashMap::new();
        year_map.insert(1, 1); // day 1 completed
        habits_data.insert("Exercise".to_string(), year_map);

        write_habits(&fs, year, &habits_data).unwrap();

        let result = last_week_habits(&fs, tz).unwrap();
        assert!(result.contains_key("Exercise"));
        assert!(result.contains_key(MOOD_HABIT));
        // Should have exactly 7 entries per habit (Mon-Sun)
        assert_eq!(result.get("Exercise").unwrap().len(), 7);
    }

    #[test]
    fn test_write_habits_empty() {
        let (fs, _t) = test_fs();
        let habits: Habits = HashMap::new();
        write_habits(&fs, 2024, &habits).unwrap();

        let filename = "2024 Habits.md";
        assert!(fs.exists(DIR_INSIGHTS, filename).unwrap());
        let content = fs.read(DIR_INSIGHTS, filename).unwrap();
        // No habits, no content (but file created)
        assert_eq!(content, "");
    }

    #[test]
    fn test_write_habits_with_data() {
        let (fs, _t) = test_fs();

        let mut habits: Habits = HashMap::new();
        let mut year_map: YearHabits = HashMap::new();
        // January 1 = day 1, mark as completed
        year_map.insert(1, 1);
        habits.insert("Exercise".to_string(), year_map);

        write_habits(&fs, 2024, &habits).unwrap();

        let content = fs.read(DIR_INSIGHTS, "2024 Habits.md").unwrap();
        assert!(content.contains("### January"));
        assert!(content.contains("Exercise"));
        // Should contain HABIT_COMPLETED for completed day
        assert!(content.contains(HABIT_COMPLETED));
    }

    #[test]
    fn test_write_habits_roundtrip() {
        let (fs, _t) = test_fs();

        // Create habit files
        fs.make_dir(DIR_HABITS).unwrap();
        fs.write(DIR_HABITS, "Exercise.md", "\u{1F3CB}").unwrap();

        // Write habits data
        let mut habits_data: Habits = HashMap::new();
        let mut ym: YearHabits = HashMap::new();
        ym.insert(1, 1);
        habits_data.insert("Exercise".to_string(), ym);

        write_habits(&fs, 2024, &habits_data).unwrap();

        // Read back using habits()
        let read_back = habits(&fs, 2024).unwrap();
        assert_eq!(read_back.get("Exercise").unwrap().get(&1), Some(&1));
    }

    #[test]
    fn test_month_name() {
        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(6), "June");
        assert_eq!(month_name(12), "December");
    }
}
