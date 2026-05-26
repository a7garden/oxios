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
}
