//! ClawHub marketplace types.
//!
//! API types matching ClawHub API responses, plus origin/lockfile types.

use std::collections::HashMap;

use serde::Deserialize;

/// ClawHub search result.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct ClawHubSearchResult {
    pub score: f64,
    pub slug: String,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<i64>,
}

/// ClawHub skill detail.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClawHubSkillDetail {
    #[serde(default)]
    pub skill: Option<ClawHubSkillMeta>,
    #[serde(rename = "latestVersion", default)]
    pub latest_version: Option<ClawHubVersion>,
    #[serde(default)]
    pub metadata: Option<ClawHubMetadata>,
    #[serde(default)]
    pub owner: Option<ClawHubOwner>,
}

/// ClawHub skill meta.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClawHubSkillMeta {
    pub slug: String,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
    #[serde(rename = "createdAt", default)]
    pub created_at: i64,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: i64,
}

/// ClawHub version entry.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClawHubVersion {
    pub version: String,
    #[serde(rename = "createdAt", default)]
    pub created_at: i64,
    pub changelog: Option<String>,
}

/// ClawHub skill metadata.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClawHubMetadata {
    #[serde(default)]
    pub os: Option<Vec<String>>,
    #[serde(default)]
    pub systems: Option<Vec<String>>,
}

/// ClawHub owner info.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClawHubOwner {
    #[serde(default)]
    pub handle: Option<String>,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}

/// ClawHub origin file (inside skill dir as .clawhub/origin.json).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClawHubOrigin {
    pub version: u32,
    pub registry: String,
    pub slug: String,
    #[serde(rename = "installedVersion")]
    pub installed_version: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
}

/// ClawHub lockfile (at workspace root as .clawhub/lock.json).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClawHubLockfile {
    pub version: u32,
    pub skills: HashMap<String, ClawHubLockEntry>,
}

/// A single entry in the lockfile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClawHubLockEntry {
    pub version: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
}

/// Response wrapper for search results.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<ClawHubSearchResult>,
}