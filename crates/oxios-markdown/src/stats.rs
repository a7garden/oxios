//! Stats: today's completion report.
//!
//! Ported from files.md (`server/stats/stats.go`) by Artem Zakirullin.

use crate::fs::{VirtualFs, display_name, is_checklist_item};
use crate::types::{DIR_ARCHIVE, FileEntry, FsError};

/// A completed item shown in today's report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompletedItem {
    /// The display name (capitalized, no extension).
    pub display_name: String,
    /// Whether this is a checklist item.
    pub is_checklist: bool,
}

/// Today's completion report.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TodayReport {
    /// Items completed today.
    pub completed_items: Vec<CompletedItem>,
    /// Total number of archived files (all time).
    pub total_done: usize,
}

/// Get list of files done today (ctime > midnight UTC).
pub fn done_today(fs: &VirtualFs) -> Result<Vec<FileEntry>, FsError> {
    let all = fs.files_and_dirs(DIR_ARCHIVE)?;
    let midnight = beginning_of_day_utc();
    Ok(all
        .into_iter()
        .filter(|f| !f.is_dir && f.ctime > midnight)
        .collect())
}

/// Get today's completion report: files done today + total archived count.
pub fn today_report(fs: &VirtualFs) -> Result<TodayReport, FsError> {
    let today_files = done_today(fs)?;
    let all_archived = fs.files_and_dirs(DIR_ARCHIVE)?;
    let total_done = all_archived.iter().filter(|f| !f.is_dir).count();

    let completed_items: Vec<CompletedItem> = today_files
        .iter()
        .map(|f| {
            let is_checklist = is_checklist_item(&f.name);
            CompletedItem {
                display_name: display_name(&f.name),
                is_checklist,
            }
        })
        .collect();

    Ok(TodayReport {
        completed_items,
        total_done,
    })
}

/// Format today's report as a string (matching Go output format).
pub fn format_today_report(report: &TodayReport) -> String {
    let mut lines: Vec<String> = Vec::new();
    for item in &report.completed_items {
        let emoji = if item.is_checklist { "☑️" } else { "✅" };
        lines.push(format!("{} <b>{}</b>", emoji, item.display_name));
    }
    lines.push(format!("\n📊 {} tasks done in total", report.total_done));
    lines.join("\n")
}

/// Returns the Unix timestamp (milliseconds) of midnight UTC today.
fn beginning_of_day_utc() -> i64 {
    use chrono::Utc;
    let now = Utc::now();
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let dt = midnight.and_utc();
    dt.timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (VirtualFs, TempDir) {
        let dir = TempDir::new().unwrap();
        let fs = VirtualFs::new(dir.path().to_path_buf()).unwrap();
        (fs, dir)
    }

    #[test]
    fn test_done_today_empty() {
        let (fs, _t) = setup();
        let result = done_today(&fs).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_today_report_empty() {
        let (fs, _t) = setup();
        let report = today_report(&fs).unwrap();
        assert!(report.completed_items.is_empty());
        assert_eq!(report.total_done, 0);
    }

    #[test]
    fn test_today_report_with_files() {
        let (fs, _t) = setup();
        fs.write(DIR_ARCHIVE, "MyTask.md", "content").unwrap();
        let report = today_report(&fs).unwrap();
        assert_eq!(report.completed_items.len(), 1);
        assert_eq!(report.total_done, 1);
        assert_eq!(report.completed_items[0].display_name, "MyTask");
        assert!(!report.completed_items[0].is_checklist);
    }

    #[test]
    fn test_format_today_report() {
        let report = TodayReport {
            completed_items: vec![CompletedItem {
                display_name: "Rust".into(),
                is_checklist: false,
            }],
            total_done: 5,
        };
        let formatted = format_today_report(&report);
        assert!(formatted.contains("✅"));
        assert!(formatted.contains("<b>Rust</b>"));
        assert!(formatted.contains("📊 5 tasks done in total"));
    }
}
