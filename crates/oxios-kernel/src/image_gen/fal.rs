//! fal.ai image-generation provider (queue API: submit → poll → fetch).
//!
//! fal models are asynchronous: submit returns a `request_id` plus absolute
//! `status_url`/`response_url`; poll `status_url` until `COMPLETED`, then GET
//! `response_url`. This mirrors LobeHub's `fal.subscribe()` (which blocks
//! until completion). Verified against the fal queue REST docs
//! (<https://fal.ai/docs/documentation/model-apis/inference/queue>):
//!
//! - Host: `https://queue.fal.run`
//! - Submit: `POST {base}/{model_id}` — body is the input object **directly**
//!   (NOT wrapped in `{"input": ...}`; the JS SDK wraps client-side, the REST
//!   API does not).
//! - Submit response: `{request_id, status_url, response_url, ...}` (absolute
//!   URLs) — use them verbatim, sidestepping model_id-in-path construction.
//! - Status: `GET {status_url}` → `{status, error?, error_type?}`. `COMPLETED`
//!   WITH an `error` field means the request FAILED.
//! - Result: `GET {response_url}` → model-specific, images at TOP LEVEL
//!   (`{images: [{url, width, height, content_type}], ...}`).
//! - Auth: `Authorization: Key {FAL_KEY}`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;

use crate::image_gen::{
    GeneratedImage, ImageGenError, ImageGenProvider, ImageGenRequest, ImageGenResult, ImageSink,
    ImageSize,
};

/// Default fal queue host.
pub const FAL_DEFAULT_BASE: &str = "https://queue.fal.run";
/// Poll interval for fal queue status (matches LobeHub `WAIT_POLL_INTERVAL_MS`).
const POLL_INTERVAL: Duration = Duration::from_millis(3000);
/// Max wait for a fal generation (matches LobeHub `MAX_WAIT_TIMEOUT_MS`).
const MAX_WAIT: Duration = Duration::from_millis(175_000);

/// fal.ai provider via the queue REST API.
pub struct FalImageProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    store: Arc<dyn ImageSink>,
}

impl FalImageProvider {
    /// Construct from already-resolved values (the tool layer resolves the
    /// `fal` credential and image dir).
    pub fn new(
        base_url: String,
        api_key: String,
        store: Arc<dyn ImageSink>,
    ) -> Result<Self, ImageGenError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ImageGenError::Transport(e.to_string()))?;
        Ok(Self {
            client,
            base_url,
            api_key,
            store,
        })
    }

    fn auth(&self) -> String {
        format!("Key {}", self.api_key)
    }

    /// Submit URL: `{base}/{model}` (model carries its own path, e.g.
    /// `fal-ai/flux/schnell`).
    fn submit_url(&self, model: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            model.trim_start_matches('/')
        )
    }

    async fn authed_get<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
    ) -> Result<T, ImageGenError> {
        let resp = self
            .client
            .get(url)
            .header(reqwest::header::AUTHORIZATION, self.auth())
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
        resp.json::<T>()
            .await
            .map_err(|e| ImageGenError::BadResponse(format!("invalid JSON: {e}")))
    }
}

#[async_trait]
impl ImageGenProvider for FalImageProvider {
    fn name(&self) -> &'static str {
        "fal"
    }

    async fn generate(&self, req: &ImageGenRequest) -> Result<ImageGenResult, ImageGenError> {
        let model = req.model.as_deref().ok_or(ImageGenError::MissingModel)?;

        // 1. Submit — body is the input object DIRECTLY (not wrapped).
        let submit_resp = self
            .client
            .post(self.submit_url(model))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&build_input(req))
            .send()
            .await
            .map_err(|e| ImageGenError::Transport(e.to_string()))?;
        let status = submit_resp.status();
        if !status.is_success() {
            let body = submit_resp.text().await.unwrap_or_default();
            return Err(ImageGenError::Http {
                status: status.as_u16(),
                body,
            });
        }
        let submit: FalSubmit = submit_resp
            .json()
            .await
            .map_err(|e| ImageGenError::BadResponse(format!("submit parse: {e}")))?;
        let status_url = submit
            .status_url
            .ok_or_else(|| ImageGenError::BadResponse("submit returned no status_url".into()))?;
        let response_url = submit
            .response_url
            .ok_or_else(|| ImageGenError::BadResponse("submit returned no response_url".into()))?;

        // 2. Poll status until COMPLETED / FAILED / timeout.
        let deadline = Instant::now() + MAX_WAIT;
        loop {
            let st: FalStatus = self.authed_get(&status_url).await?;
            match st.status.as_str() {
                "COMPLETED" => {
                    // COMPLETED may still carry an error → the job failed.
                    if let Some(err) = st.error {
                        return Err(ImageGenError::BadResponse(format!("fal job failed: {err}")));
                    }
                    break;
                }
                "FAILED" | "ERROR" => {
                    return Err(ImageGenError::BadResponse(format!(
                        "fal job failed: {}",
                        st.error.unwrap_or_else(|| "unknown".into())
                    )));
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(ImageGenError::Timeout {
                    context: "fal generation".into(),
                });
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }

        // 3. Fetch result — images at TOP LEVEL (no `.data` wrapper).
        let result: FalResult = self.authed_get(&response_url).await?;
        let images = normalize_fal_images(&result.images, self.store.as_ref())?;

        Ok(ImageGenResult {
            images,
            provider: "fal".into(),
            model: model.into(),
            revised_prompt: result.revised_prompt,
        })
    }
}

