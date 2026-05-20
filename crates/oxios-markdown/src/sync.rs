//! Sync engine for client-server file synchronization.
//!
//! Ported from files.md (`server/sync/sync.go`) by Artem Zakirullin.
//! Implements mtime-based 3-way merge synchronization with conflict resolution.

use std::collections::HashMap;

use crate::fs::VirtualFs;
use crate::fslog::FsLog;
use crate::merge::merge;
use crate::types::{
    FsError, SyncError, SyncFile, SyncRequest, SyncResponse,
    STATUS_MERGED, STATUS_NOT_MODIFIED, STATUS_OK, STATUS_UPDATED_ON_SERVER,
    DIR_MEDIA, DIR_USER_ROOT, MD_EXT,
};

/// Configuration for the sync engine.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Knowledge base config filename (usually "config.json").
    pub config_filename: String,
    /// Storage directory prefix for user data.
    pub storage_dir: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            config_filename: "config.json".to_string(),
            storage_dir: String::new(),
        }
    }
}

/// Sync engine: handles batch and single-file synchronization.
pub struct SyncEngine {
    fs: VirtualFs,
    config: SyncConfig,
    fslog: FsLog,
}

impl SyncEngine {
    /// Create a new sync engine.
    pub fn new(fs: VirtualFs, config: SyncConfig, fslog: FsLog) -> Self {
        Self { fs, config, fslog }
    }

    /// Get a reference to the underlying filesystem.
    pub fn fs(&self) -> &VirtualFs {
        &self.fs
    }

    /// Perform batch file synchronization.
    ///
    /// Algorithm:
    /// 1. Apply client deletions
    /// 2. Save client modifications (merge on conflict)
    /// 3. Send server files that the client doesn't have
    /// 4. Include rename log entries
    pub fn sync_filenames(&self, user_id: i64, request: SyncRequest) -> Result<SyncResponse, SyncError> {
        let mut files_to_send: Vec<SyncFile> = Vec::new();
        let mut dir_timestamps: HashMap<String, i64> = HashMap::new();

        let mut last_sync: i64 = 0;
        for ts in request.timestamps.values() {
            if *ts > last_sync { last_sync = *ts; }
        }

        let renames = if last_sync != 0 {
            let user_prefix = format!("{}/{}/", self.config.storage_dir, user_id);
            self.fslog.renames_since(&user_prefix, last_sync)
        } else {
            HashMap::new()
        };

        // Process deletions
        for path in &request.deleted {
            let rel = path.trim_start_matches('/');
            let _ = self.fs.del(DIR_USER_ROOT, rel);
        }

        // Process modifications
        for client_file in &request.modified {
            let rel = client_file.path.trim_start_matches('/');
            let server_mtime = self.fs.mtime(DIR_USER_ROOT, rel).ok();
            let mut content = client_file.content.clone();

            match server_mtime {
                None => {} // New file, use client content
                Some(server_modified) => {
                    if server_modified > client_file.last_modified {
                        if let Ok(server_content) = self.fs.read(DIR_USER_ROOT, rel) {
                            content = merge(&server_content, &client_file.content);
                        }
                    }
                }
            }

            // Skip config file
            if client_file.path == self.config.config_filename { continue; }

            match self.fs.write(DIR_USER_ROOT, rel, &content) {
                Err(FsError::QuotaExceeded) => return Err(SyncError::QuotaExceeded),
                Err(e) => tracing::warn!(path = %rel, error = %e, "Sync write failed"),
                Ok(_) => {}
            }
        }

        // Build response with files the client needs
        let server_timestamps = self.fs.mtimes(DIR_USER_ROOT, &[MD_EXT, ".txt"])
            .map_err(|e| SyncError::Storage(e.to_string()))?;

        for (path, server_time) in &server_timestamps {
            let parts: Vec<&str> = path.split('/').collect();
            let dir = if parts.len() == 1 { "." } else { parts[0] };
            let client_dir_time = request.timestamps.get(dir).copied().unwrap_or(0);

            if server_time > &client_dir_time {
                if let Ok(content) = self.fs.read(DIR_USER_ROOT, path) {
                    files_to_send.push(SyncFile {
                        status: STATUS_OK.to_string(),
                        path: path.clone(),
                        last_modified: *server_time,
                        client_last_modified: 0,
                        client_last_synced: 0,
                        content,
                    });
                }
            }

            let existing = dir_timestamps.get(dir).copied().unwrap_or(0);
            if *server_time > existing {
                dir_timestamps.insert(dir.to_string(), *server_time);
            }
        }

        Ok(SyncResponse {
            status: STATUS_OK.to_string(),
            files: files_to_send,
            timestamps: dir_timestamps,
            renames,
        })
    }

