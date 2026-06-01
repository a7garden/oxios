//! Skills.sh API client.
//!
//! HTTP client for the skills.sh REST API (v1). Supports search, listing,
//! skill detail, curated skills, and security audits.
//!
//! # Authentication
//!
//! All endpoints require a Bearer token (`SKILLS_SH_TOKEN` env var or config).
//! Request an API key by emailing `skills-api@vercel.com`.

use anyhow::{Context, Result};
use url::Url;

use super::types::{
    SkillsShAuditResponse, SkillsShCuratedResponse, SkillsShListResponse, SkillsShSearchResponse,
    SkillsShSkillDetail,
};

const DEFAULT_BASE_URL: &str = "https://skills.sh";

/// Skills.sh API client.
#[derive(Clone)]
pub struct SkillsShClient {
    base_url: Url,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl SkillsShClient {
    /// Create a new client.
    ///
    /// - `base_url`: Override the default `https://skills.sh`.
    /// - `api_key`: Bearer token for authentication. If `None`, falls back to
    ///   the `SKILLS_SH_TOKEN` environment variable.
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Result<Self> {
        let base = base_url
            .map(|s| Url::parse(&s))
            .unwrap_or_else(|| Url::parse(DEFAULT_BASE_URL))?;
        let base = base
            .join("/")
            .map_err(|e| anyhow::anyhow!("invalid base URL: {e}"))?;

        let api_key = api_key.or_else(|| std::env::var("SKILLS_SH_TOKEN").ok());

        Ok(Self {
            base_url: base,
            client: reqwest::Client::new(),
            api_key,
        })
    }

    /// Returns the base URL.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Whether an API key is configured.
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Search for skills by name, source, or description.
    ///
    /// Single-word queries use fuzzy matching. Multi-word queries use semantic search.
    pub async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<SkillsShSearchResponse> {
        let mut url = self.base_url.join("/api/v1/skills/search")?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("limit", &limit.unwrap_or(50).to_string());

        let resp = self.get_response(url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("skills.sh search failed ({status}): {body}");
        }

        resp.json().await.context("parse skills.sh search response")
    }

    /// Paginated leaderboard of all skills.
    ///
    /// - `view`: `"all-time"` (default), `"trending"`, or `"hot"`.
    /// - `page`: 0-indexed page number.
    /// - `per_page`: 1–500 results per page.
    pub async fn list(
        &self,
        view: Option<&str>,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> Result<SkillsShListResponse> {
        let mut url = self.base_url.join("/api/v1/skills")?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("view", view.unwrap_or("all-time"));
            if let Some(p) = page {
                qp.append_pair("page", &p.to_string());
            }
            if let Some(pp) = per_page {
                qp.append_pair("per_page", &pp.to_string());
            }
        }

        let resp = self.get_response(url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("skills.sh list failed ({status}): {body}");
        }

        resp.json().await.context("parse skills.sh list response")
    }

    /// Get detailed information about a single skill including file contents.
    ///
    /// The `id` parameter is the stable `"{source}/{slug}"` identifier,
    /// e.g. `"vercel-labs/agent-skills/next-js-development"`.
    pub async fn get_skill(&self, id: &str) -> Result<SkillsShSkillDetail> {
        // The id is used directly as the path: /api/v1/skills/{id}
        let url = self
            .base_url
            .join(&format!("/api/v1/skills/{id}"))
            .context("construct skill detail URL")?;

        let resp = self.get_response(url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("skills.sh get_skill {id} failed ({status}): {body}");
        }

        resp.json()
            .await
            .context("parse skills.sh skill detail response")
    }

    /// Get the official curated set of first-party skills.
    pub async fn curated(&self) -> Result<SkillsShCuratedResponse> {
        let url = self.base_url.join("/api/v1/skills/curated")?;

        let resp = self.get_response(url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("skills.sh curated failed ({status}): {body}");
        }

        resp.json()
            .await
            .context("parse skills.sh curated response")
    }

    /// Get security audit results for a skill.
    pub async fn audit(&self, id: &str) -> Result<SkillsShAuditResponse> {
        let url = self
            .base_url
            .join(&format!("/api/v1/skills/audit/{id}"))
            .context("construct audit URL")?;

        let resp = self.get_response(url).await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("no audits found for skill {id}");
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("skills.sh audit {id} failed ({status}): {body}");
        }

        resp.json().await.context("parse skills.sh audit response")
    }

    // ─── Internal ────────────────────────────────────────────────────────

    /// Build an authenticated GET request and send it.
    async fn get_response(&self, url: Url) -> Result<reqwest::Response> {
        let mut req = self.client.get(url);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        let resp = req.send().await?;
        Ok(resp)
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new_default() {
        let client = SkillsShClient::new(None, None).unwrap();
        assert_eq!(client.base_url.as_str(), "https://skills.sh/");
    }

    #[test]
    fn test_client_new_custom_url() {
        let client =
            SkillsShClient::new(Some("https://staging.skills.sh".to_string()), None).unwrap();
        assert_eq!(client.base_url.as_str(), "https://staging.skills.sh/");
    }

    #[test]
    fn test_client_api_key_from_param() {
        let client = SkillsShClient::new(None, Some("sk_test_123".to_string())).unwrap();
        assert!(client.has_api_key());
    }
}
