//! Request/result types for image generation.
//!
//! Field names align with oxicode-ai's dead-code types
//! (`ImageGenerationRequest`/`ImageGenerationResponse`, `oxicode-ai/src/types.rs:335-395`)
//! so a future oxicode-sdk implementation can be swapped in with minimal friction.
//! Unlike oxicode-ai (which returns `Vec<Vec<u8>>` bytes), Oxios returns URLs —
//! agents consume images as markdown `![](url)`.

use serde::{Deserialize, Serialize};

/// A text-to-image generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenRequest {
    /// The prompt describing the desired image.
    pub prompt: String,
    /// Provider model id. `None` → caller (tool/config) must supply a default.
    pub model: Option<String>,
    /// Number of images to generate (1-8).
    #[serde(default = "default_n")]
    pub n: u8,
    /// Output dimensions. `None` → provider default.
    pub size: Option<ImageSize>,
    /// Quality hint (e.g. `"standard"`, `"hd"`). Provider-specific.
    pub quality: Option<String>,
    /// Optional reference image URL for image-to-image (Phase 2).
    pub reference_image_url: Option<String>,
}

fn default_n() -> u8 {
    1
}

/// Supported image dimensions (OpenAI-compatible size strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageSize {
    /// 1024×1024 square.
    #[serde(rename = "1024x1024")]
    Square1024,
    /// 1792×1024 landscape.
    #[serde(rename = "1792x1024")]
    Landscape1792,
    /// 1024×1792 portrait.
    #[serde(rename = "1024x1792")]
    Portrait1792,
}

impl ImageSize {
    /// The wire string sent to the provider.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Square1024 => "1024x1024",
            Self::Landscape1792 => "1792x1024",
            Self::Portrait1792 => "1024x1792",
        }
    }
}

/// A single generated image, normalized to a URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    /// A URL the frontend can fetch. Either the provider's CDN URL or a
    /// locally-served `/api/images/<uuid>.<ext>` (for b64-only providers).
    pub url: String,
    /// Pixel width, if reported by the provider.
    pub width: Option<u32>,
    /// Pixel height, if reported by the provider.
    pub height: Option<u32>,
}

/// The result of an image generation call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenResult {
    /// Generated images, each normalized to a URL.
    pub images: Vec<GeneratedImage>,
    /// Provider that produced the images.
    pub provider: String,
    /// Model used (resolved default or explicit).
    pub model: String,
    /// Revised prompt, if the provider rewrites it (e.g. dall-e-3).
    pub revised_prompt: Option<String>,
}
