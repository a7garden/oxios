//! Nightly worker — daily cleanup of completed items.
//!
//! Ported from files.md (`server/worker.go`) by Artem Zakirullin.
//! Handles removal of completed checklist items from Chat.md and Later.md,
//! archiving them to Done.md, and adding journal entries.
//!
//! Bot/Telegram dependencies are removed — this module contains pure functions.

use chrono::{Datelike, FixedOffset, Utc};
use regex::Regex;

use crate::chat::read_chat_msgs;
use crate::fs::VirtualFs;
use crate::journal::{add_record as journal_add_record, today_header as journal_today_header};
use crate::parser::norm_new_lines;
use crate::types::{
    FsError, KnowledgeConfig,
    DIR_ARCHIVE, DIR_USER_ROOT,
    CHAT_FILENAME, LATER_FILENAME, DONE_FILENAME,
};

/// Result of a nightly cleanup run.
#[derive(Debug, Default)]
pub struct NightlyReport {
    /// Number of items archived to Done.md.
    pub archived_count: usize,
    /// Number of journal entries added.
    pub journal_count: usize,
}

/// Remove completed checklist items from Chat.md and Later.md,
/// archive them to Done.md, and add journal entries.
///
/// This is a pure function that operates on the virtual filesystem.
/// The `timezone` parameter controls how timestamps are formatted.
pub fn remove_completed_items(
    fs: &VirtualFs,
    config: &KnowledgeConfig,
) -> Result<NightlyReport, FsError> {
    let tz = parse_timezone(&config.timezone);
    let mut report = NightlyReport::default();

    // Targets: (filename, reducer function)
    // We apply two reducers to Chat.md:
    //   1. checklist removal (from both Chat and Later)
    //   2. inbox entry removal (Chat only)
    let targets: &[(&str, fn(&str) -> (String, String))] = &[
        (CHAT_FILENAME, remove_completed_checklist),
        (LATER_FILENAME, remove_completed_checklist),
        (CHAT_FILENAME, remove_completed_inbox_entries),
    ];

    for &(filename, reducer) in targets {
        let md = match fs.read(DIR_USER_ROOT, filename) {
            Ok(content) => content,
            Err(FsError::Io(_)) => continue, // file doesn't exist
            Err(e) => return Err(e),
        };

        let (reduced_md, removed_md) = reducer(&md);
        if removed_md.is_empty() {
            continue;
        }

        fs.write(DIR_USER_ROOT, filename, &reduced_md)?;

        // Archive removed items to Done.md
        let done_md = match fs.read(DIR_ARCHIVE, DONE_FILENAME) {
            Ok(content) => content,
            Err(FsError::Io(_)) => String::new(),
            Err(e) => return Err(e),
        };

        let now_tz = Utc::now().with_timezone(&tz);
        let header = format!(
            "#### {} {}, {}",
            now_tz.day(),
            now_tz.format("%B"),
            now_tz.format("%A")
        );

        let updated_done = add_header_and_text(&done_md, &header, &removed_md);
        fs.write(DIR_ARCHIVE, DONE_FILENAME, &updated_done)?;

        // Add journal entries for each completed task
        let tasks = checklist_items(&removed_md);
        for task in &tasks {
            let stripped = strip_chat_timestamp(task);
            let _ = journal_add_record(
                fs,
                &format!("✅ {}", stripped),
                tz,
            );
            report.journal_count += 1;
        }
        report.archived_count += tasks.len();
    }

    Ok(report)
}

/// Remove completed checklist items (`- [x]` / `- [X]`) from markdown.
///
/// Returns `(kept_content, removed_lines)`.
pub fn remove_completed_checklist(md: &str) -> (String, String) {
    let mut kept = Vec::new();
    let mut removed = String::new();

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
            removed.push_str(trimmed);
            removed.push('\n');
        } else {
            kept.push(line);
        }
    }

    (kept.join("\n"), removed)
}