    /// Synchronize a single file.
    pub fn sync_file(&self, _user_id: i64, client_file: SyncFile) -> Result<SyncResponse, SyncError> {
        let rel = client_file.path.trim_start_matches('/');
        let server_content = self.fs.read(DIR_USER_ROOT, rel).ok();
        let server_mtime = self.fs.mtime(DIR_USER_ROOT, rel).ok().unwrap_or(0);

        // Already up to date?
        if let Some(ref content) = server_content {
            if *content == client_file.content {
                return Ok(SyncResponse {
                    status: STATUS_NOT_MODIFIED.to_string(),
                    ..SyncResponse::default()
                });
            }
        }

        let mut status = STATUS_OK.to_string();
        let mut content = client_file.content.clone();
        let mut should_update = true;

        if let Some(ref server_content) = server_content {
            let not_modified_on_client = client_file.client_last_synced != 0
                && client_file.client_last_modified == client_file.client_last_synced;
            let modified_on_server = server_mtime > client_file.last_modified;

            if modified_on_server && not_modified_on_client {
                content = server_content.clone();
                should_update = false;
            } else if modified_on_server {
                content = merge(server_content, &client_file.content);
                status = STATUS_MERGED.to_string();
            }
        }

        if should_update {
            self.fs.write(DIR_USER_ROOT, rel, &content)
                .map_err(|e| SyncError::Storage(e.to_string()))?;
            return Ok(SyncResponse {
                status: STATUS_UPDATED_ON_SERVER.to_string(),
                ..SyncResponse::default()
            });
        }

        let final_mtime = self.fs.mtime(DIR_USER_ROOT, rel).unwrap_or(0);
        Ok(SyncResponse {
            status: status.clone(),
            files: vec![SyncFile {
                status,
                path: client_file.path,
                last_modified: final_mtime,
                client_last_modified: client_file.last_modified,
                client_last_synced: client_file.client_last_synced,
                content,
            }],
            ..SyncResponse::default()
        })
    }
}

// ── Media types and methods ───────────────────────────────

/// Media file entry.
#[derive(Debug, Clone)]
pub struct MediaEntry {
    /// Filename within the media directory.
    pub filename: String,
    /// Last modified timestamp (millis since epoch).
    pub last_modified: i64,
}

/// Media sync response.
#[derive(Debug, Clone)]
pub struct MediaSyncResponse {
    /// Media files modified since the given timestamp.
    pub files: Vec<MediaEntry>,
    /// Latest modification timestamp among returned files.
    pub timestamp: i64,
}

impl SyncEngine {
    /// List media files modified since a given timestamp.
    ///
    /// Returns all media files whose mtime > `since_timestamp`,
    /// along with the latest timestamp for incremental sync.
    pub fn sync_media_filenames(&self, since_timestamp: i64) -> Result<MediaSyncResponse, SyncError> {
        let mtimes = self.fs
            .mtimes(DIR_MEDIA, &[])
            .map_err(|e| SyncError::Storage(e.to_string()))?;

        let mut files: Vec<MediaEntry> = Vec::new();
        let mut latest_timestamp: i64 = 0;

        for (filename, mod_time) in &mtimes {
            if *mod_time <= since_timestamp {
                continue;
            }
            if *mod_time > latest_timestamp {
                latest_timestamp = *mod_time;
            }
            files.push(MediaEntry {
                filename: filename.clone(),
                last_modified: *mod_time,
            });
        }

        Ok(MediaSyncResponse {
            files,
            timestamp: latest_timestamp,
        })
    }

