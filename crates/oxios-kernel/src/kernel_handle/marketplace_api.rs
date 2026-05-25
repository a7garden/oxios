//! Marketplace API — ClawHub integration facade.
//!
//! Exposes search, install, and update for ClawHub skills via the kernel.

use std::sync::Arc;

use crate::clawhub::{
    ClawHubClient, ClawHubInstaller, ClawHubSearchResult, ClawHubSkillDetail, InstallResult,
    UpdateAvailable, UpdateResult,
};

/// Marketplace system calls — wraps ClawHubInstaller and ClawHubClient.
#[derive(Clone)]
pub struct MarketplaceApi {
    installer: Arc<ClawHubInstaller>,
    client: Arc<ClawHubClient>,
}

impl MarketplaceApi {
    /// Create a new MarketplaceApi.
    pub fn new(installer: Arc<ClawHubInstaller>, client: Arc<ClawHubClient>) -> Self {
        Self { installer, client }
    }

    /// Search ClawHub for skills by query.
    pub async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<ClawHubSearchResult>> {
        self.client.search_skills(query, limit).await
    }

    /// Get full detail for a skill by slug.
    pub async fn get_skill(&self, slug: &str) -> anyhow::Result<ClawHubSkillDetail> {
        self.client.get_skill(slug).await
    }

    /// Install a skill from ClawHub.
    ///
    /// If `version` is `None`, the latest version is installed.
    pub async fn install(
        &self,
        slug: &str,
        version: Option<&str>,
    ) -> anyhow::Result<InstallResult> {
        self.installer.install(slug, version).await
    }

    /// Check which installed skills have newer versions available.
    pub async fn check_updates(&self) -> anyhow::Result<Vec<UpdateAvailable>> {
        self.installer.check_updates().await
    }

    /// Update a specific installed skill to the latest version.
    pub async fn update(&self, slug: &str) -> anyhow::Result<UpdateResult> {
        self.installer.update(slug).await
    }

    /// Update all installed ClawHub skills.
    pub async fn update_all(&self) -> anyhow::Result<Vec<UpdateResult>> {
        self.installer.update_all().await
    }
}