//! OpenAI `/v1/images/generations` provider.
//!
//! Also works with OpenAI-compatible endpoints (local Stable Diffusion WebUI,
//! Azure OpenAI, OpenRouter images, …) given the right `base_url`. Response
//! normalization (§4.3 of the port design) handles both `url` and `b64_json`
//! return shapes, since which one a model returns is the user's choice.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;

use crate::image_gen::{
    GeneratedImage, ImageGenError, ImageGenProvider, ImageGenRequest, ImageGenResult, ImageSink,
};

/// OpenAI image-generation client.
pub struct OpenAiImageProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    store: Arc<dyn ImageSink>,
}

impl OpenAiImageProvider {
    /// Construct from already-resolved values.
    ///
    /// The caller (tool/config layer) resolves the credential and image dir;
    /// this keeps the module fully unit-testable with a fake [`ImageSink`].
    pub fn new(
        base_url: String,
        api_key: String,
        store: Arc<dyn ImageSink>,
    ) -> Result<Self, ImageGenError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ImageGenError::Transport(e.to_string()))?;
        Ok(Self {
            client,
            base_url,
            api_key,
            store,
        })
    }
}

#[derive(Deserialize)]
struct OpenAiResponse {
    data: Vec<OpenAiImage>,
}

#[derive(Deserialize)]
struct OpenAiImage {
    url: Option<String>,
    b64_json: Option<String>,
    revised_prompt: Option<String>,
}

#[async_trait]
impl ImageGenProvider for OpenAiImageProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn generate(&self, req: &ImageGenRequest) -> Result<ImageGenResult, ImageGenError> {
        let model = req.model.as_deref().ok_or(ImageGenError::MissingModel)?;
        let body = serde_json::json!({
            "model": model,
            "prompt": req.prompt,
            "n": req.n.clamp(1, 8),
            "size": req.size.map(|s| s.as_str()).unwrap_or("1024x1024"),
        });
        let url = format!("{}/images/generations", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ImageGenError::Transport(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ImageGenError::Http {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| ImageGenError::BadResponse(format!("invalid JSON: {e}")))?;

        let revised_prompt = parsed
            .data
            .iter()
            .rev()
            .find_map(|d| d.revised_prompt.clone());
        let images = normalize_images(&parsed.data, self.store.as_ref())?;

        Ok(ImageGenResult {
            images,
            provider: "openai".into(),
            model: model.into(),
            revised_prompt,
        })
    }
}

/// Map provider image entries to fetchable URLs. Pure over the [`ImageSink`].
///
/// - `url` present  → used directly (e.g. `dall-e-3`).
/// - only `b64_json` → decoded, persisted via the sink, served URL returned
///   (e.g. `gpt-image-1`, which always returns base64).
fn normalize_images(
    data: &[OpenAiImage],
    store: &dyn ImageSink,
) -> Result<Vec<GeneratedImage>, ImageGenError> {
    data.iter()
        .map(|img| match (&img.url, &img.b64_json) {
            (Some(url), _) => Ok(GeneratedImage {
                url: url.clone(),
                width: None,
                height: None,
            }),
            (None, Some(b64)) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| ImageGenError::Base64(e.to_string()))?;
                let url = store.save(bytes, "png")?;
                Ok(GeneratedImage {
                    url,
                    width: None,
                    height: None,
                })
            }
            (None, None) => Err(ImageGenError::BadResponse(
                "image entry has neither url nor b64_json".into(),
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use super::*;
    use crate::image_gen::ImageSink;

    /// Records saves instead of hitting disk — lets us test `normalize_images`
    /// without a real provider/HTTP.
    struct FakeSink {
        saved: Mutex<Vec<(Vec<u8>, String)>>,
    }

    impl ImageSink for FakeSink {
        fn save(&self, bytes: Vec<u8>, ext: &str) -> Result<String, ImageGenError> {
            let n = self.saved.lock().len();
            let url = format!("/api/images/fake-{n}.{ext}");
            self.saved.lock().push((bytes, ext.into()));
            Ok(url)
        }
    }

    fn img(url: Option<&str>, b64: Option<&str>) -> OpenAiImage {
        OpenAiImage {
            url: url.map(Into::into),
            b64_json: b64.map(Into::into),
            revised_prompt: None,
        }
    }

    #[test]
    fn url_response_passes_through() {
        let sink = FakeSink {
            saved: Mutex::new(vec![]),
        };
        let out = normalize_images(&[img(Some("https://cdn/x.png"), None)], &sink).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "https://cdn/x.png");
        // Nothing persisted — url used directly.
        assert!(sink.saved.lock().is_empty());
    }

    #[test]
    fn b64_response_is_decoded_and_persisted() {
        // "hello" → base64
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"hello");
        let sink = FakeSink {
            saved: Mutex::new(vec![]),
        };
        let out = normalize_images(&[img(None, Some(&b64))], &sink).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].url.starts_with("/api/images/fake-0.png"));
        let (bytes, ext) = &sink.saved.lock()[0];
        assert_eq!(bytes, b"hello");
        assert_eq!(ext, "png");
    }

    #[test]
    fn mixed_batch_normalizes_each_entry() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"img");
        let sink = FakeSink {
            saved: Mutex::new(vec![]),
        };
        let out = normalize_images(
            &[img(Some("https://cdn/a.png"), None), img(None, Some(&b64))],
            &sink,
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://cdn/a.png");
        assert!(out[1].url.starts_with("/api/images/"));
        assert_eq!(sink.saved.lock().len(), 1);
    }

    #[test]
    fn missing_url_and_b64_is_an_error() {
        let sink = FakeSink {
            saved: Mutex::new(vec![]),
        };
        let err = normalize_images(&[img(None, None)], &sink).unwrap_err();
        assert!(matches!(err, ImageGenError::BadResponse(_)));
    }
}
