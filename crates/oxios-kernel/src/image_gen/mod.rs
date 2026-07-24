//! Image generation provider clients.
//!
//! `oxi-sdk` 0.58.0 has no image generation capability — the `Provider`
//! trait offers only `stream()`/`name()`, and oxi-ai's
//! `ImageGenerationRequest`/`ImageGenerationResponse` types are dead code
//! (defined, never implemented, not re-exported). This module is Oxios's own
//! image-gen backend, built directly on `reqwest`.
//!
//! Type shapes align with the oxi-ai dead types (`types.rs`) so a future
//! oxi-sdk implementation can replace this module with minimal friction.
//! Unlike oxi-ai (which returns `Vec<Vec<u8>>` bytes), Oxios always normalizes
//! results to URLs — agents consume images as markdown `![](url)`.

mod fal;
mod openai;
mod store;
mod types;

use async_trait::async_trait;

pub(crate) use fal::FAL_DEFAULT_BASE;
pub use fal::FalImageProvider;
pub use openai::OpenAiImageProvider;
pub use store::{FsImageStore, ImageSink};
pub use types::{GeneratedImage, ImageGenRequest, ImageGenResult, ImageSize};

/// Errors from image generation.
#[derive(Debug, thiserror::Error)]
pub enum ImageGenError {
    /// The request did not specify a model (the caller must resolve a default).
    #[error("image generation requires a model")]
    MissingModel,
    /// The provider response had no usable image (neither `url` nor `b64_json`).
    #[error("bad image response: {0}")]
    BadResponse(String),
    /// The provider returned a non-success HTTP status.
    #[error("provider error ({status}): {body}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Response body (truncated by the provider).
        body: String,
    },
    /// A network/transport error.
    #[error("transport error: {0}")]
    Transport(String),
    /// Failed to persist image bytes locally.
    #[error("image store error: {0}")]
    Store(String),
    /// Base64 decoding of a `b64_json` response failed.
    #[error("base64 decode error: {0}")]
    Base64(String),
    /// Polling timed out before the provider returned a result (async providers).
    #[error("operation timed out: {context}")]
    Timeout {
        /// Context describing what timed out.
        context: String,
    },
}

/// A text-to-image provider.
///
/// Implementations resolve their own credentials and base URL at construction
/// time (see [`OpenAiImageProvider::new`]). Callers inject an already-resolved
/// `(base_url, api_key, image_dir)` so the module stays unit-testable with
/// mocks — credential resolution is the tool layer's job.
#[async_trait]
pub trait ImageGenProvider: Send + Sync {
    /// Provider identifier (e.g. `"openai"`).
    fn name(&self) -> &'static str;

    /// Generate images. The result's `images[].url` are always fetchable URLs
    /// — b64-only providers persist locally and return a served URL.
    ///
    /// `req.model` MUST be set; the caller resolves a config default first.
    async fn generate(&self, req: &ImageGenRequest) -> Result<ImageGenResult, ImageGenError>;
}
