//! Skills.sh API types.
//!
//! Matches the skills.sh v1 REST API response shapes.

use serde::{Deserialize, Serialize};

/// A single skill returned by listing/search endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsShSkill {
    /// Stable unique identifier: `"{source}/{slug}"`.
    pub id: String,
    /// URL-safe skill slug.
    pub slug: String,
    /// Human-readable name.
    pub name: String,
    /// Source repository or provider (e.g. `"vercel-labs/agent-skills"`).
    pub source: String,
    /// Total deduplicated install count.
    pub installs: i64,
    /// `"github"` or `"well-known"`.
    #[serde(rename = "sourceType")]
    pub source_type: String,
    /// GitHub URL or well-known base URL.
    #[serde(rename = "installUrl")]
    pub install_url: Option<String>,
    /// Direct link on skills.sh.
    pub url: String,
    /// Present and true if this skill is a detected fork/copy.
    #[serde(rename = "isDuplicate", default, skip_serializing_if = "std::ops::Not::not")]
    pub is_duplicate: bool,
}

/// Paginated skill listing response.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsShListResponse {
    pub data: Vec<SkillsShSkill>,
    pub pagination: SkillsShPagination,
}

/// Pagination metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsShPagination {
    pub page: i64,
    #[serde(rename = "perPage")]
    pub per_page: i64,
    pub total: i64,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
}

/// Search response from skills.sh.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsShSearchResponse {
    pub data: Vec<SkillsShSkill>,
    pub query: String,
    /// `"fuzzy"` or `"semantic"`.
    #[serde(rename = "searchType")]
    pub search_type: String,
    pub count: i64,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<i64>,
}

/// Detailed skill information with file contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsShSkillDetail {
    pub id: String,
    pub source: String,
    pub slug: String,
    pub installs: i64,
    /// SHA-256 hash of the skill's file contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// All files in the skill folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<SkillsShFile>>,
}

/// A single file within a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsShFile {
    /// Relative filename (e.g. `"SKILL.md"`).
    pub path: String,
    /// Full text content.
    pub contents: String,
}

/// Security audit result for a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsShAuditEntry {
    pub provider: String,
    pub slug: String,
    /// `"pass"`, `"warn"`, or `"fail"`.
    pub status: String,
    pub summary: String,
    #[serde(rename = "auditedAt")]
    pub audited_at: String,
    #[serde(rename = "riskLevel", default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
}

/// Audit response from skills.sh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsShAuditResponse {
    pub id: String,
    pub source: String,
    pub slug: String,
    pub audits: Vec<SkillsShAuditEntry>,
}

/// Curated owner entry.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsShCuratedOwner {
    pub owner: String,
    #[serde(rename = "totalInstalls")]
    pub total_installs: i64,
    #[serde(rename = "featuredRepo")]
    pub featured_repo: String,
    #[serde(rename = "featuredSkill")]
    pub featured_skill: String,
    pub skills: Vec<SkillsShSkill>,
}

/// Curated response from skills.sh.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsShCuratedResponse {
    pub data: Vec<SkillsShCuratedOwner>,
    #[serde(rename = "totalOwners")]
    pub total_owners: i64,
    #[serde(rename = "totalSkills")]
    pub total_skills: i64,
    #[serde(rename = "generatedAt")]
    pub generated_at: String,
}
