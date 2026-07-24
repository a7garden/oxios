//! Persistence for generated image bytes.
//!
//! Providers that return only base64 (e.g. `gpt-image-1`) need their output
//! materialized to disk and served under a stable URL so agents can emit
//! markdown `![](url)`. The sink is a trait so the provider logic stays
//! unit-testable with a fake.

use std::path::PathBuf;

use crate::image_gen::ImageGenError;

/// Persists image bytes and returns a URL the frontend can fetch.
pub trait ImageSink: Send + Sync {
    /// Write `bytes` with extension `ext`, return the served URL.
    fn save(&self, bytes: Vec<u8>, ext: &str) -> Result<String, ImageGenError>;
}

/// Filesystem-backed store. Writes into `dir`, serves under `serve_prefix`.
///
/// `serve_prefix` is the binary's API path (default `/api/images/`); the
/// actual `GET /api/images/:name` route lives in the binary crate.
pub struct FsImageStore {
    dir: PathBuf,
    serve_prefix: String,
}

impl FsImageStore {
    /// Create a new store rooted at `dir`, serving under `serve_prefix`.
    pub fn new(dir: PathBuf, serve_prefix: String) -> Self {
        Self { dir, serve_prefix }
    }
}

impl ImageSink for FsImageStore {
    fn save(&self, bytes: Vec<u8>, ext: &str) -> Result<String, ImageGenError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| ImageGenError::Store(e.to_string()))?;
        let name = format!("{}.{ext}", uuid::Uuid::new_v4());
        std::fs::write(self.dir.join(&name), &bytes)
            .map_err(|e| ImageGenError::Store(e.to_string()))?;
        Ok(format!("{}{name}", self.serve_prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_writes_file_and_returns_prefixed_url() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FsImageStore::new(tmp.path().to_path_buf(), "/api/images/".into());
        let url = store.save(vec![1, 2, 3, 4], "png").unwrap();
        assert!(url.starts_with("/api/images/"));
        assert!(url.ends_with(".png"));
        // File materialized on disk.
        let name = url.strip_prefix("/api/images/").unwrap();
        let written = std::fs::read(tmp.path().join(name)).unwrap();
        assert_eq!(written, vec![1, 2, 3, 4]);
    }

    #[test]
    fn save_creates_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("deeply").join("nested");
        let store = FsImageStore::new(nested, "/api/images/".into());
        let url = store.save(vec![0], "png").unwrap();
        assert!(url.starts_with("/api/images/"));
    }
}
