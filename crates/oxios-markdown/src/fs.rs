//! Sandboxed filesystem abstraction for the knowledge base.
//!
//! Ported from files.md (`server/fs/fs.go`, `core/fs.rs`) by Artem Zakirullin.
//! Each knowledge base has its own root directory. All paths are validated
//! to prevent path traversal attacks.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use md5::{Digest as Md5Digest, Md5};

use crate::types::{FileEntry, FsError, DIR_ARCHIVE, DIR_JOURNAL, DIR_MEDIA, DIR_USER_ROOT};

/// Forbidden filename characters and their safe replacements.
const FORBIDDEN_CHARS: &[(&str, &str)] = &[
    ("<", "＜"),
    (">", "＞"),
    (":", "꞉"),
    ("\"", "″"),
    ("|", "⼁"),
    ("\\", "＼"),
    ("?", "？"),
    ("*", "﹡"),
    ("\x00", ""),
    ("/", "／"),
];

/// System directories to exclude from user-facing listings.
pub const SYSTEM_DIRS: &[&str] = &["archive", "media", "journal", "insights", "img"];

/// System files to exclude from user-facing listings.
pub const SYSTEM_FILES: &[&str] = &[
    "Chat.md", "Later.md", "Done.md", "Shop.md", "Watch.md", "Read.md",
];

/// Files/dirs to ignore during listing.
const IGNORED_NAMES: &[&str] = &[".", "..", ".obsidian", ".gitignore", ".DS_Store", ".git"];

// ============================================================================
// VirtualFs
// ============================================================================

/// Sandboxed filesystem for a single knowledge base.
///
/// All file operations are constrained to the root directory.
/// Path traversal attempts are rejected.
#[derive(Clone, Debug)]
pub struct VirtualFs {
    root: PathBuf,
    quota_kb: i64,
}

