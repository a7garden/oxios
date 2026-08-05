//! Image generation tool — wraps the `image_gen` provider behind `AgentTool`.
//!
//! Action-based (mirrors LobeHub's 4-API builtin tool, pared down to the
//! synchronous Phase-1 surface):
//! - `generate` — generate images from a prompt.
//! - `list_models` — echo the configured provider/model (availability check).
//!
//! The provider, base URL, and default model come from `[image-gen]` config.
//! The API key is resolved via [`CredentialStore`] using the same chain as
//! the chat engine — no separate credential is needed.
//!
//! Config is read once at construction (a boot-time snapshot via
//! `kernel.infra.config()`); the tool registers per-agent, so Phase-1 does
//! not need hot-reload.

use std::sync::Arc;

use async_trait::async_trait;
use oxicode_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{Value, json};

use crate::credential::CredentialStore;
use crate::image_gen::{
    FalImageProvider, FsImageStore, GeneratedImage, ImageGenProvider, ImageGenRequest, ImageSink,
    ImageSize, OpenAiImageProvider,
};
use crate::kernel_handle::KernelHandle;

/// URL prefix under which persisted images are served (fallback for FsImageStore).
const IMAGE_SERVE_PREFIX: &str = "/api/images/";

/// Agent tool for image generation (Phase 1: OpenAI-compatible providers).
pub struct ImageGenerationTool {
    provider: String,
    base_url: String,
    default_model: String,
    default_num: u8,
    /// `[engine].api_key` override forwarded to credential resolution.
    engine_api_key: Option<String>,
    /// Image sink — AssetStore when available, FsImageStore fallback.
    image_sink: Arc<dyn ImageSink>,
}

impl ImageGenerationTool {
    /// Create from a [`KernelHandle`].
    ///
    /// Reads the `[image-gen]` config snapshot once and resolves the image
    /// sink: [`AssetStore`] when attached (unified store), falling back to
    /// [`FsImageStore`] writing to `<workspace>/images/`.
    ///
    /// [`AssetStore`]: crate::asset_store::AssetStore
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        let cfg = kernel.infra.config();
        let ig = &cfg.image_gen;
        let image_sink: Arc<dyn ImageSink> = kernel
            .asset_store
            .as_ref()
            .map(|s| s.clone() as Arc<dyn ImageSink>)
            .unwrap_or_else(|| {
                Arc::new(FsImageStore::new(
                    kernel.state.workspace_path().join("images"),
                    IMAGE_SERVE_PREFIX.into(),
                )) as Arc<dyn ImageSink>
            });
        Self {
            provider: ig.provider.clone(),
            base_url: ig.base_url.clone(),
            default_model: ig.default_model.clone(),
            default_num: ig.default_num,
            engine_api_key: cfg.api_key(),
            image_sink,
        }
    }
}

impl std::fmt::Debug for ImageGenerationTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageGenerationTool")
            .field("provider", &self.provider)
            .field("image_sink", &"<dyn ImageSink>")
            .finish()
    }
}

#[async_trait]
impl AgentTool for ImageGenerationTool {
    fn name(&self) -> &str {
        "image_generation"
    }

    fn label(&self) -> &str {
        "Image Generation"
    }