/// Remove completed inbox entries (chat blocks starting with `- [x]`).
///
/// Returns `(surviving_content, removed_markdown)`. Removed items are
/// formatted as `- [x] <item>` for archival in Done.md.
pub fn remove_completed_inbox_entries(md: &str) -> (String, String) {
    let blocks = read_chat_msgs(md);

    let done_re = Regex::new(r"^- \[[xX]\] ").unwrap();
    let ts_re = Regex::new(
        &format!(
            r"^(?:- \[[ xX]\] )?{}`{}\d{{2}}:\d{{2}}`{}` ` ",
            "", "", ""
        ),
    )
    .unwrap();
    // Build the regex properly: matches optional checkbox + backtick timestamp
    let ts_re = Regex::new(r"^(?:- \[[ xX]\] )?`\d{2}:\d{2}` ").unwrap();

    let mut kept: Vec<String> = Vec::new();
    let mut removed = String::new();

    for block in blocks {
        let first_line = if let Some(nl) = block.find('\n') {
            &block[..nl]
        } else {
            &block
        };

        if !done_re.is_match(first_line) {
            kept.push(block);
            continue;
        }

        // Strip the optional checkbox + timestamp prefix
        let body = ts_re.replace_all(&block, "");
        // Flatten continuation lines into spaces
        let body = body.replace('\n', " ");
        removed.push_str("- [x] ");
        removed.push_str(&body);
        removed.push('\n');
    }

    let new_md = kept.join("\n").trim().to_string();
    (new_md, removed)
}

/// Move due scheduled tasks to Chat.md.
///
/// Finds schedules where `scheduled_at <= now()`, appends the task content
/// to Chat.md, and either reschedules (if cron is set) or removes the schedule.
///
/// Returns list of moved task filenames.
pub fn move_due_tasks(
    fs: &VirtualFs,
    config: &mut KnowledgeConfig,
) -> Result<Vec<String>, FsError> {
    let now_ts = Utc::now().timestamp();
    let mut moved = Vec::new();

    // Collect indices of due tasks (iterate in reverse to allow safe removal)
    let due_indices: Vec<usize> = config
        .schedules
        .iter()
        .enumerate()
        .filter(|(_, s)| s.scheduled_at <= now_ts)
        .map(|(i, _)| i)
        .collect();

    // Process in reverse order so index removal doesn't shift
    for idx in due_indices.into_iter().rev() {
        let schedule = &config.schedules[idx];
        let filename = schedule.filename.clone();
        let cron = schedule.cron.clone();

        // Try to append the task to Chat.md
        if let Ok(task_content) = fs.read(DIR_USER_ROOT, &filename) {
            append_to_chat(fs, &task_content)?;
        }

        moved.push(filename.clone());

        if !cron.is_empty() {
            // Reschedule: calculate next execution time
            if let Some(next_ts) = next_exclude_today(&cron) {
                // Update the schedule
                if let Some(s) = config.schedules.get_mut(idx) {
                    s.scheduled_at = next_ts;
                }
            }
        } else {
            // One-time task: remove from schedule
            config.schedules.remove(idx);
        }
    }

    Ok(moved)
}

/// Generate a schedule report for display.
///
/// Takes a list of (display_name, scheduled_at) pairs and returns
/// a formatted string grouped by day.
pub fn schedule_report(schedules: &[(String, i64)]) -> String {
    let mut day_order: Vec<String> = Vec::new();
    let mut day_tasks: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let now_ts = Utc::now().timestamp();

    for (display_name, scheduled_at) in schedules {
        let day = format_schedule_day(*scheduled_at, now_ts);
        if !day_tasks.contains_key(&day) {
            day_order.push(day.clone());
        }
        day_tasks.entry(day).or_default().push(display_name.clone());
    }

    let mut report = String::new();
    for day in &day_order {
        report.push_str(&format!("{}\n", day));
        if let Some(tasks) = day_tasks.get(day) {
            for task in tasks {
                report.push_str(&format!("- {}\n", task));
            }
        }
        report.push('\n');
    }

    report.trim().to_string()
}

