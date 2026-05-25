//! Model download and file management for EmbeddingGemma.
//!
//! Downloads model files from HuggingFace Hub (mlx-community/embeddinggemma-300m-4bit)
//! and caches them locally. Uses `hf-hub` for download management.

use std::path::Path;

use anyhow::{Context, Result};

/// HuggingFace model repository.
const MODEL_REPO: &str = "mlx-community/embeddinggemma-300m-4bit";

/// Files required for the embedding model.
const REQUIRED_FILES: &[&str] = &[
    "model.safetensors",
    "config.json",
    "tokenizer.json",
    "tokenizer.model",
    "tokenizer_config.json",
    "special_tokens_map.json",
];

/// Model loader for EmbeddingGemma via HuggingFace Hub.
pub struct MlxModelLoader;

impl MlxModelLoader {
    /// Ensure all model files are present. Downloads if missing.
    ///
    /// Uses `hf-hub` to download from `mlx-community/embeddinggemma-300m-4bit`.
    /// Files are cached in the standard HuggingFace cache directory.
    /// The `model_dir` is typically `~/.oxios/models/embeddinggemma-300m-4bit/`.
    pub fn ensure_model(model_dir: &Path) -> Result<()> {
        // Check if model already exists
        if model_dir.join("model.safetensors").exists()
            && model_dir.join("config.json").exists()
            && model_dir.join("tokenizer.json").exists()
        {
            tracing::debug!(dir = %model_dir.display(), "Model files already present");
            return Ok(());
        }

        // Create directory
        std::fs::create_dir_all(model_dir)
            .with_context(|| format!("Failed to create model dir: {}", model_dir.display()))?;

        // Download via hf-hub
        tracing::info!(repo = MODEL_REPO, "Downloading EmbeddingGemma model (~173MB)...");

        let api = hf_hub::api::sync::ApiBuilder::new()
            .with_cache_dir(model_dir.parent().unwrap_or(model_dir).to_path_buf())
            .build()
            .context("Failed to create HuggingFace API client")?;

        let repo = api.model(MODEL_REPO.to_string());

        for filename in REQUIRED_FILES {
            match repo.get(filename) {
                Ok(path) => {
                    // Copy/symlink to model_dir
                    let dest = model_dir.join(filename);
                    if !dest.exists() {
                        // hf-hub stores in its own cache; copy to our model_dir
                        std::fs::copy(&path, &dest).with_context(|| {
                            format!("Failed to copy {} to {}", path.display(), dest.display())
                        })?;
                    }
                    tracing::debug!(file = filename, "Downloaded");
                }
                Err(e) => {
                    // Non-critical files (special_tokens_map, etc.) can be missing
                    if filename == &"model.safetensors" || filename == &"config.json" {
                        anyhow::bail!("Failed to download required file {}: {}", filename, e);
                    }
                    tracing::warn!(file = filename, error = %e, "Optional file not found");
                }
            }
        }

        tracing::info!(dir = %model_dir.display(), "Model download complete");
        Ok(())
    }

    /// Load the tokenizer from the model directory.
    pub fn load_tokenizer(model_dir: &Path) -> Result<tokenizers::Tokenizer> {
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer from {}: {}", tokenizer_path.display(), e))?;
        Ok(tokenizer)
    }

    /// Get the model directory path for a given workspace.
    pub fn model_dir_for_workspace(workspace: &Path) -> std::path::PathBuf {
        workspace.join("models").join("embeddinggemma-300m-4bit")
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
        let dir = MlxModelLoader::model_dir_for_workspace(Path::new("/home/user/.oxios/workspace"));
        assert_eq!(
            dir,
            std::path::PathBuf::from("/home/user/.oxios/workspace/models/embeddinggemma-300m-4bit")
        );
    }
}