    fn description(&self) -> &'static str {
        "Generate images from a text prompt via an OpenAI-compatible image model. \
         When generation completes, show each image by emitting markdown \
         `![](url)` using the URLs from the result EXACTLY as given — do not \
         rewrite, shorten, or translate them. Include a brief caption only. \
         Do not retry automatically on content-policy or billing errors; \
         report the error concisely instead."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["generate", "list_models"],
                    "description": "Operation: 'generate' (default) or 'list_models' (show configured model)."
                },
                "prompt": {
                    "type": "string",
                    "description": "Text-to-image prompt (required for 'generate')."
                },
                "model": {
                    "type": "string",
                    "description": "Provider model id. Omit to use the configured default."
                },
                "n": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 8,
                    "default": 1,
                    "description": "Number of images to generate."
                },
                "size": {
                    "type": "string",
                    "enum": ["1024x1024", "1792x1024", "1024x1792"],
                    "description": "Output dimensions. Omit for provider default."
                },
                "quality": {
                    "type": "string",
                    "description": "Quality hint (e.g. 'standard', 'hd'). Provider-specific."
                },
                "reference_image_url": {
                    "type": "string",
                    "description": "Reference image URL for image-to-image (fal providers). Optional."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxicode_sdk::ToolError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("generate");

        if action == "list_models" {
            return Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "provider": self.provider,
                    "default_model": self.default_model,
                    "default_num": self.default_num,
                    "sizes": ["1024x1024", "1792x1024", "1024x1792"],
                }))
                .unwrap_or_default(),
            ));
        }

        if action != "generate" {
            return Err(format!(
                "Unknown action '{action}'. Valid: generate, list_models."
            ));
        }

        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: prompt".to_string())?;

        // Model: explicit param → configured default → error.
        let model = params
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                if self.default_model.is_empty() {
                    None
                } else {
                    Some(self.default_model.clone())
                }
            })
            .ok_or_else(|| {
                "No model specified and no [image-gen].default_model configured".to_string()
            })?;

        let n = params
            .get("n")
            .and_then(|v| v.as_u64())
            .unwrap_or(u64::from(self.default_num)) as u8;

        let size = params
            .get("size")
            .and_then(|v| v.as_str())
            .and_then(parse_size);

        let quality = params
            .get("quality")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        // Resolve the API key via the same chain the chat engine uses.
        let api_key = match CredentialStore::resolve(&self.provider, self.engine_api_key.as_deref())
        {
            Some((key, _src)) => key,
            None => {
                return Ok(AgentToolResult::error(format!(
                    "No API key resolved for provider '{}'. Set it via the engine key, \
                         ~/.oxios/auth.json, or OXIOS_{}_API_KEY.",
                    self.provider,
                    self.provider.to_uppercase()
                )));
            }
        };

        let store = self.image_sink.clone();

        let provider: Box<dyn ImageGenProvider> = match self.provider.as_str() {
            "fal" => {
                let fal_base = fal_base_url(&self.base_url);
                match FalImageProvider::new(fal_base, api_key.clone(), store) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        return Ok(AgentToolResult::error(format!("provider init failed: {e}")));
                    }
                }
            }
            _ => match OpenAiImageProvider::new(self.base_url.clone(), api_key, store) {
                Ok(p) => Box::new(p),
                Err(e) => return Ok(AgentToolResult::error(format!("provider init failed: {e}"))),
            },
        };

        let reference_image_url = params
            .get("reference_image_url")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let req = ImageGenRequest {
            prompt: prompt.to_owned(),
            model: Some(model),
            n,
            size,
            quality,
            reference_image_url,
        };

        match provider.generate(&req).await {
            Ok(result) => Ok(AgentToolResult::success(
                serde_json::to_string(&GenerationToolOutput::new(result, prompt))
                    .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!(
                "image generation failed: {e}"
            ))),
        }
    }
}

/// Parse an OpenAI size string into [`ImageSize`].
fn parse_size(s: &str) -> Option<ImageSize> {
    match s {
        "1024x1024" => Some(ImageSize::Square1024),
        "1792x1024" => Some(ImageSize::Landscape1792),
        "1024x1792" => Some(ImageSize::Portrait1792),
        _ => None,
    }
}

/// Pick the fal queue base URL: use the configured base unless it is empty or
/// still the OpenAI default (common when the user switches provider to fal).
fn fal_base_url(configured: &str) -> String {
    let c = configured.trim();
    if c.is_empty() || c.contains("openai.com") {
        crate::image_gen::FAL_DEFAULT_BASE.into()
    } else {
        c.into()
    }
}

/// Tool output payload (serialized as the tool result string).
#[derive(serde::Serialize)]
struct GenerationToolOutput {
    action: &'static str,
    images: Vec<GeneratedImage>,
    prompt: String,
    provider: String,
    model: String,
    revised_prompt: Option<String>,
}

impl GenerationToolOutput {
    fn new(r: crate::image_gen::ImageGenResult, prompt: &str) -> Self {
        Self {
            action: "generate",
            prompt: prompt.to_owned(),
            images: r.images,
            provider: r.provider,
            model: r.model,
            revised_prompt: r.revised_prompt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> ImageGenerationTool {
        ImageGenerationTool {
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            default_model: "gpt-image-1".into(),
            default_num: 1,
            engine_api_key: None,
            image_sink: Arc::new(FsImageStore::new(
                std::path::PathBuf::from("/tmp"),
                IMAGE_SERVE_PREFIX.into(),
            )) as Arc<dyn ImageSink>,
        }
    }

    #[test]
    fn parse_size_maps_known_strings() {
        assert_eq!(parse_size("1024x1024"), Some(ImageSize::Square1024));
        assert_eq!(parse_size("1792x1024"), Some(ImageSize::Landscape1792));
        assert_eq!(parse_size("1024x1792"), Some(ImageSize::Portrait1792));
        assert_eq!(parse_size("bogus"), None);
    }

    #[test]
    fn schema_has_generate_and_list_models() {
        let schema = tool().parameters_schema();
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert!(actions.iter().any(|a| a == "generate"));
        assert!(actions.iter().any(|a| a == "list_models"));
    }
}
