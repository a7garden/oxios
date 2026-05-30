//! Project API — Project management system calls (RFC-011).
//!
//! Provides API endpoints for:
//! - Listing and querying Projects
//! - CRUD operations on Projects
//! - Project-Memory association (link/unlink)

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::project::{Project, ProjectManager};

/// Serialized Project info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub emoji: String,
    pub memory_visible: bool,
    pub last_active: String,
}

impl From<&Project> for ProjectInfo {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name.clone(),
            description: project.description.clone(),
            source: project.source.to_string(),
            paths: project
                .paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            tags: project.tags.clone(),
            emoji: project.emoji.clone(),
            memory_visible: project.memory_visible,
            last_active: project.last_active_at.to_rfc3339(),
        }
    }
}

/// Project system calls.
#[allow(dead_code)]
pub struct ProjectApi {
    /// Project manager for Project lifecycle.
    pub(crate) project_manager: Arc<ProjectManager>,
}

impl ProjectApi {
    /// Create a new ProjectApi.
    pub fn new(project_manager: Arc<ProjectManager>) -> Self {
        Self { project_manager }
    }

    /// List all Projects.
    pub fn list_projects(&self) -> Vec<ProjectInfo> {
        self.project_manager
            .list_projects()
            .iter()
            .map(|p| ProjectInfo::from(p))
            .collect()
    }

    /// Get Project details by ID.
    pub fn get_project(&self, id: &str) -> Option<ProjectInfo> {
        let project_id = uuid::Uuid::parse_str(id).ok()?;
        self.project_manager
            .get_project(project_id)
            .as_ref()
            .map(ProjectInfo::from)
    }
}
