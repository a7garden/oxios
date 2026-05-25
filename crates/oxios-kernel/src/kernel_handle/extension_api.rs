//! Extension API — skills and host tools (RFC-009 Phase 3).
//!
//! The unified `SkillManager` replaces the previous separate
//! `ProgramManager` + `SkillStore` pair.

use crate::host_tools::{HostToolStatus, HostToolValidator};
use crate::program::{HostRequirementsCheck, InstallSource, Program};
use crate::skill::{Skill, SkillManager, SkillMeta};
use std::sync::Arc;

/// Extension system calls.
pub struct ExtensionApi {
    /// Unified skill manager (programs + skills).
    pub(crate) skill_manager: Arc<SkillManager>,
    pub(crate) host_tool_validator: Arc<HostToolValidator>,
}

impl ExtensionApi {
    /// Create a new ExtensionApi.
    pub fn new(
        skill_manager: Arc<SkillManager>,
        host_tool_validator: Arc<HostToolValidator>,
    ) -> Self {
        Self {
            skill_manager,
            host_tool_validator,
        }
    }

    /// List installed programs (delegates to SkillManager).
    pub async fn list_programs(&self) -> Vec<Program> {
        self.skill_manager.list_programs().await
    }

    /// Get program details.
    pub async fn get_program(&self, name: &str) -> Option<Program> {
        self.skill_manager.get_program(name).await
    }

    /// Install a program from source.
    pub async fn install_program(&self, source: InstallSource) -> anyhow::Result<Program> {
        self.skill_manager.install_program(source).await
    }

    /// Uninstall a program.
    pub async fn uninstall_program(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.uninstall_program(name).await
    }

    /// Enable a program.
    pub async fn enable_program(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.enable_program(name).await
    }

    /// Disable a program.
    pub async fn disable_program(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.disable_program(name).await
    }

    /// Check host requirements for a program.
    pub async fn check_host_requirements(
        &self,
        name: &str,
    ) -> anyhow::Result<HostRequirementsCheck> {
        self.skill_manager.check_host_requirements(name).await
    }

    /// List all skills.
    pub async fn list_skills(&self) -> anyhow::Result<Vec<SkillMeta>> {
        self.skill_manager.list_skills().await
    }

    /// Load skill by name.
    pub async fn load_skill(&self, name: &str) -> anyhow::Result<Option<Skill>> {
        self.skill_manager.load_skill(name).await
    }

    /// Create a new skill.
    pub async fn create_skill(
        &self,
        name: &str,
        description: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        self.skill_manager.create_skill(name, description, content).await
    }

    /// Delete a skill.
    pub async fn delete_skill(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.delete_skill(name).await
    }

    /// Skill manager reference.
    pub fn skill_manager(&self) -> &Arc<SkillManager> {
        &self.skill_manager
    }

    /// Full host tool check.
    pub fn check_host_tools(&self) -> HostToolStatus {
        self.host_tool_validator.full_check()
    }
}