/// Calculate next cron execution time, excluding today.
///
/// Supports simple "HH:MM" format (e.g., "9:00", "14:30").
/// Returns the Unix timestamp of the next occurrence.
pub fn next_exclude_today(cron_expr: &str) -> Option<i64> {
    // Parse simple HH:MM format
    let parts: Vec<&str> = cron_expr.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;

    if hour > 23 || minute > 59 {
        return None;
    }

    // Calculate tomorrow at HH:MM UTC
    let now = Utc::now();
    let tomorrow = now.date_naive() + chrono::Duration::days(1);
    let target = tomorrow
        .and_hms_opt(hour, minute, 0)?
        .and_utc()
        .timestamp();

    Some(target)
}

// ── Internal helpers ─────────────────────────────────────────

/// Parse a timezone string (e.g., "+09:00", "UTC") into a FixedOffset.
fn parse_timezone(tz_str: &str) -> FixedOffset {
    if tz_str == "UTC" || tz_str.is_empty() {
        return FixedOffset::east_opt(0).unwrap();
    }
    // Try parsing "+HH:MM" or "-HH:MM"
    if let Ok(offset) = tz_str.parse::<FixedOffset>() {
        return offset;
    }
    FixedOffset::east_opt(0).unwrap()
}

/// Extract checklist items from markdown text.
/// Returns the text of each `- [x]` or `- [ ]` item (trimmed).
fn checklist_items(md: &str) -> Vec<String> {
    let re = Regex::new(r"^- \[[ xX]\] (.+)$").unwrap();
    let mut items = Vec::new();
    for line in md.lines() {
        let trimmed = line.trim();
        if let Some(caps) = re.captures(trimmed) {
            if let Some(m) = caps.get(1) {
                items.push(m.as_str().to_string());
            }
        }
    }
    items
}

/// Strip a leading `` `HH:MM` `` timestamp from a chat entry.
fn strip_chat_timestamp(s: &str) -> String {
    let re = Regex::new(r"^`\d{2}:\d{2}` ").unwrap();
    re.replace(s, "").to_string()
}

/// Add a header and text block to existing markdown content.
fn add_header_and_text(existing: &str, header: &str, text: &str) -> String {
    let mut result = existing.trim().to_string();
    if !result.is_empty() {
        result.push('\n');
    }
    result.push_str(header);
    result.push('\n');
    result.push_str(text.trim());
    result
}

/// Append content to Chat.md with a timestamp header.
fn append_to_chat(fs: &VirtualFs, content: &str) -> Result<(), FsError> {
    let existing = match fs.read(DIR_USER_ROOT, CHAT_FILENAME) {
        Ok(c) => c,
        Err(FsError::Io(_)) => String::new(),
        Err(e) => return Err(e),
    };

    let normalized = norm_new_lines(&existing);
    let mut new_content = normalized.trim().to_string();
    if !new_content.is_empty() {
        new_content.push('\n');
    }

    let now = Utc::now();
    let header = format!(
        "#### {} {}, {}",
        now.date_naive().day(),
        now.format("%B"),
        now.format("%A")
    );

    new_content.push_str(&header);
    new_content.push('\n');
    new_content.push_str(content.trim());
    new_content.push('\n');

    fs.write(DIR_USER_ROOT, CHAT_FILENAME, &new_content)
}

/// Format a scheduled timestamp as a human-readable day label.
fn format_schedule_day(scheduled_at: i64, now_ts: i64) -> String {
    let today_start = beginning_of_day(now_ts);
    let task_start = beginning_of_day(scheduled_at);
    let diff_days = (task_start - today_start) / 86400;

    let dt = Utc.timestamp_opt(scheduled_at, 0).unwrap();

    match diff_days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        2..=6 => format!("{} {:02}", dt.format("%A"), dt.day()),
        7..=13 => format!("Next {}", dt.format("%A %d")),
        _ => format!("{} {}, {}", dt.format("%d %B"), dt.weekday(), dt.year()),
    }
}

