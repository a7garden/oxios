//! Filesystem change log for sync.
//!
//! Ported from files.md (`server/sync/fslog.rs`) by Artem Zakirullin.
//! Tracks file renames and deletes so clients can be notified.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use parking_lot::Mutex;

const RENAME_OP: &str = "ren";
const DELETE_OP: &str = "del";

/// Filesystem change logger.
///
/// Thread-safe append-only log of file renames and deletes.
/// Used by the sync engine to notify clients of file movements.
pub struct FsLog {
    log_path: std::path::PathBuf,
    lock: Mutex<()>,
}

impl FsLog {
    /// Create a new FsLog writing to the given path.
    pub fn new(log_path: std::path::PathBuf) -> Self {
        Self {
            log_path,
            lock: Mutex::new(()),
        }
    }

    /// Record a file rename.
    pub fn log_rename(&self, time: i64, old_path: &str, new_path: &str) {
        let _guard = self.lock.lock();
        let _ = self.append(&format!(
            "{} {} {} {}",
            time,
            RENAME_OP,
            percent_encoding::utf8_percent_encode(old_path, percent_encoding::NON_ALPHANUMERIC),
            percent_encoding::utf8_percent_encode(new_path, percent_encoding::NON_ALPHANUMERIC),
        ));
    }

    /// Record a file deletion.
    pub fn log_delete(&self, time: i64, path: &str) {
        let _guard = self.lock.lock();
        let _ = self.append(&format!(
            "{} {} {}",
            time,
            DELETE_OP,
            percent_encoding::utf8_percent_encode(path, percent_encoding::NON_ALPHANUMERIC),
        ));
    }

    /// Read rename entries since a given timestamp.
    ///
    /// Returns a map of new_path → old_path.
    pub fn renames_since(
        &self,
        user_prefix: &str,
        after_timestamp: i64,
    ) -> HashMap<String, String> {
        let _guard = self.lock.lock();
        let file = match File::open(&self.log_path) {
            Ok(f) => f,
            Err(_) => return HashMap::new(),
        };

        let reader = BufReader::new(file);
        let mut result = HashMap::new();

        for line in reader.lines().map_while(Result::ok) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 4 {
                continue;
            }
            let timestamp: i64 = match parts[0].parse() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if parts[1] != RENAME_OP {
                continue;
            }
            if timestamp < after_timestamp {
                continue;
            }

            let old_path = decode_path(parts[2]);
            let new_path = decode_path(parts[3]);

            if !old_path.starts_with(user_prefix) || !new_path.starts_with(user_prefix) {
                continue;
            }

            let old_rel = old_path
                .strip_prefix(user_prefix)
                .unwrap_or(&old_path)
                .to_string();
            let new_rel = new_path
                .strip_prefix(user_prefix)
                .unwrap_or(&new_path)
                .to_string();
            result.insert(new_rel, old_rel);
        }
        result
    }

    fn append(&self, record: &str) -> std::io::Result<()> {
        if let Some(parent) = self.log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "{record}")?;
        file.sync_all()?;
        Ok(())
    }
}

fn decode_path(encoded: &str) -> String {
    percent_encoding::percent_decode_str(encoded)
        .decode_utf8_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_and_read_renames() {
        let dir = TempDir::new().unwrap();
        let log = FsLog::new(dir.path().join("fslog"));
        log.log_rename(1000, "/storage/1/a.md", "/storage/1/b.md");
        log.log_rename(2000, "/storage/1/c.md", "/storage/1/d.md");

        let renames = log.renames_since("/storage/1/", 1500);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames.get("b.md"), None); // the new path is d.md, old is c.md
    }
}
