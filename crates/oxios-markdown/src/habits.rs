//! Habit tracking.
//!
//! Ported from files.md (`server/habits/mod.rs`) by Artem Zakirullin.
//! Reads/writes habit data from the insights directory.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::Datelike;
use unicode_segmentation::UnicodeSegmentation;

use crate::fs::VirtualFs;
use crate::parser::norm_new_lines;
use crate::types::{
    FsError, Habits,
    HABIT_COMPLETED, HABIT_COMPLETED_AT_WEEKEND, HABIT_SKIPPED,
    MOOD_EMOJIS, MOOD_HABIT,
    DIR_HABITS, DIR_INSIGHTS, MD_EXT,
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
    fn from(e: FsError) -> Self { HabitsError::Other(e.to_string()) }
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
        if line.is_empty() { continue; }

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
        if parts.len() < 2 { continue; }

        let days = parts[0];
        let habit = parts[1];
        let first_day = chrono::NaiveDate::from_ymd_opt(year, month as u32, 1).unwrap();
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

        let marker = format!("{}{}{}", HABIT_SKIPPED, HABIT_COMPLETED_AT_WEEKEND, HABIT_COMPLETED);
        if !days.contains(marker.chars().next().unwrap().to_string().as_str()) { continue; }

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
pub fn emoji_for_status(habit_name: &str, day: &chrono::DateTime<chrono::FixedOffset>, status: i32) -> &'static str {
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
        if !trimmed.is_empty() { return trimmed.to_string(); }
    }
    weekday_emoji(habit_name).to_string()
}

/// Get emoji for a weekday or month name.
pub fn weekday_emoji(key: &str) -> &'static str {
    match key.to_lowercase().as_str() {
        "monday" => "🌑", "tuesday" => "🌒", "wednesday" => "🌓",
        "thursday" => "🌔", "friday" => "🌕", "saturday" => "🌝",
        "sunday" => "🌛", _ => "⚡️",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono::FixedOffset;

    #[test]
    fn test_emoji_for_status() {
        let saturday = FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2024, 1, 6, 12, 0, 0).unwrap();
        assert_eq!(emoji_for_status("Exercise", &saturday, 1), HABIT_COMPLETED_AT_WEEKEND);
        assert_eq!(emoji_for_status("Exercise", &saturday, 0), HABIT_SKIPPED);
    }

    #[test]
    fn test_mood_emoji() {
        let day = FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(emoji_for_status(MOOD_HABIT, &day, 0), "⚪️");
        assert_eq!(emoji_for_status(MOOD_HABIT, &day, 5), "😊");
    }

    #[test]
    fn test_weekday_emoji() {
        assert_eq!(weekday_emoji("monday"), "🌑");
        assert_eq!(weekday_emoji("unknown"), "⚡️");
    }
}