/// Calculate the beginning of a day (midnight) as a Unix timestamp.
fn beginning_of_day(timestamp: i64) -> i64 {
    let dt = Utc.timestamp_opt(timestamp, 0).unwrap();
    let date = dt.date_naive();
    date.and_hms_milli_opt(0, 0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_fs() -> (VirtualFs, TempDir) {
        let dir = TempDir::new().unwrap();
        let fs = VirtualFs::new(dir.path().to_path_buf()).unwrap();
        (fs, dir)
    }

    // ── remove_completed_checklist ──────────────────────────

    #[test]
    fn test_remove_completed_checklist() {
        let md = "- [ ] Pending\n- [x] Done\n- [X] Also done\n- [ ] Keep";
        let (kept, removed) = remove_completed_checklist(md);
        assert!(kept.contains("Pending"));
        assert!(kept.contains("Keep"));
        assert!(!kept.contains("Done"));
        assert!(removed.contains("- [x] Done"));
        assert!(removed.contains("- [X] Also done"));
    }

    #[test]
    fn test_remove_completed_checklist_no_completed() {
        let md = "- [ ] Pending\n- [ ] Another";
        let (kept, removed) = remove_completed_checklist(md);
        assert_eq!(removed, "");
        assert!(kept.contains("Pending"));
    }

    #[test]
    fn test_remove_completed_checklist_empty() {
        let md = "";
        let (kept, removed) = remove_completed_checklist(md);
        assert_eq!(kept, "");
        assert_eq!(removed, "");
    }

    // ── remove_completed_inbox_entries ──────────────────────

    #[test]
    fn test_remove_completed_inbox_entries() {
        let md = "#### 19 May\n- [x] `09:00` Completed task\n- [ ] `10:00` Pending task";
        let (kept, removed) = remove_completed_inbox_entries(md);
        assert!(kept.contains("Pending"));
        assert!(!kept.contains("Completed"));
        assert!(removed.contains("- [x] Completed task"));
    }

    #[test]
    fn test_remove_completed_inbox_entries_multiline_block() {
        let md = "- [x] `09:00` Multi\nline task\n- [ ] Keep this";
        let (kept, removed) = remove_completed_inbox_entries(md);
        assert!(kept.contains("Keep"));
        assert!(removed.contains("- [x] Multi line task"));
    }

    #[test]
    fn test_remove_completed_inbox_entries_no_completed() {
        let md = "#### 19 May\n- [ ] `09:00` Pending\n- [ ] `10:00` Also pending";
        let (kept, removed) = remove_completed_inbox_entries(md);
        assert!(removed.is_empty());
    }

    // ── remove_completed_items ──────────────────────────────

    #[test]
    fn test_remove_completed_items_basic() {
        let (fs, _t) = test_fs();
        fs.create_system_dirs().unwrap();

        // Write Chat.md with completed items
        fs.write(DIR_USER_ROOT, CHAT_FILENAME, "- [x] Completed task\n- [ ] Pending task")
            .unwrap();

        let config = KnowledgeConfig::default();
        let report = remove_completed_items(&fs, &config).unwrap();
        assert_eq!(report.archived_count, 2); // checklist + inbox both process chat

        // Chat.md should only contain the pending task
        let chat = fs.read(DIR_USER_ROOT, CHAT_FILENAME).unwrap();
        assert!(chat.contains("Pending"));
        assert!(!chat.contains("Completed task"));

        // Done.md should contain the completed task
        let done = fs.read(DIR_ARCHIVE, DONE_FILENAME).unwrap();
        assert!(done.contains("Completed task"));
    }

    #[test]
    fn test_remove_completed_items_both_files() {
        let (fs, _t) = test_fs();
        fs.create_system_dirs().unwrap();

        fs.write(DIR_USER_ROOT, CHAT_FILENAME, "- [x] Chat done\n- [ ] Chat pending")
            .unwrap();
        fs.write(DIR_USER_ROOT, LATER_FILENAME, "- [x] Later done\n- [ ] Later pending")
            .unwrap();

        let config = KnowledgeConfig::default();
        let report = remove_completed_items(&fs, &config).unwrap();
        assert!(report.archived_count >= 2);

        // Later.md should only contain pending
        let later = fs.read(DIR_USER_ROOT, LATER_FILENAME).unwrap();
        assert!(later.contains("Later pending"));
        assert!(!later.contains("Later done"));
    }

    // ── next_exclude_today ──────────────────────────────────

    #[test]
    fn test_next_exclude_today_valid() {
        let result = next_exclude_today("9:00");
        assert!(result.is_some());
        let ts = result.unwrap();
        // Should be in the future
        assert!(ts > Utc::now().timestamp());
    }

    #[test]
    fn test_next_exclude_today_invalid() {
        assert!(next_exclude_today("invalid").is_none());
        assert!(next_exclude_today("25:00").is_none());
        assert!(next_exclude_today("9:60").is_none());
        assert!(next_exclude_today("").is_none());
    }

    #[test]
    fn test_next_exclude_today_format() {
        let result = next_exclude_today("14:30");
        assert!(result.is_some());
        let ts = result.unwrap();
        let dt = Utc.timestamp_opt(ts, 0).unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
    }

    // ── schedule_report ─────────────────────────────────────

    #[test]
    fn test_schedule_report() {
        let now_ts = Utc::now().timestamp();
        let schedules = vec![
            ("Task A".to_string(), now_ts),
            ("Task B".to_string(), now_ts + 86400),
        ];
        let report = schedule_report(&schedules);
        assert!(report.contains("Today"));
        assert!(report.contains("Tomorrow"));
        assert!(report.contains("Task A"));
        assert!(report.contains("Task B"));
    }

    // ── move_due_tasks ──────────────────────────────────────

    #[test]
    fn test_move_due_tasks_past_schedule() {
        let (fs, _t) = test_fs();

        let past_ts = Utc::now().timestamp() - 3600; // 1 hour ago
        let mut config = KnowledgeConfig::default();
        config.schedules.push(crate::types::Schedule {
            filename: "Task.md".to_string(),
            scheduled_at: past_ts,
            cron: String::new(),
            cmd: String::new(),
        });

        let moved = move_due_tasks(&fs, &mut config).unwrap();
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0], "Task.md");
        // Schedule should be removed (no cron)
        assert!(config.schedules.is_empty());
    }

    #[test]
    fn test_move_due_tasks_future_schedule() {
        let (fs, _t) = test_fs();

        let future_ts = Utc::now().timestamp() + 86400; // tomorrow
        let mut config = KnowledgeConfig::default();
        config.schedules.push(crate::types::Schedule {
            filename: "Task.md".to_string(),
            scheduled_at: future_ts,
            cron: String::new(),
            cmd: String::new(),
        });

        let moved = move_due_tasks(&fs, &mut config).unwrap();
        assert!(moved.is_empty());
        assert_eq!(config.schedules.len(), 1);
    }

    #[test]
    fn test_move_due_tasks_cron_reschedules() {
        let (fs, _t) = test_fs();

        let past_ts = Utc::now().timestamp() - 3600;
        let mut config = KnowledgeConfig::default();
        config.schedules.push(crate::types::Schedule {
            filename: "Recurring.md".to_string(),
            scheduled_at: past_ts,
            cron: "9:00".to_string(),
            cmd: String::new(),
        });

        let moved = move_due_tasks(&fs, &mut config).unwrap();
        assert_eq!(moved.len(), 1);
        // Should still have the schedule (rescheduled)
        assert_eq!(config.schedules.len(), 1);
        assert!(config.schedules[0].scheduled_at > Utc::now().timestamp());
    }

    // ── add_header_and_text ─────────────────────────────────

    #[test]
    fn test_add_header_and_text() {
        let result = add_header_and_text("existing", "#### Header", "some text");
        assert!(result.contains("existing"));
        assert!(result.contains("#### Header"));
        assert!(result.contains("some text"));
    }

    #[test]
    fn test_add_header_and_text_empty_existing() {
        let result = add_header_and_text("", "#### Header", "some text");
        assert!(result.starts_with("#### Header"));
    }
}