impl VirtualFs {
    /// Create a new VirtualFs rooted at the given directory.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        if !root.exists() {
            std::fs::create_dir_all(&root)?;
        }
        Ok(Self { root, quota_kb: 0 })
    }

    /// Set a storage quota in kilobytes (0 = unlimited).
    pub fn with_quota(mut self, quota_kb: i64) -> Self {
        self.quota_kb = quota_kb;
        self
    }

    /// Get the root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the configured quota in KB (0 = unlimited).
    pub fn quota_kb(&self) -> i64 {
        self.quota_kb
    }

    // ── Path Safety ──────────────────────────────────────────

    /// Build a safe absolute path from a directory and filename.
    ///
    /// Rejects path traversal attempts (e.g., `../../etc/passwd`).
    pub fn safe_path(&self, dir: &str, filename: &str) -> Result<PathBuf, FsError> {
        let dir_trimmed = dir.trim();
        if dir_trimmed.starts_with("..") {
            return Err(FsError::UnsafePath);
        }

        let relative: PathBuf = if dir == DIR_USER_ROOT {
            if filename.is_empty() {
                return Ok(self.root.clone());
            }
            PathBuf::from(filename)
        } else {
            PathBuf::from(dir).join(filename)
        };

        let rel_str = relative.to_string_lossy();
        if rel_str.starts_with('/') || rel_str.starts_with("../") {
            return Err(FsError::UnsafePath);
        }

        let full = self.root.join(&relative);

        // Normalize and verify we didn't escape root
        let stripped = full
            .strip_prefix(&self.root)
            .map_err(|_| FsError::UnsafePath)?;
        let (normalized, escaped) = normalize_path(stripped);
        if escaped || normalized.to_string_lossy().contains("..") {
            return Err(FsError::UnsafePath);
        }

        Ok(self.root.join(&normalized))
    }

    // ── POSIX Path API (단일 path 문자열) ────────────────────

    /// Read file content by POSIX-style relative path.
    /// `path` examples: "Rust.md", "brain/Rust.md", "journal/2024.08 August.md"
    pub fn read_path(&self, path: &str) -> Result<String, FsError> {
        let (dir, filename) = split_posix_path(path);
        self.read(dir, filename)
    }

    /// Write file content by POSIX-style relative path.
    pub fn write_path(&self, path: &str, content: &str) -> Result<(), FsError> {
        let (dir, filename) = split_posix_path(path);
        self.write(dir, filename, content)
    }

    /// Delete file by POSIX-style relative path.
    pub fn delete_path(&self, path: &str) -> Result<(), FsError> {
        let (dir, filename) = split_posix_path(path);
        self.del(dir, filename)
    }

    /// Rename/move file by POSIX-style relative paths.
    pub fn rename_path(&self, old_path: &str, new_path: &str) -> Result<(), FsError> {
        let (old_dir, old_filename) = split_posix_path(old_path);
        let (new_dir, new_filename) = split_posix_path(new_path);
        self.rename(old_dir, old_filename, new_dir, new_filename)
    }

    /// Check if file exists by POSIX-style relative path.
    pub fn exists_path(&self, path: &str) -> Result<bool, FsError> {
        let (dir, filename) = split_posix_path(path);
        self.exists(dir, filename)
    }

    /// Get mtime by POSIX-style relative path.
    pub fn mtime_path(&self, path: &str) -> Result<i64, FsError> {
        let (dir, filename) = split_posix_path(path);
        self.mtime(dir, filename)
    }

    // ── Basic I/O ───────────────────────────────────────────

    /// Check if a file or directory exists.
    pub fn exists(&self, dir: &str, filename: &str) -> Result<bool, FsError> {
        let path = self.safe_path(dir, filename)?;
        Ok(path.exists())
    }

    /// Read file contents as a string.
    pub fn read(&self, dir: &str, filename: &str) -> Result<String, FsError> {
        let path = self.safe_path(dir, filename)?;
        let mut file = std::fs::File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    /// Write content to a file, creating parent directories as needed.
    pub fn write(&self, dir: &str, filename: &str, content: &str) -> Result<(), FsError> {
        let path = self.safe_path(dir, filename)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if self.quota_kb > 0 {
            let new_size = content.len() as i64;
            let old_size = std::fs::metadata(&path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            let used = self.calculate_used_quota()?;
            let available = (self.quota_kb * 1024) - used;
            if (new_size - old_size) > available {
                return Err(FsError::QuotaExceeded);
            }
        }

        let mut file = std::fs::File::create(&path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Read a file as raw bytes.
    pub fn read_bytes(&self, dir: &str, filename: &str) -> Result<Vec<u8>, FsError> {
        let path = self.safe_path(dir, filename)?;
        Ok(std::fs::read(&path)?)
    }

    /// Write raw bytes to a file, creating parent directories as needed.
    /// Respects the configured quota (same logic as `write()`).
    pub fn write_bytes(&self, dir: &str, filename: &str, data: &[u8]) -> Result<(), FsError> {
        let path = self.safe_path(dir, filename)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if self.quota_kb > 0 {
            let new_size = data.len() as i64;
            let old_size = std::fs::metadata(&path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            let used = self.calculate_used_quota()?;
            let available = (self.quota_kb * 1024) - used;
            if (new_size - old_size) > available {
                return Err(FsError::QuotaExceeded);
            }
        }

        std::fs::write(&path, data)?;
        Ok(())
    }

    /// Read a file by POSIX path as raw bytes.
    pub fn read_path_bytes(&self, path: &str) -> Result<Vec<u8>, FsError> {
        let (dir, filename) = split_posix_path(path);
        self.read_bytes(dir, filename)
    }

    /// Write raw bytes to a file by POSIX path.
    pub fn write_path_bytes(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        let (dir, filename) = split_posix_path(path);
        self.write_bytes(dir, filename, data)
    }

    /// Delete a file.
    pub fn del(&self, dir: &str, filename: &str) -> Result<(), FsError> {
        let path = self.safe_path(dir, filename)?;
        std::fs::remove_file(&path)?;
        Ok(())
    }

    /// Rename/move a file.
    pub fn rename(
        &self,
        old_dir: &str,
        old_filename: &str,
        new_dir: &str,
        new_filename: &str,
    ) -> Result<(), FsError> {
        let old_path = self.safe_path(old_dir, old_filename)?;
        let new_path = self.safe_path(new_dir, new_filename)?;
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&old_path, &new_path)?;
        Ok(())
    }

    /// Create a directory.
    pub fn make_dir(&self, dir: &str) -> Result<(), FsError> {
        let path = self.safe_path(dir, "")?;
        std::fs::create_dir_all(&path)?;
        Ok(())
    }

    /// Touch a file: create if missing, update mtime if present.
    pub fn touch(&self, dir: &str, filename: &str) -> Result<(), FsError> {
        let path = self.safe_path(dir, filename)?;
        if path.exists() {
            let now = SystemTime::now();
            filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(now))?;
        } else {
            self.write(dir, filename, "")?;
        }
        Ok(())
    }

    // ── Metadata ─────────────────────────────────────────────

    /// Get the ctime/mtime of a file in milliseconds since epoch.
    pub fn ctime(&self, dir: &str, filename: &str) -> Result<i64, FsError> {
        let path = self.safe_path(dir, filename)?;
        let meta = std::fs::metadata(&path)?;
        Ok(mtime_to_ms(meta.modified()?))
    }

    /// Get the modification time of a file in milliseconds since epoch.
    pub fn mtime(&self, dir: &str, filename: &str) -> Result<i64, FsError> {
        let path = self.safe_path(dir, filename)?;
        let meta = std::fs::metadata(&path)?;
        Ok(mtime_to_ms(meta.modified()?))
    }

    /// Recursively collect mtimes for all files with given extensions.
    pub fn mtimes(&self, root: &str, extensions: &[&str]) -> Result<HashMap<String, i64>, FsError> {
        let root_path = self.safe_path(root, "")?;
        let mut result = HashMap::new();
        self.walk_dir(&root_path, &root_path, extensions, &mut result)?;
        Ok(result)
    }

    // ── Listing ─────────────────────────────────────────────

    /// List files and directories in a directory.
    pub fn files_and_dirs(&self, dir: &str) -> Result<Vec<FileEntry>, FsError> {
        let user_path = self.safe_path(dir, "")?;
        if !user_path.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&user_path)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if IGNORED_NAMES.contains(&name.as_str()) {
                continue;
            }

            let meta = std::fs::metadata(&path)?;
            let is_dir = meta.is_dir();
            let ctime = mtime_to_ms(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
            let hash = hash_filename(&name);
            let display_name = display_name(&name);
            let has_content = !is_dir && meta.len() > 0;

            entries.push(FileEntry::new(
                name,
                hash,
                display_name,
                ctime,
                has_content,
                is_dir,
                dir.to_string(),
            ));
        }
        Ok(entries)
    }

    /// List only directories in the root.
    pub fn dirs(&self) -> Result<Vec<FileEntry>, FsError> {
        Ok(self
            .files_and_dirs(DIR_USER_ROOT)?
            .into_iter()
            .filter(|f| f.is_dir)
            .collect())
    }

    /// Check if a file has non-whitespace content.
    pub fn is_multiline(&self, dir: &str, filename: &str) -> Result<bool, FsError> {
        let content = self.read(dir, filename)?;
        Ok(!content.trim().is_empty())
    }

    /// Create the standard system directories (archive, media, journal).
    pub fn create_system_dirs(&self) -> Result<(), FsError> {
        for dir in [DIR_ARCHIVE, DIR_MEDIA, DIR_JOURNAL] {
            self.make_dir(dir)?;
        }
        Ok(())
    }

    /// Reverse a hash to find the original filename.
    pub fn unhash(&self, dir: &str, filename_hash: &str) -> Result<String, FsError> {
        if dir == DIR_USER_ROOT && filename_hash == DIR_USER_ROOT {
            return Ok(DIR_USER_ROOT.to_string());
        }
        let files = self.files_and_dirs(dir)?;
        for file in &files {
            if hash_filename(&file.name).starts_with(filename_hash) {
                return Ok(file.name.clone());
            }
        }
        for file in &files {
            if file.name.starts_with(filename_hash) {
                return Ok(file.name.clone());
            }
        }
        Err(FsError::CannotUnhash)
    }

    /// Search files by name across the entire knowledge base.
    pub fn search_files_by_name(&self, query: &str) -> Result<Vec<FileEntry>, FsError> {
        let query_lower = query.to_lowercase().trim().to_string();
        if query_lower.contains('/') {
            return Err(FsError::UnsafePath);
        }

        let mut notes = Vec::new();
        self.collect_md_files(&self.root, &self.root, &mut notes)?;

        if !query_lower.is_empty() {
            let matching: Vec<FileEntry> = notes
                .iter()
                .filter(|f| {
                    let top = f.parent_dir.split('/').next().unwrap_or("");
                    top.to_lowercase().starts_with(&query_lower)
                        || f.display_name.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
            if !matching.is_empty() {
                notes = matching;
            }
        }

        notes.sort_by_key(|a| Reverse(a.ctime));
        Ok(notes)
    }

    // ── Private helpers ─────────────────────────────────────

    #[allow(clippy::only_used_in_recursion)]
    fn walk_dir(
        &self,
        root_path: &Path,
        current_path: &Path,
        extensions: &[&str],
        result: &mut HashMap<String, i64>,
    ) -> Result<(), FsError> {
        if !current_path.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(current_path)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if filename.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                self.walk_dir(root_path, &path, extensions, result)?;
            } else {
                if !extensions.is_empty() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| format!(".{e}"));
                    let ext_match = ext
                        .as_ref()
                        .map(|e| extensions.contains(&e.as_str()))
                        .unwrap_or(false);
                    if !ext_match {
                        continue;
                    }
                }

                let rel = path
                    .strip_prefix(root_path)
                    .map_err(|_| FsError::UnsafePath)?;
                let display = rel.to_string_lossy();
                let display_path = if display.starts_with('/') || display.starts_with('\\') {
                    display[1..].to_string()
                } else {
                    display.to_string()
                };

                let meta = std::fs::metadata(&path)?;
                result.insert(display_path, mtime_to_ms(meta.modified()?));
            }
        }
        Ok(())
    }

    #[allow(clippy::only_used_in_recursion)]
    fn collect_md_files(
        &self,
        root_path: &Path,
        current_path: &Path,
        files: &mut Vec<FileEntry>,
    ) -> Result<(), FsError> {
        if !current_path.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(current_path)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if path.is_dir() {
                if filename.starts_with('.') {
                    continue;
                }
                self.collect_md_files(root_path, &path, files)?;
            } else {
                if !filename.ends_with(".md") || filename.starts_with('.') {
                    continue;
                }

                let meta = std::fs::metadata(&path)?;
                let rel = path
                    .strip_prefix(root_path)
                    .map_err(|_| FsError::UnsafePath)?;
                let parent = rel
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let parent_str = if parent.is_empty() || parent == "." {
                    DIR_USER_ROOT.to_string()
                } else {
                    parent
                };

                let ctime = mtime_to_ms(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
                let hash = hash_filename(filename);
                let display_name = display_name(filename);

                files.push(FileEntry::new(
                    filename.to_string(),
                    hash,
                    display_name,
                    ctime,
                    meta.len() > 0,
                    false,
                    parent_str,
                ));
            }
        }
        Ok(())
    }

    fn calculate_used_quota(&self) -> std::io::Result<i64> {
        let mut total = 0i64;
        if self.root.exists() {
            for entry in std::fs::read_dir(&self.root)? {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_file() {
                    total += meta.len() as i64;
                } else if meta.is_dir() {
                    total += dir_size(entry.path())?;
                }
            }
        }
        Ok(total)
    }
}

