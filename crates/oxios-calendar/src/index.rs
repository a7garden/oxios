//! Calendar index for fast lookup without re-parsing .ics files.
//!
//! The index is stored as `index.json` in the calendar directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::IndexEntry;

/// Index file name.
const INDEX_FILE: &str = "index.json";

/// In-memory index mapping event UIDs to their metadata.
#[derive(Debug)]
pub struct CalendarIndex {
    /// UID → index entry.
    entries: HashMap<String, IndexEntry>,
    /// Path to the index.json file.
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexFile {
    /// Map of UID → entry.
    entries: HashMap<String, IndexEntry>,
}

impl CalendarIndex {
    /// Load the index from disk, or create an empty one if the file doesn't exist.
    ///
    /// The `dir` argument is the calendar directory (not the index file itself).
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(INDEX_FILE);
        let entries = if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            let file: IndexFile = serde_json::from_str(&data)?;
            file.entries
        } else {
            HashMap::new()
        };
        Ok(Self { entries, path })
    }

    /// Persist the index to disk atomically (F17).
    ///
    /// Writes to a sibling temporary file then renames it over the target,
    /// so a crash mid-write can never leave a partially-written `index.json`
    /// — readers always see either the previous or the new complete file.
    pub fn save(&self) -> anyhow::Result<()> {
        let file = IndexFile {
            entries: self.entries.clone(),
        };
        let data = serde_json::to_string_pretty(&file)?;
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, &data)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Insert or replace an entry.
    pub fn insert(&mut self, uid: String, entry: IndexEntry) {
        self.entries.insert(uid, entry);
    }

    /// Remove an entry by UID.
    pub fn remove(&mut self, uid: &str) -> Option<IndexEntry> {
        self.entries.remove(uid)
    }

    /// Look up a single entry by UID.
    pub fn get(&self, uid: &str) -> Option<&IndexEntry> {
        self.entries.get(uid)
    }

    /// Return all entries whose start time falls within `[from, to)`.
    ///
    /// For recurring events, only the base start time is checked; expansion
    /// is handled separately by the engine.
    pub fn list_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<&IndexEntry> {
        self.entries
            .values()
            .filter(|e| e.dtstart >= from && e.dtstart < to)
            .collect()
    }

    /// Search entries by summary text (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&IndexEntry> {
        let lower = query.to_lowercase();
        self.entries
            .values()
            .filter(|e| e.summary.to_lowercase().contains(&lower))
            .collect()
    }

    /// Return all entries in the index.
    pub fn all_entries(&self) -> Vec<&IndexEntry> {
        self.entries.values().collect()
    }

    /// Number of entries in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventSource;
    use chrono::TimeZone;

    fn make_entry(uid: &str, summary: &str, start: DateTime<Utc>) -> IndexEntry {
        IndexEntry {
            file: format!("{}.ics", uid),
            summary: summary.to_string(),
            dtstart: start,
            dtend: start + chrono::Duration::hours(1),
            rrule: None,
            status: "CONFIRMED".to_string(),
            source: EventSource::User,
        }
    }

    #[test]
    fn insert_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let mut idx = CalendarIndex::load(dir.path()).unwrap();
        let uid = "test-1";
        let start = Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap();
        idx.insert(uid.to_string(), make_entry(uid, "Test Event", start));
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.get(uid).unwrap().summary, "Test Event");
    }

    #[test]
    fn list_range_filters() {
        let dir = tempfile::tempdir().unwrap();
        let mut idx = CalendarIndex::load(dir.path()).unwrap();

        let t1 = Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 6, 15, 9, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2026, 6, 20, 9, 0, 0).unwrap();

        idx.insert("e1".to_string(), make_entry("e1", "Event 1", t1));
        idx.insert("e2".to_string(), make_entry("e2", "Event 2", t2));
        idx.insert("e3".to_string(), make_entry("e3", "Event 3", t3));

        let from = Utc.with_ymd_and_hms(2026, 6, 12, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 6, 18, 0, 0, 0).unwrap();
        let results = idx.list_range(from, to);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].summary, "Event 2");
    }

    #[test]
    fn search_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let mut idx = CalendarIndex::load(dir.path()).unwrap();

        let t = Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap();
        idx.insert("s1".to_string(), make_entry("s1", "Weekly Standup", t));
        idx.insert("s2".to_string(), make_entry("s2", "Sprint Review", t));

        let results = idx.search("standup");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].summary, "Weekly Standup");
    }

    #[test]
    fn save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let uid = "persist-test";
        let t = Utc.with_ymd_and_hms(2026, 6, 10, 9, 0, 0).unwrap();

        {
            let mut idx = CalendarIndex::load(dir.path()).unwrap();
            idx.insert(uid.to_string(), make_entry(uid, "Persisted Event", t));
            idx.save().unwrap();
        }

        let idx2 = CalendarIndex::load(dir.path()).unwrap();
        assert_eq!(idx2.len(), 1);
        assert_eq!(idx2.get(uid).unwrap().summary, "Persisted Event");
    }
}
