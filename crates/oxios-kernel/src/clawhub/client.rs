//! ClawHub API client.
//!
//! Thin wrapper around ClawHub REST API — search, get detail, download archive.

use std::path::PathBuf;

use anyhow::Result;
use sha2::Digest;
use url::Url;

use super::types::{ClawHubSearchResult, ClawHubSkillDetail, SearchResponse};

const DEFAULT_BASE_URL: &str = "https://clawhub.ai";

/// Result of a successful archive download.
#[derive(Debug)]
pub struct DownloadedArchive {
    /// Temporary file containing the zip bytes.
    pub path: PathBuf,
    /// SHA-256 hex digest of the downloaded bytes.
    pub sha256: String,
}

/// ClawHub API client.
#[derive(Clone)]
pub struct ClawHubClient {
    base_url: Url,
    client: reqwest::Client,
}

impl ClawHubClient {
    /// Create a new client targeting the given base URL, or the public
    /// ClawHub registry if `base_url` is `None`.
    pub fn new(base_url: Option<String>) -> Result<Self> {
        let base = base_url
            .map(|s| Url::parse(&s))
            .unwrap_or_else(|| Url::parse(DEFAULT_BASE_URL))?;
        let base = base.join("/").map_err(|e| anyhow::anyhow!("invalid base URL: {}", e))?;

        Ok(Self {
            base_url: base,
            client: reqwest::Client::new(),
        })
    }

    /// Returns the base URL of the registry this client targets.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Search for skills by query string.
    pub async fn search_skills(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ClawHubSearchResult>> {
        let mut url = self.base_url.join("/api/v1/search")?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("limit", &limit.unwrap_or(20).to_string());

        let mut req = self.client.get(url);
        if let Ok(token) = std::env::var("CLAWHUB_TOKEN") {
            if !token.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "ClawHub search failed ({}): {}",
                status,
                body
            );
        }

        let body: SearchResponse = resp.json().await?;
        Ok(body.results)
    }

    /// Fetch full detail for a skill by slug.
    pub async fn get_skill(&self, slug: &str) -> Result<ClawHubSkillDetail> {
        let url = self.base_url.join(&format!("/api/v1/skills/{}", slug))?;

        let mut req = self.client.get(url);
        if let Ok(token) = std::env::var("CLAWHUB_TOKEN") {
            if !token.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "ClawHub get_skill {} failed ({}): {}",
                slug,
                status,
                body
            );
        }

        let detail: ClawHubSkillDetail = resp.json().await?;
        Ok(detail)
    }

    /// Download a skill archive (zip) returning the temp file path and sha256.
    pub async fn download_skill(
        &self,
        slug: &str,
        version: Option<&str>,
    ) -> Result<DownloadedArchive> {
        let mut url = self.base_url.join("/api/v1/download")?;
        url.query_pairs_mut()
            .append_pair("slug", slug);
        if let Some(v) = version {
            url.query_pairs_mut().append_pair("version", v);
        }

        let mut req = self.client.get(url);
        if let Ok(token) = std::env::var("CLAWHUB_TOKEN") {
            if !token.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "ClawHub download {} failed ({}): {}",
                slug,
                status,
                body
            );
        }

        let bytes = resp.bytes().await?;
        let sha256 = sha2::Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        // Write to a temp file (deleted when dropped)
        let mut tmp = tempfile::Builder::new()
            .prefix("clawhub-")
            .suffix(".zip")
            .tempfile()?;
        std::io::Write::write_all(&mut tmp, &bytes)?;

        let path = tmp.into_temp_path().keep().map_err(|e| {
            anyhow::anyhow!("failed to persist temp file: {}", e)
        })?;

        Ok(DownloadedArchive { path, sha256 })
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new_default() {
        let client = ClawHubClient::new(None).unwrap();
        assert_eq!(client.base_url.as_str(), "https://clawhub.ai/");
    }

    #[test]
    fn test_client_new_custom_url() {
        let client =
            ClawHubClient::new(Some("https://staging.clawhub.ai".to_string())).unwrap();
        assert_eq!(client.base_url.as_str(), "https://staging.clawhub.ai/");
    }

    #[test]
    fn test_client_new_invalid_url() {
        assert!(ClawHubClient::new(Some("not-a-url".to_string())).is_err());
    }
}