// ============================================================================
// Free Functions
// ============================================================================

/// Compute MD5 hash of a filename (first 11 hex characters).
pub fn hash_filename(filename: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(filename.as_bytes());
    hex::encode(hasher.finalize())[..11].to_string()
}

/// Compute short hash (first 5 hex characters).
pub fn short_hash(filename: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(filename.as_bytes());
    hex::encode(hasher.finalize())[..5].to_string()
}

/// Sanitize a filename by replacing forbidden characters.
pub fn sanitize_filename(filename: &str) -> String {
    let mut result = filename.to_string();
    for (forbidden, safe) in FORBIDDEN_CHARS {
        result = result.replace(forbidden, safe);
    }
    result
}

/// Reverse sanitize: restore original forbidden characters.
pub fn unsanitize_filename(filename: &str) -> String {
    let mut result = filename.to_string();
    for (forbidden, safe) in FORBIDDEN_CHARS {
        if !forbidden.is_empty() && *forbidden != "\x00" {
            result = result.replace(safe, forbidden);
        }
    }
    result
}

/// Get display name from filename: capitalized, without `.md` extension.
pub fn display_name(filename: &str) -> String {
    let trimmed = filename.trim();
    let without_ext = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    let mut chars = without_ext.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// Check if a filename represents a checklist item.
pub fn is_checklist_item(filename: &str) -> bool {
    let trimmed = filename.trim();
    if !trimmed.starts_with('-') {
        return false;
    }
    if let Some(pos) = trimmed.rfind('-') {
        pos > 0 && pos < trimmed.len() - 1
    } else {
        false
    }
}

/// Filter: exclude checklist files.
pub fn exclude_checklists(files: &[FileEntry]) -> Vec<FileEntry> {
    files
        .iter()
        .filter(|f| {
            let name = f.name.trim_end_matches(".md");
            !(name.starts_with('_') && name.ends_with('_'))
        })
        .cloned()
        .collect()
}

/// Filter: exclude system directories.
pub fn exclude_system_dirs(files: &[FileEntry]) -> Vec<FileEntry> {
    files
        .iter()
        .filter(|f| !SYSTEM_DIRS.contains(&f.name.as_str()))
        .cloned()
        .collect()
}

/// Filter: exclude system files.
pub fn exclude_system_files(files: &[FileEntry]) -> Vec<FileEntry> {
    files
        .iter()
        .filter(|f| !SYSTEM_FILES.contains(&f.name.as_str()))
        .cloned()
        .collect()
}

/// Filter: only directories.
pub fn only_dirs(files: &[FileEntry]) -> Vec<FileEntry> {
    files.iter().filter(|f| f.is_dir).cloned().collect()
}

/// Filter: only files (not directories).
pub fn only_files(files: &[FileEntry]) -> Vec<FileEntry> {
    files.iter().filter(|f| !f.is_dir).cloned().collect()
}

/// Filter: only user markdown files (exclude system files, dirs, non-md).
pub fn only_user_md_files(files: &[FileEntry]) -> Vec<FileEntry> {
    files
        .iter()
        .filter(|f| {
            !f.is_dir && f.name.ends_with(".md") && !SYSTEM_FILES.contains(&f.name.as_str())
        })
        .cloned()
        .collect()
}

/// Sort files by ctime descending (newest first).
pub fn sort_by_ctime_desc(files: &mut [FileEntry]) {
    files.sort_by_key(|a| Reverse(a.ctime));
}

/// Extract filenames from a list of file entries.
pub fn only_filenames(files: &[FileEntry]) -> Vec<String> {
    files.iter().map(|f| f.name.clone()).collect()
}

/// Split a POSIX-style path like "brain/Rust.md" into (dir, filename).
/// Root-level files like "Chat.md" become ("/", "Chat.md").
pub fn split_posix_path(path: &str) -> (&str, &str) {
    let path = path.trim_start_matches('/');
    if let Some(slash_pos) = path.rfind('/') {
        let (dir, file) = path.split_at(slash_pos);
        (dir, &file[1..])
    } else {
        (crate::types::DIR_USER_ROOT, path)
    }
}

// ── Internal helpers ────────────────────────────────────────

fn normalize_path(path: &Path) -> (PathBuf, bool) {
    let mut components = Vec::new();
    let mut escaped = false;
    for component in path.components() {
        match component {
            std::path::Component::Normal(s) => components.push(s),
            std::path::Component::ParentDir => {
                if components.is_empty() {
                    escaped = true;
                } else {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {}
        }
    }
    (components.iter().collect(), escaped)
}

fn mtime_to_ms(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn dir_size(path: PathBuf) -> std::io::Result<i64> {
    let mut total = 0i64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_file() {
            total += meta.len() as i64;
        } else if meta.is_dir() {
            total += dir_size(entry.path())?;
        }
    }
    Ok(total)
}

// ============================================================================
// Tests
// ============================================================================

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
    fn test_write_and_read() {
        let (fs, _t) = test_fs();
        fs.write("brain", "test.md", "Hello").unwrap();
        assert_eq!(fs.read("brain", "test.md").unwrap(), "Hello");
    }

    #[test]
    fn test_exists() {
        let (fs, _t) = test_fs();
        assert!(!fs.exists("/", "nope.md").unwrap());
        fs.write("/", "exists.md", "x").unwrap();
        assert!(fs.exists("/", "exists.md").unwrap());
    }

    #[test]
    fn test_delete() {
        let (fs, _t) = test_fs();
        fs.write("/", "del.md", "x").unwrap();
        fs.del("/", "del.md").unwrap();
        assert!(!fs.exists("/", "del.md").unwrap());
    }

    #[test]
    fn test_rename() {
        let (fs, _t) = test_fs();
        fs.write("/", "old.md", "data").unwrap();
        fs.rename("/", "old.md", "/", "new.md").unwrap();
        assert!(!fs.exists("/", "old.md").unwrap());
        assert_eq!(fs.read("/", "new.md").unwrap(), "data");
    }

    #[test]
    fn test_path_traversal_rejected() {
        let (fs, _t) = test_fs();
        assert!(fs.safe_path("../etc", "passwd").is_err());
        assert!(fs.safe_path("a", "../../etc/passwd").is_err());
    }

    #[test]
    fn test_touch_creates_file() {
        let (fs, _t) = test_fs();
        fs.touch("/", "new.md").unwrap();
        assert!(fs.exists("/", "new.md").unwrap());
    }

    #[test]
    fn test_hash_filename_deterministic() {
        assert_eq!(hash_filename("test.md"), hash_filename("test.md"));
        assert_eq!(hash_filename("test.md").len(), 11);
    }

    #[test]
    fn test_display_name() {
        assert_eq!(display_name("rust.md"), "Rust");
        assert_eq!(display_name(" filename "), "Filename");
    }

    #[test]
    fn test_sanitize_roundtrip() {
        let original = "test/file:name";
        let sanitized = sanitize_filename(original);
        assert_ne!(sanitized, original);
        assert_eq!(unsanitize_filename(&sanitized), original);
    }

    #[test]
    fn test_files_and_dirs() {
        let (fs, _t) = test_fs();
        fs.make_dir("brain").unwrap();
        fs.write("brain", "Rust.md", "content").unwrap();
        let entries = fs.files_and_dirs("brain").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Rust.md");
    }

    #[test]
    fn test_create_system_dirs() {
        let (fs, _t) = test_fs();
        fs.create_system_dirs().unwrap();
        assert!(fs.exists(DIR_ARCHIVE, "").unwrap());
        assert!(fs.exists(DIR_MEDIA, "").unwrap());
        assert!(fs.exists(DIR_JOURNAL, "").unwrap());
    }

    #[test]
    fn test_mtimes() {
        let (fs, _t) = test_fs();
        fs.write("/", "a.md", "a").unwrap();
        let mtimes = fs.mtimes("/", &[".md"]).unwrap();
        assert!(mtimes.contains_key("a.md"));
    }

    #[test]
    fn test_search_files_by_name() {
        let (fs, _t) = test_fs();
        fs.make_dir("brain").unwrap();
        fs.write("brain", "Rust.md", "").unwrap();
        let results = fs.search_files_by_name("brain").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_unhash() {
        let (fs, _t) = test_fs();
        fs.write("/", "target.md", "x").unwrap();
        let h = hash_filename("target.md");
        assert_eq!(fs.unhash("/", &h).unwrap(), "target.md");
    }

    #[test]
    fn test_filter_functions() {
        let f = FileEntry::new(
            "a.md".into(),
            "h".into(),
            "A".into(),
            0,
            true,
            false,
            "/".into(),
        );
        let d = FileEntry::new(
            "dir".into(),
            "h".into(),
            "Dir".into(),
            0,
            false,
            true,
            "/".into(),
        );
        assert_eq!(only_dirs(&[f.clone(), d.clone()]).len(), 1);
        assert_eq!(only_files(&[f.clone(), d]).len(), 1);
    }

    #[test]
    fn test_quota_enforcement() {
        let dir = TempDir::new().unwrap();
        let fs = VirtualFs::new(dir.path().to_path_buf())
            .unwrap()
            .with_quota(1); // 1 KB
        assert!(fs.write("/", "big.md", &"x".repeat(2048)).is_err());
    }

    #[test]
    fn test_read_write_bytes() {
        let (fs, _t) = test_fs();
        let data: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]; // PNG header fragment
        fs.write_bytes("media", "image.png", data).unwrap();
        let read_back = fs.read_bytes("media", "image.png").unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_write_bytes_quota() {
        let dir = TempDir::new().unwrap();
        let fs = VirtualFs::new(dir.path().to_path_buf())
            .unwrap()
            .with_quota(1); // 1 KB
        let big = vec![0u8; 2048];
        assert!(fs.write_bytes("/", "big.bin", &big).is_err());
    }

    #[test]
    fn test_path_bytes_roundtrip() {
        let (fs, _t) = test_fs();
        let data = b"\x00\x01\x02\xFF binary data";
        fs.write_path_bytes("sub/file.bin", data).unwrap();
        let read_back = fs.read_path_bytes("sub/file.bin").unwrap();
        assert_eq!(read_back, data);
    }
}