// ── Wire types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FalSubmit {
    #[serde(default)]
    #[allow(dead_code)]
    request_id: Option<String>,
    #[serde(default)]
    status_url: Option<String>,
    #[serde(default)]
    response_url: Option<String>,
}

#[derive(Deserialize)]
struct FalStatus {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    error_type: Option<String>,
}

#[derive(Deserialize)]
struct FalResult {
    #[serde(default)]
    images: Vec<FalImage>,
    #[serde(default)]
    revised_prompt: Option<String>,
}

#[derive(Deserialize)]
struct FalImage {
    url: Option<String>,
    #[serde(default, rename = "b64_json")]
    b64_json: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

// ── Pure helpers (unit-tested) ────────────────────────────────────────────

/// Build the fal input payload (sent DIRECTLY as the submit body).
fn build_input(req: &ImageGenRequest) -> serde_json::Value {
    let mut input = serde_json::json!({
        "prompt": req.prompt,
        "num_images": req.n.clamp(1, 8),
    });
    if let Some(size) = req.size {
        let (w, h) = match size {
            ImageSize::Square1024 => (1024, 1024),
            ImageSize::Landscape1792 => (1792, 1024),
            ImageSize::Portrait1792 => (1024, 1792),
        };
        input["image_size"] = serde_json::json!({ "width": w, "height": h });
    }
    if let Some(ref url) = req.reference_image_url {
        input["image_url"] = serde_json::json!(url);
    }
    input
}

/// Normalize fal output images to fetchable URLs (carrying dimensions).
fn normalize_fal_images(
    images: &[FalImage],
    store: &dyn ImageSink,
) -> Result<Vec<GeneratedImage>, ImageGenError> {
    if images.is_empty() {
        return Err(ImageGenError::BadResponse(
            "fal result has no images".into(),
        ));
    }
    images
        .iter()
        .map(|img| match (&img.url, &img.b64_json) {
            (Some(url), _) => Ok(GeneratedImage {
                url: url.clone(),
                width: img.width,
                height: img.height,
            }),
            (None, Some(b64)) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| ImageGenError::Base64(e.to_string()))?;
                let url = store.save(bytes, "png")?;
                Ok(GeneratedImage {
                    url,
                    width: img.width,
                    height: img.height,
                })
            }
            (None, None) => Err(ImageGenError::BadResponse(
                "fal image has neither url nor b64_json".into(),
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use super::*;
    use crate::image_gen::ImageSink;

    struct FakeSink {
        saved: Mutex<Vec<(Vec<u8>, String)>>,
    }
    impl ImageSink for FakeSink {
        fn save(&self, bytes: Vec<u8>, ext: &str) -> Result<String, ImageGenError> {
            let n = self.saved.lock().len();
            let url = format!("/api/images/fal-{n}.{ext}");
            self.saved.lock().push((bytes, ext.into()));
            Ok(url)
        }
    }
    fn sink() -> FakeSink {
        FakeSink {
            saved: Mutex::new(vec![]),
        }
    }
    fn img(url: Option<&str>, b64: Option<&str>) -> FalImage {
        FalImage {
            url: url.map(Into::into),
            b64_json: b64.map(Into::into),
            width: Some(1024),
            height: Some(1024),
        }
    }

    #[test]
    fn build_input_maps_prompt_num_size_and_reference() {
        let req = ImageGenRequest {
            prompt: "a cat".into(),
            model: Some("fal-ai/flux/dev".into()),
            n: 3,
            size: Some(ImageSize::Landscape1792),
            quality: None,
            reference_image_url: Some("https://x/ref.png".into()),
        };
        let input = build_input(&req);
        assert_eq!(input["prompt"], "a cat");
        assert_eq!(input["num_images"], 3);
        assert_eq!(input["image_size"]["width"], 1792);
        assert_eq!(input["image_size"]["height"], 1024);
        assert_eq!(input["image_url"], "https://x/ref.png");
    }

    #[test]
    fn url_images_pass_through_with_dimensions() {
        let s = sink();
        let r = normalize_fal_images(&[img(Some("https://fal/cdn/1.png"), None)], &s).unwrap();
        assert_eq!(r[0].url, "https://fal/cdn/1.png");
        assert_eq!(r[0].width, Some(1024));
        assert!(s.saved.lock().is_empty());
    }

    #[test]
    fn b64_images_are_persisted() {
        let s = sink();
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"falimg");
        let r = normalize_fal_images(&[img(None, Some(&b64))], &s).unwrap();
        assert!(r[0].url.starts_with("/api/images/fal-0.png"));
        assert_eq!(s.saved.lock()[0].0, b"falimg");
    }

    #[test]
    fn empty_images_is_an_error() {
        let s = sink();
        assert!(normalize_fal_images(&[], &s).is_err());
    }

    #[test]
    fn submit_url_places_model_directly_in_path() {
        let p = FalImageProvider {
            client: reqwest::Client::new(),
            base_url: "https://queue.fal.run".into(),
            api_key: "k".into(),
            store: Arc::new(crate::image_gen::FsImageStore::new(
                std::path::PathBuf::from("/tmp"),
                "/api/images/".into(),
            )),
        };
        assert_eq!(
            p.submit_url("fal-ai/flux/schnell"),
            "https://queue.fal.run/fal-ai/flux/schnell"
        );
        assert_eq!(
            p.submit_url("/fal-ai/flux/schnell"),
            "https://queue.fal.run/fal-ai/flux/schnell"
        );
    }
}