    /// Upload a media file (from raw bytes).
    pub fn sync_media_upload(&self, filename: &str, data: &[u8]) -> Result<(), SyncError> {
        let exists = self.fs.exists(DIR_MEDIA, filename)
            .map_err(|e| SyncError::Storage(e.to_string()))?;

        if exists {
            // File already exists, skip
            return Ok(());
        }

        self.fs.write_bytes(DIR_MEDIA, filename, data)
            .map_err(|e| match e {
                FsError::QuotaExceeded => SyncError::QuotaExceeded,
                other => SyncError::Storage(other.to_string()),
            })?;

        Ok(())
    }

    /// Read a media file as raw bytes.
    pub fn sync_media_read(&self, filename: &str) -> Result<Vec<u8>, SyncError> {
        self.fs.read_bytes(DIR_MEDIA, filename)
            .map_err(|e| SyncError::Storage(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_engine() -> (SyncEngine, TempDir) {
        let dir = TempDir::new().unwrap();
        let fs = VirtualFs::new(dir.path().to_path_buf()).unwrap();
        let fslog = FsLog::new(dir.path().join("fslog"));
        let config = SyncConfig { config_filename: "config.json".into(), storage_dir: dir.path().to_string_lossy().to_string() };
        (SyncEngine::new(fs, config, fslog), dir)
    }

    #[test]
    fn test_sync_file_new() {
        let (engine, _t) = test_engine();
        let resp = engine.sync_file(1, SyncFile {
            status: String::new(), path: "test.md".into(),
            last_modified: 0, client_last_modified: 0, client_last_synced: 0,
            content: "hello".into(),
        }).unwrap();
        assert_eq!(resp.status, STATUS_UPDATED_ON_SERVER);
    }

    #[test]
    fn test_sync_file_not_modified() {
        let (engine, _t) = test_engine();
        engine.fs.write(DIR_USER_ROOT, "test.md", "hello").unwrap();
        let resp = engine.sync_file(1, SyncFile {
            status: String::new(), path: "test.md".into(),
            last_modified: 0, client_last_modified: 0, client_last_synced: 0,
            content: "hello".into(),
        }).unwrap();
        assert_eq!(resp.status, STATUS_NOT_MODIFIED);
    }

    #[test]
    fn test_batch_sync_creates_files() {
        let (engine, _t) = test_engine();
        let resp = engine.sync_filenames(1, SyncRequest {
            modified: vec![SyncFile {
                status: String::new(), path: "new.md".into(),
                last_modified: 0, client_last_modified: 0, client_last_synced: 0,
                content: "new content".into(),
            }],
            deleted: vec![],
            timestamps: HashMap::new(),
        }).unwrap();
        assert_eq!(resp.status, STATUS_OK);
        assert!(engine.fs.exists(DIR_USER_ROOT, "new.md").unwrap());
    }

    #[test]
    fn test_sync_media_upload_and_read() {
        let (engine, _t) = test_engine();
        engine.fs.make_dir(DIR_MEDIA).unwrap();

        // Binary data that is NOT valid UTF-8
        let data: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0xFF, 0xD8, 0x00];

        engine.sync_media_upload("photo.png", data).unwrap();

        let read_back = engine.sync_media_read("photo.png").unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_sync_media_upload_skips_existing() {
        let (engine, _t) = test_engine();
        engine.fs.make_dir(DIR_MEDIA).unwrap();

        engine.sync_media_upload("file.bin", b"original").unwrap();
        // Uploading again should skip (no overwrite)
        engine.sync_media_upload("file.bin", b"updated").unwrap();

        let content = engine.sync_media_read("file.bin").unwrap();
        assert_eq!(content, b"original");
    }
}
