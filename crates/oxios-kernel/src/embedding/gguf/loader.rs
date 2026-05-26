//! GGUF model download and file management for EmbeddingGemma.
//!
//! Downloads model files from HuggingFace Hub (`unsloth/embeddinggemma-300m-GGUF`)
//! and manages the local cache. Uses `llama-gguf::HfClient` for downloads.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use llama_gguf::HfClient;

/// HuggingFace model repository.
const MODEL_REPO: &str = "unsloth/embeddinggemma-300m-GGUF";

/// Specific GGUF quantization file to download.
const MODEL_FILE: &str = "embeddinggemma-300m-Q4_K_M.gguf";

/// Approximate model file size in MB (for display purposes).
const MODEL_SIZE_MB: u64 = 329;

/// Human-readable model name (for display purposes).
pub const MODEL_DISPLAY_NAME: &str = "EmbeddingGemma-300m";

/// Model loader for EmbeddingGemma via HuggingFace Hub.
pub struct GgufModelLoader;

impl GgufModelLoader {
    /// Ensure the GGUF model file is present. Downloads if missing.
    ///
    /// Uses `llama-gguf::HfClient` for download with caching.
    /// The `model_dir` is typically `~/.oxios/models/embeddinggemma-300m/`.
    ///
    /// # Returns
    /// Path to the GGUF file on disk.
    pub fn ensure_model(model_dir: &Path) -> Result<PathBuf> {
        let gguf_path = model_dir.join(MODEL_FILE);
        if gguf_path.exists() {
            tracing::debug!(path = %gguf_path.display(), "Model already cached");
            return Ok(gguf_path);
        }

        // Create directory
        std::fs::create_dir_all(model_dir)
            .with_context(|| format!("Failed to create model dir: {}", model_dir.display()))?;

        // Download via llama-gguf's built-in HuggingFace client
        tracing::info!(
            repo = MODEL_REPO,
            file = MODEL_FILE,
            "Downloading EmbeddingGemma GGUF model (~329MB)..."
        );

        let hf = HfClient::with_cache_dir(model_dir.to_path_buf());
        let downloaded = hf
            .download_file(MODEL_REPO, MODEL_FILE, true)
            .with_context(|| {
                format!(
                    "Failed to download {} from {}",
                    MODEL_FILE, MODEL_REPO
                )
            })?;

        // Copy to expected location if downloaded elsewhere
        if downloaded != gguf_path {
            std::fs::copy(&downloaded, &gguf_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    downloaded.display(),
                    gguf_path.display()
                )
            })?;
        }

        tracing::info!(path = %gguf_path.display(), "Model download complete");
        Ok(gguf_path)
    }

    /// Check if the model file is already cached on disk.
    pub fn is_model_cached(model_dir: &Path) -> bool {
        model_dir.join(MODEL_FILE).exists()
    }

    /// Prefetch the model file in the background (non-blocking).
    ///
    /// Spawns a blocking tokio task that calls [`ensure_model`].
    /// Errors are logged but not propagated — this is best-effort.
    ///
    /// Call this during startup (`oxios start`) or after onboarding
    /// so the model is ready before the first search.
    pub fn spawn_prefetch(model_dir: PathBuf) {
        if Self::is_model_cached(&model_dir) {
            tracing::debug!("Embedding model already cached, skipping prefetch");
            return;
        }

        tracing::info!(dir = %model_dir.display(), "Spawning background model prefetch (~329MB)");
        tokio::task::spawn_blocking(move || {
            match Self::ensure_model(&model_dir) {
                Ok(path) => {
                    tracing::info!(path = %path.display(), "Model prefetch complete");
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Model prefetch failed — will retry on first search"
                    );
                }
            }
        });
    }

    /// Get the model directory path for a given workspace.
    pub fn model_dir_for_workspace(workspace: &Path) -> PathBuf {
        workspace.join("models").join("embeddinggemma-300m")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_dir_path() {
        let dir = GgufModelLoader::model_dir_for_workspace(Path::new(
            "/home/user/.oxios/workspace",
        ));
        assert_eq!(
            dir,
            PathBuf::from("/home/user/.oxios/workspace/models/embeddinggemma-300m")
        );
    }

    #[test]
    fn test_is_model_cached_false() {
        let dir = PathBuf::from("/nonexistent/path");
        assert!(!GgufModelLoader::is_model_cached(&dir));
    }

    #[test]
    fn test_is_model_cached_true() {
        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("embeddinggemma-300m-Q4_K_M.gguf");
        std::fs::write(&model_path, b"fake model").unwrap();
        assert!(GgufModelLoader::is_model_cached(dir.path()));
    }
}
