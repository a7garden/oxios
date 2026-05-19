//! Schedule management for tasks.
//!
//! Ported from files.md (`server/schedule/mod.rs`) by Artem Zakirullin.
//! Manages scheduled tasks stored in the knowledge config.

use chrono::{Datelike, TimeZone, Utc, FixedOffset};

use crate::fs::VirtualFs;
use crate::types::{KnowledgeConfig, Schedule, DIR_USER_ROOT};

/// Schedule-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum ScheduleError {
    /// Config read failure.
    #[error("config read: {0}")]
    Read(String),
    /// Config write failure.
    #[error("config write: {0}")]
    Write(String),
}

/// Manages scheduled tasks for a knowledge base.
pub struct ScheduleManager<'a> {
    fs: &'a VirtualFs,
    config_filename: &'a str,
}

impl<'a> ScheduleManager<'a> {
    /// Create a new schedule manager.
    pub fn new(fs: &'a VirtualFs, config_filename: &'a str) -> Self {
        Self { fs, config_filename }
    }

    /// Get all schedules.
    pub fn schedules(&self) -> Result<Vec<Schedule>, ScheduleError> {
        let cfg = self.read_config()?;
        Ok(cfg.schedules)
    }

    /// Add or update a schedule for a filename.
    pub fn add(&self, filename: &str, scheduled_at: i64, cron: &str) -> Result<(), ScheduleError> {
        let mut cfg = self.read_config()?;
        if let Some(s) = cfg.schedules.iter_mut().find(|s| s.filename == filename) {
            s.scheduled_at = scheduled_at;
            s.cron = cron.to_string();
        } else {
            cfg.schedules.push(Schedule {
                filename: filename.to_string(),
                scheduled_at,
                cron: cron.to_string(),
                cmd: String::new(),
            });
        }
        self.write_config(&cfg)
    }

    /// Delete a schedule by filename.
    pub fn delete(&self, filename: &str) -> Result<(), ScheduleError> {
        let mut cfg = self.read_config()?;
        cfg.schedules.retain(|s| s.filename != filename);
        self.write_config(&cfg)
    }

    fn read_config(&self) -> Result<KnowledgeConfig, ScheduleError> {
        if !self.fs.exists(DIR_USER_ROOT, self.config_filename).map_err(|e| ScheduleError::Read(e.to_string()))? {
            return Ok(KnowledgeConfig::default());
        }
        let content = self.fs.read(DIR_USER_ROOT, self.config_filename)
            .map_err(|e| ScheduleError::Read(e.to_string()))?;
        serde_json::from_str(&content).map_err(|e| ScheduleError::Read(e.to_string()))
    }

    fn write_config(&self, cfg: &KnowledgeConfig) -> Result<(), ScheduleError> {
        let json = serde_json::to_string_pretty(cfg).map_err(|e| ScheduleError::Write(e.to_string()))?;
        self.fs.write(DIR_USER_ROOT, self.config_filename, &json)
            .map_err(|e| ScheduleError::Write(e.to_string()))
    }
}

/// Format a schedule date for display.
pub fn format_schedule_date(scheduled_at: i64, timezone: FixedOffset) -> String {
    let now = Utc::now().timestamp();
    let today_start = beginning_of_day(now);
    let task_start = beginning_of_day(scheduled_at);
    let diff_days = (task_start - today_start) / 86400;

    let tz_dt = Utc.timestamp_opt(scheduled_at, 0).unwrap().with_timezone(&timezone);

    match diff_days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        2..=6 => format!("{} {:02}", tz_dt.format("%A"), tz_dt.day()),
        7..=13 => format!("Next {}", tz_dt.format("%A %d")),
        _ => format!("{} {}, {}", tz_dt.format("%d %B"), tz_dt.weekday(), tz_dt.year()),
    }
}

/// Calculate the beginning of a day (midnight) as a Unix timestamp.
pub fn beginning_of_day(timestamp: i64) -> i64 {
    let dt = Utc.timestamp_opt(timestamp, 0).unwrap();
    let date = dt.date_naive();
    date.and_hms_milli_opt(0, 0, 0, 0).unwrap().and_utc().timestamp()
}

/// Calculate tomorrow's midnight timestamp.
pub fn tomorrow_timestamp() -> i64 {
    let tomorrow = Utc::now().date_naive() + chrono::Duration::days(1);
    tomorrow.and_hms_milli_opt(0, 0, 0, 0).unwrap().and_utc().timestamp()
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

    #[test]
    fn test_add_and_list() {
        let (fs, _t) = test_fs();
        let mgr = ScheduleManager::new(&fs, "config.json");
        mgr.add("Task.md", 1000000, "").unwrap();
        mgr.add("Other.md", 2000000, "9:00").unwrap();
        let schedules = mgr.schedules().unwrap();
        assert_eq!(schedules.len(), 2);
    }

    #[test]
    fn test_update_existing() {
        let (fs, _t) = test_fs();
        let mgr = ScheduleManager::new(&fs, "config.json");
        mgr.add("Task.md", 1000000, "").unwrap();
        mgr.add("Task.md", 2000000, "10:00").unwrap();
        let schedules = mgr.schedules().unwrap();
        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].scheduled_at, 2000000);
    }

    #[test]
    fn test_delete() {
        let (fs, _t) = test_fs();
        let mgr = ScheduleManager::new(&fs, "config.json");
        mgr.add("Task.md", 1000000, "").unwrap();
        mgr.delete("Task.md").unwrap();
        assert!(mgr.schedules().unwrap().is_empty());
    }

    #[test]
    fn test_format_date() {
        let tz = FixedOffset::east_opt(0).unwrap();
        let ts = Utc::now().timestamp() + 86400;
        let formatted = format_schedule_date(ts, tz);
        assert_eq!(formatted, "Tomorrow");
    }

    #[test]
    fn test_tomorrow() {
        assert!(tomorrow_timestamp() > Utc::now().timestamp());
    }
}
