//! Journal entry management.
//!
//! Ported from files.md (`server/journal/mod.rs`) by Artem Zakirullin.
//! Manages daily journal files with timestamped records.

use chrono::{Datelike, FixedOffset, Utc};
use regex::Regex;

use crate::fs::VirtualFs;
use crate::parser::{has_image, norm_new_lines};
use crate::types::{FsError, DIR_JOURNAL};

/// Image markdown pattern.
const IMG_PATTERN: &str = r"!\[.*?\]\(.*?\)";

/// Add a text record to today's journal entry.
pub fn add_record(fs: &VirtualFs, record: &str, timezone: FixedOffset) -> Result<(), FsError> {
    let record = record.trim();
    if record.is_empty() { return Ok(()); }

    let filename = today_journal_filename(timezone);
    let exists = fs.exists(DIR_JOURNAL, &filename)?;

    let mut md = if exists {
        let content = fs.read(DIR_JOURNAL, &filename)?;
        norm_new_lines(&content).trim().to_string()
    } else {
        String::new()
    };

    if !md.is_empty() { md.push('\n'); }
    if !md.contains(&today_header(timezone)) {
        md.push_str(&today_header(timezone));
        md.push('\n');
    }

    let timestamp = Utc::now().with_timezone(&timezone).format("`15:04`");
    if has_image(record) {
        let re = Regex::new(IMG_PATTERN).unwrap();
        let img_link = re.find(record).map(|m| m.as_str().to_string()).unwrap_or_default();
        let rest = record.replace(&img_link, "").trim().to_string();
        md.push_str(&format!("{}\n{} {}\n", img_link, timestamp, rest));
    } else {
        md.push_str(&format!("{} {}\n", timestamp, record));
    }

    fs.write(DIR_JOURNAL, &filename, &md)
}

/// Add an emoji indicator to today's journal header.
pub fn add_emoji(fs: &VirtualFs, emoji: &str, timezone: FixedOffset) -> Result<(), FsError> {
    if emoji.is_empty() { return Ok(()); }

    let filename = today_journal_filename(timezone);
    if !fs.exists(DIR_JOURNAL, &filename)? {
        let md = format!("{} {}", today_header(timezone), emoji);
        return fs.write(DIR_JOURNAL, &filename, &md);
    }

    let mut md = fs.read(DIR_JOURNAL, &filename)?;
    md = norm_new_lines(&md).trim().to_string();

    let header = today_header(timezone);
    let header_re = Regex::new(&format!("({}) *(.*)", regex::escape(&header))).unwrap();
    if header_re.is_match(&md) {
        let replacement = format!("$1 {}", emoji);
        md = header_re.replace(&md, &replacement).to_string();
    } else {
        md.push_str(&format!("\n{} {}", header, emoji));
    }

    fs.write(DIR_JOURNAL, &filename, &md)
}

/// Get today's journal filename (e.g., "2026.05 May.md").
pub fn today_journal_filename(timezone: FixedOffset) -> String {
    Utc::now().with_timezone(&timezone).format("%Y.%m %B.md").to_string()
}

/// Get today's journal header line.
pub fn today_header(timezone: FixedOffset) -> String {
    let now_tz = Utc::now().with_timezone(&timezone);
    format!("## {} {}, {}", now_tz.date_naive().day(), now_tz.format("%B"), now_tz.format("%A"))
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
    fn test_journal_filename_format() {
        let tz = FixedOffset::east_opt(0).unwrap();
        let name = today_journal_filename(tz);
        assert!(name.ends_with(".md"));
    }

    #[test]
    fn test_header_format() {
        let tz = FixedOffset::east_opt(0).unwrap();
        let header = today_header(tz);
        assert!(header.starts_with("## "));
    }

    #[test]
    fn test_add_record_creates_file() {
        let (fs, _t) = test_fs();
        let tz = FixedOffset::east_opt(0).unwrap();
        add_record(&fs, "test note", tz).unwrap();
        let filename = today_journal_filename(tz);
        assert!(fs.exists(DIR_JOURNAL, &filename).unwrap());
        let content = fs.read(DIR_JOURNAL, &filename).unwrap();
        assert!(content.contains("test note"));
    }

    #[test]
    fn test_add_emoji_creates_file() {
        let (fs, _t) = test_fs();
        let tz = FixedOffset::east_opt(0).unwrap();
        add_emoji(&fs, "🙂", tz).unwrap();
        let filename = today_journal_filename(tz);
        assert!(fs.exists(DIR_JOURNAL, &filename).unwrap());
    }
}
